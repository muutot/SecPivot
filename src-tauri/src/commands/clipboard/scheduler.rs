//! One worker and one pending deadline; replacement/cancel drops the old
//! payload immediately instead of leaving one sleeping thread per copy.

use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

struct Pending<T> {
    deadline: Instant,
    generation: u64,
    payload: T,
}

struct State<T> {
    pending: Option<Pending<T>>,
    generation: u64,
    stopped: bool,
}

pub(super) struct LatestDeadline<T> {
    state: Mutex<State<T>>,
    wake: Condvar,
}

impl<T: Send + 'static> LatestDeadline<T> {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                pending: None,
                generation: 0,
                stopped: false,
            }),
            wake: Condvar::new(),
        })
    }

    pub(super) fn start_worker(
        self: &Arc<Self>,
        callback: impl Fn(&Self, u64, T) + Send + 'static,
    ) -> Result<JoinHandle<()>, String> {
        let worker = Arc::clone(self);
        std::thread::Builder::new()
            .name("clipboard-wipe".into())
            .spawn(move || worker.run(callback))
            .map_err(|error| format!("无法创建剪贴板清除任务: {error}"))
    }

    /// Reserve before fingerprinting, clipboard reads or worker initialization.
    /// A newer reservation/cancel invalidates both pending and preparing work.
    pub(super) fn reserve(&self) -> Result<u64, String> {
        let mut state = self.state.lock().map_err(|_| "剪贴板任务锁已损坏")?;
        if state.stopped {
            return Err("剪贴板清除任务已停止".to_owned());
        }
        state.generation = state.generation.wrapping_add(1);
        state.pending = None;
        self.wake.notify_one();
        Ok(state.generation)
    }

    /// Install only while the originating request still owns its reservation.
    pub(super) fn schedule_reserved(
        &self,
        generation: u64,
        payload: T,
        delay: Duration,
    ) -> Result<bool, String> {
        let deadline = Instant::now()
            .checked_add(delay)
            .ok_or_else(|| "剪贴板清除时间超出支持范围".to_owned())?;
        let mut state = self.state.lock().map_err(|_| "剪贴板任务锁已损坏")?;
        if state.stopped {
            return Err("剪贴板清除任务已停止".to_owned());
        }
        if state.generation != generation {
            return Ok(false);
        }
        state.pending = Some(Pending {
            deadline,
            generation,
            payload,
        });
        self.wake.notify_one();
        Ok(true)
    }

    pub(super) fn cancel(&self) -> Result<(), String> {
        self.reserve().map(|_| ())
    }

    pub(super) fn is_current(&self, generation: u64) -> bool {
        self.state
            .lock()
            .is_ok_and(|state| !state.stopped && state.generation == generation)
    }

    fn run(&self, callback: impl Fn(&Self, u64, T)) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        loop {
            if state.stopped {
                return;
            }
            if let Some(pending) = state.pending.as_ref() {
                let delay = pending.deadline.saturating_duration_since(Instant::now());
                if delay.is_zero() {
                    let pending = state.pending.take().expect("checked pending task");
                    // Clipboard I/O must never hold the scheduling mutex.
                    drop(state);
                    callback(self, pending.generation, pending.payload);
                    let Ok(next) = self.state.lock() else {
                        return;
                    };
                    state = next;
                    continue;
                }
                let Ok((next, _)) = self.wake.wait_timeout(state, delay) else {
                    return;
                };
                state = next;
            } else {
                let Ok(next) = self.wake.wait(state) else {
                    return;
                };
                state = next;
            }
        }
    }

    #[cfg(test)]
    fn start(
        callback: impl Fn(&Self, u64, T) + Send + 'static,
    ) -> Result<(Arc<Self>, JoinHandle<()>), String> {
        let scheduler = Self::new();
        let thread = scheduler.start_worker(callback)?;
        Ok((scheduler, thread))
    }

    #[cfg(test)]
    fn schedule(&self, payload: T, delay: Duration) -> Result<bool, String> {
        self.schedule_reserved(self.reserve()?, payload, delay)
    }

    #[cfg(test)]
    fn stop(&self) {
        let mut state = self.state.lock().unwrap();
        state.stopped = true;
        state.pending = None;
        self.wake.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    struct DropProbe(Arc<AtomicUsize>);
    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn a_thousand_replacements_and_cancel_release_pending_payloads_immediately() {
        let released = Arc::new(AtomicUsize::new(0));
        let (fired_tx, fired_rx) = mpsc::channel();
        let (scheduler, thread) = LatestDeadline::start(move |_, _, _: DropProbe| {
            fired_tx.send(()).unwrap();
        })
        .unwrap();
        for _ in 0..1_000 {
            scheduler
                .schedule(DropProbe(Arc::clone(&released)), Duration::from_secs(3_600))
                .unwrap();
        }
        let after_replacement = released.load(Ordering::SeqCst);
        scheduler.cancel().unwrap();
        let after_cancel = released.load(Ordering::SeqCst);
        scheduler.stop();
        thread.join().unwrap();
        assert_eq!(after_replacement, 999);
        assert_eq!(after_cancel, 1_000);
        assert!(fired_rx.try_recv().is_err());
    }

    #[test]
    fn a_replacement_wakes_the_worker_and_uses_its_own_deadline() {
        let (fired_tx, fired_rx) = mpsc::channel();
        let (scheduler, thread) = LatestDeadline::start(move |scheduler, generation, id| {
            fired_tx
                .send((id, scheduler.is_current(generation)))
                .unwrap();
        })
        .unwrap();
        scheduler.schedule(1, Duration::from_secs(3_600)).unwrap();
        scheduler.schedule(2, Duration::ZERO).unwrap();
        let fired = fired_rx.recv_timeout(Duration::from_secs(2));
        scheduler.stop();
        thread.join().unwrap();
        assert_eq!(fired.unwrap(), (2, true));
        assert!(fired_rx.try_recv().is_err());
    }

    #[test]
    fn a_task_runs_after_its_deadline_without_another_schedule() {
        let (fired_tx, fired_rx) = mpsc::channel();
        let (scheduler, thread) = LatestDeadline::start(move |_, _, ()| {
            fired_tx.send(Instant::now()).unwrap();
        })
        .unwrap();
        let started = Instant::now();
        let delay = Duration::from_millis(50);
        scheduler.schedule((), delay).unwrap();
        let fired = fired_rx.recv_timeout(Duration::from_secs(2));
        scheduler.stop();
        thread.join().unwrap();
        assert!(fired.unwrap().duration_since(started) >= delay);
    }

    #[test]
    fn an_in_flight_callback_rechecks_generation_after_replacement() {
        let (started_tx, started_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let (fired_tx, fired_rx) = mpsc::channel();
        let (scheduler, thread) = LatestDeadline::start(move |scheduler, generation, id| {
            started_tx.send(id).unwrap();
            if resume_rx.recv_timeout(Duration::from_secs(2)).is_ok() {
                fired_tx
                    .send((id, scheduler.is_current(generation)))
                    .unwrap();
            }
        })
        .unwrap();
        scheduler.schedule(1, Duration::ZERO).unwrap();
        assert_eq!(started_rx.recv_timeout(Duration::from_secs(2)).unwrap(), 1);
        scheduler.schedule(2, Duration::ZERO).unwrap();
        resume_tx.send(()).unwrap();
        let first = fired_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(started_rx.recv_timeout(Duration::from_secs(2)).unwrap(), 2);
        resume_tx.send(()).unwrap();
        let second = fired_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        scheduler.stop();
        thread.join().unwrap();
        assert_eq!(first, (1, false));
        assert_eq!(second, (2, true));
    }

    #[test]
    fn cancellation_during_request_preparation_rejects_the_old_schedule() {
        let (fired_tx, fired_rx) = mpsc::channel();
        let (scheduler, thread) = LatestDeadline::start(move |_, _, id| {
            fired_tx.send(id).unwrap();
        })
        .unwrap();
        // The old command is preparing its fingerprint/worker when lock
        // cancellation runs. Its eventual installation must stay invalid.
        let reservation = scheduler.reserve().unwrap();
        scheduler.cancel().unwrap();
        let installed = scheduler
            .schedule_reserved(reservation, 1, Duration::ZERO)
            .unwrap();
        let fired = fired_rx.recv_timeout(Duration::from_millis(100));
        scheduler.stop();
        thread.join().unwrap();
        assert!(!installed);
        assert!(fired.is_err(), "a cancelled request installed a new wipe");
    }

    #[test]
    fn cancellation_before_first_worker_start_still_rejects_old_work() {
        let scheduler = LatestDeadline::new();
        let reservation = scheduler.reserve().unwrap();
        // No worker exists at the cancellation boundary.
        scheduler.cancel().unwrap();
        let (fired_tx, fired_rx) = mpsc::channel();
        let thread = scheduler
            .start_worker(move |_, _, id| fired_tx.send(id).unwrap())
            .unwrap();
        let installed = scheduler
            .schedule_reserved(reservation, 1, Duration::ZERO)
            .unwrap();
        let fired = fired_rx.recv_timeout(Duration::from_millis(100));
        scheduler.stop();
        thread.join().unwrap();
        assert!(!installed);
        assert!(fired.is_err());
    }

    #[test]
    fn preparation_finishing_out_of_order_preserves_the_newest_request() {
        let released = Arc::new(AtomicUsize::new(0));
        let scheduler = LatestDeadline::new();
        let old = scheduler.reserve().unwrap();
        let current = scheduler.reserve().unwrap();
        assert!(scheduler
            .schedule_reserved(
                current,
                DropProbe(Arc::clone(&released)),
                Duration::from_secs(3_600)
            )
            .unwrap());
        assert!(!scheduler
            .schedule_reserved(old, DropProbe(Arc::clone(&released)), Duration::ZERO)
            .unwrap());
        assert_eq!(released.load(Ordering::SeqCst), 1);
        let pending = scheduler.state.lock().unwrap();
        assert_eq!(pending.pending.as_ref().unwrap().generation, current);
        drop(pending);
        scheduler.cancel().unwrap();
        assert_eq!(released.load(Ordering::SeqCst), 2);
    }
}
