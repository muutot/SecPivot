//! Bounded connection workers for the opt-in browser integration servers.
//! Admission happens before spawning, including incomplete HTTP/WS handshakes.
//! The budget belongs to managed service state and survives listener restarts.

use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::Duration;

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

/// Shared accept boundary: restore blocking mode and admit before dispatching.
/// Stop joins only the listener; in-flight workers retain their budget permits.
pub(crate) fn accept_loop(
    listener: &TcpListener,
    stop: &mpsc::Receiver<()>,
    budget: Arc<ConnectionBudget>,
    handler: impl Fn(TcpStream, SocketAddr) + Send + Sync + 'static,
) {
    let handler = Arc::new(handler);
    while matches!(stop.try_recv(), Err(mpsc::TryRecvError::Empty)) {
        match listener.accept() {
            Ok((stream, addr)) => {
                if stream.set_nonblocking(false).is_err() {
                    continue;
                }
                let handler = Arc::clone(&handler);
                if let Err(error) = budget.try_spawn(move || handler(stream, addr)) {
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
            accept_loop(&listener, &stop_rx, budget, move |mut stream, _| {
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let _ = started.send(());
                let mut byte = [0];
                if stream.read_exact(&mut byte).is_ok() {
                    let _ = stream.write_all(&byte);
                }
            });
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
    fn listener_stop_and_restart_share_in_flight_capacity() {
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
        assert_eq!(
            budget.active.load(Ordering::Acquire),
            MAX_CONNECTION_WORKERS
        );

        let (addr, stop, listener) = start_test_listener(Arc::clone(&budget), started_tx);
        let mut refused = TcpStream::connect(addr).unwrap();
        refused
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let rejection = refused.read(&mut [0]);
        let extra_started = started_rx.try_recv().is_ok();
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
        assert!(
            matches!(rejection, Ok(0))
                || rejection.is_err_and(|error| { error.kind() == io::ErrorKind::ConnectionReset }),
            "a restart must reject connections while old workers occupy the cap"
        );
        assert!(!extra_started);
        assert_eq!(reply, *b"x");
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
            |_, _| {
                panic!("a stopped listener must not dispatch");
            },
        );
    }
}
