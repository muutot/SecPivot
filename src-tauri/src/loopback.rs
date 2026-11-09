//! Bounded connection workers for the opt-in browser integration servers.
//! Admission happens before spawning, including incomplete HTTP/WS handshakes.
//! The budget belongs to managed service state and survives listener restarts.

use std::collections::HashMap;
use std::io;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

mod stream;
pub(crate) use stream::CancelableStream;

pub(crate) const MAX_CONNECTION_WORKERS: usize = 32;

#[derive(Default)]
pub(crate) struct ConnectionBudget {
    active: AtomicUsize,
}

struct ConnectionPermit(Arc<ConnectionBudget>);

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::Release);
    }
}

impl ConnectionBudget {
    /// Rejected tasks are dropped immediately; no queue retains their sockets.
    /// The permit also drops on thread creation failure or handler unwind.
    pub(crate) fn try_spawn(
        self: &Arc<Self>,
        task: impl FnOnce() + Send + 'static,
    ) -> io::Result<Option<JoinHandle<()>>> {
        if self
            .active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < MAX_CONNECTION_WORKERS).then_some(active + 1)
            })
            .is_err()
        {
            return Ok(None);
        }
        let permit = ConnectionPermit(Arc::clone(self));
        std::thread::Builder::new()
            .name("browser-connection".into())
            .spawn(move || {
                let _permit = permit;
                task();
            })
            .map(Some)
    }
}

#[derive(Default)]
struct LifetimeState {
    stopped: bool,
    next_id: u64,
    sockets: HashMap<u64, TcpStream>,
    approvals: HashMap<u64, mpsc::Sender<bool>>,
}

/// One listener generation. Restart uses a fresh lifetime, while the worker
/// budget remains shared. No key, password or decrypted request lives here.
#[derive(Default)]
pub(crate) struct ListenerLifetime {
    state: Mutex<LifetimeState>,
}

enum RegistrationKind {
    Socket,
    Approval,
}

pub(crate) struct LifetimeRegistration {
    owner: Arc<ListenerLifetime>,
    id: u64,
    kind: RegistrationKind,
}

impl Drop for LifetimeRegistration {
    fn drop(&mut self) {
        let mut state = self
            .owner
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match self.kind {
            RegistrationKind::Socket => {
                state.sockets.remove(&self.id);
            }
            RegistrationKind::Approval => {
                state.approvals.remove(&self.id);
            }
        }
    }
}

impl ListenerLifetime {
    pub(crate) fn is_active(&self) -> bool {
        self.state.lock().is_ok_and(|state| !state.stopped)
    }

    /// Linearize a short dispatch with stop. Never include socket/storage I/O,
    /// persistence work or approval waits; callers catch protocol panics inside.
    pub(crate) fn run_if_active<T>(&self, action: impl FnOnce() -> T) -> Option<T> {
        let state = self.state.lock().ok()?;
        if state.stopped {
            return None;
        }
        Some(action())
    }

    fn register_socket(
        self: &Arc<Self>,
        stream: &TcpStream,
    ) -> io::Result<Option<LifetimeRegistration>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("listener lifetime lock poisoned"))?;
        if state.stopped {
            return Ok(None);
        }
        let socket = stream.try_clone()?;
        let id = state.next_id;
        state.next_id = state.next_id.wrapping_add(1);
        state.sockets.insert(id, socket);
        Ok(Some(LifetimeRegistration {
            owner: Arc::clone(self),
            id,
            kind: RegistrationKind::Socket,
        }))
    }

    pub(crate) fn watch_approval(
        self: &Arc<Self>,
        sender: mpsc::Sender<bool>,
    ) -> Option<LifetimeRegistration> {
        let mut state = self.state.lock().ok()?;
        if state.stopped {
            return None;
        }
        let id = state.next_id;
        state.next_id = state.next_id.wrapping_add(1);
        state.approvals.insert(id, sender);
        Some(LifetimeRegistration {
            owner: Arc::clone(self),
            id,
            kind: RegistrationKind::Approval,
        })
    }

    pub(crate) fn stop(&self) {
        // This state controls only cancellation/resources, not vault data.
        // Poison must not prevent releasing registered sockets and waiters.
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.stopped = true;
        for (_, socket) in state.sockets.drain() {
            let _ = socket.shutdown(Shutdown::Both);
        }
        for (_, sender) in state.approvals.drain() {
            let _ = sender.send(false);
        }
    }
}

struct StopOnExit(Arc<ListenerLifetime>);
impl Drop for StopOnExit {
    fn drop(&mut self) {
        self.0.stop();
    }
}

/// Shared accept boundary: restore blocking mode and admit before dispatching.
/// Exiting closes registered sockets/wakes approvals. Already admitted storage
/// jobs can finish and retain their worker permit until they return.
pub(crate) fn accept_loop(
    listener: &TcpListener,
    stop: &mpsc::Receiver<()>,
    budget: Arc<ConnectionBudget>,
    lifetime: Arc<ListenerLifetime>,
    handler: impl Fn(CancelableStream, SocketAddr, Arc<ListenerLifetime>) + Send + Sync + 'static,
) {
    let _stop_on_exit = StopOnExit(Arc::clone(&lifetime));
    let handler = Arc::new(handler);
    while lifetime.is_active() && matches!(stop.try_recv(), Err(mpsc::TryRecvError::Empty)) {
        match listener.accept() {
            Ok((stream, addr)) => {
                if stream.set_nonblocking(false).is_err() {
                    continue;
                }
                let registration = match lifetime.register_socket(&stream) {
                    Ok(Some(registration)) => registration,
                    _ => continue,
                };
                let handler = Arc::clone(&handler);
                let lifetime = Arc::clone(&lifetime);
                if let Err(error) = budget.try_spawn(move || {
                    let _registration = registration;
                    if lifetime.is_active() {
                        let Ok(stream) = CancelableStream::new(stream, Arc::clone(&lifetime))
                        else {
                            return;
                        };
                        handler(stream, addr, lifetime);
                    }
                }) {
                    eprintln!("[browser] connection worker unavailable: {error}");
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::time::Instant;

    fn start_test_listener(
        budget: Arc<ConnectionBudget>,
        started: mpsc::Sender<()>,
    ) -> (SocketAddr, mpsc::Sender<()>, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop_tx, stop_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            accept_loop(
                &listener,
                &stop_rx,
                budget,
                Arc::new(ListenerLifetime::default()),
                move |mut stream, _, _| {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(10)))
                        .unwrap();
                    let _ = started.send(());
                    let mut byte = [0];
                    if stream.read_exact(&mut byte).is_ok() {
                        let _ = stream.write_all(&byte);
                    }
                },
            );
        });
        (addr, stop_tx, worker)
    }

    fn wait_until_idle(budget: &ConnectionBudget) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while budget.active.load(Ordering::Acquire) != 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(budget.active.load(Ordering::Acquire), 0);
    }

    #[test]
    fn handler_unwind_releases_the_connection_permit() {
        let budget = Arc::new(ConnectionBudget::default());
        let worker = budget
            .try_spawn(|| panic!("synthetic handler unwind"))
            .unwrap()
            .unwrap();
        assert!(worker.join().is_err());
        assert_eq!(budget.active.load(Ordering::Acquire), 0);
        budget.try_spawn(|| {}).unwrap().unwrap().join().unwrap();
    }

    #[test]
    fn listener_stop_releases_sockets_and_restart_can_serve_again() {
        let budget = Arc::new(ConnectionBudget::default());
        let (started_tx, started_rx) = mpsc::channel();
        let (addr, stop, listener) = start_test_listener(Arc::clone(&budget), started_tx.clone());
        let clients: Vec<_> = (0..MAX_CONNECTION_WORKERS)
            .map(|_| TcpStream::connect(addr).unwrap())
            .collect();
        for _ in 0..MAX_CONNECTION_WORKERS {
            started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        stop.send(()).unwrap();
        listener.join().unwrap();
        wait_until_idle(&budget);

        let (addr, stop, listener) = start_test_listener(Arc::clone(&budget), started_tx);
        drop(clients);
        wait_until_idle(&budget);

        let mut recovered = TcpStream::connect(addr).unwrap();
        recovered
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        recovered.write_all(b"x").unwrap();
        let mut reply = [0];
        recovered.read_exact(&mut reply).unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(recovered);
        stop.send(()).unwrap();
        listener.join().unwrap();
        wait_until_idle(&budget);
        assert_eq!(reply, *b"x");
    }

    #[test]
    fn stopped_listener_cannot_serve_an_existing_connection() {
        let budget = Arc::new(ConnectionBudget::default());
        let (started_tx, started_rx) = mpsc::channel();
        let (addr, stop, listener) = start_test_listener(Arc::clone(&budget), started_tx);
        let mut client = TcpStream::connect(addr).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        stop.send(()).unwrap();
        listener.join().unwrap();
        let write = client.write_all(b"x");
        let mut byte = [0];
        let reply = client.read(&mut byte);
        drop(client);
        wait_until_idle(&budget);
        eprintln!("request after listener stop: write={write:?}, read={reply:?}, byte={byte:?}");
        assert!(
            !matches!(reply, Ok(1)),
            "a stopped service processed a new request on its old socket"
        );
    }

    #[test]
    fn dropped_stop_sender_terminates_the_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let (stop_tx, stop_rx) = mpsc::channel();
        drop(stop_tx);
        accept_loop(
            &listener,
            &stop_rx,
            Arc::new(ConnectionBudget::default()),
            Arc::new(ListenerLifetime::default()),
            |_, _, _| {
                panic!("a stopped listener must not dispatch");
            },
        );
    }

    #[test]
    fn admitted_work_keeps_its_permit_until_completion_after_stop() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let budget = Arc::new(ConnectionBudget::default());
        let worker_budget = Arc::clone(&budget);
        let (stop_tx, stop_rx) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let finish_rx = Mutex::new(finish_rx);
        let worker = std::thread::spawn(move || {
            accept_loop(
                &listener,
                &stop_rx,
                worker_budget,
                Arc::new(ListenerLifetime::default()),
                move |mut stream, _, _| {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    stream.read_exact(&mut [0]).unwrap();
                    started_tx.send(()).unwrap();
                    // Model work already admitted before stop, such as a
                    // durable write that must finish/adopt its result.
                    finish_rx
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(3))
                        .unwrap();
                },
            );
        });
        let mut peer = TcpStream::connect(addr).unwrap();
        peer.write_all(b"x").unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        stop_tx.send(()).unwrap();
        worker.join().unwrap();
        assert_eq!(budget.active.load(Ordering::Acquire), 1);
        finish_tx.send(()).unwrap();
        wait_until_idle(&budget);
    }
}
