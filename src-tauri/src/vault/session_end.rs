//! Nonsecret one-shot observers of the backend-active vault lifetime.
//! Callers hold the active vault mutex before registering or ending observers.

use std::sync::{Arc, Mutex, Weak};

pub(crate) struct SessionEndSignal {
    callback: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl SessionEndSignal {
    pub(crate) fn new(callback: impl FnOnce() + Send + 'static) -> Arc<Self> {
        Arc::new(Self {
            callback: Mutex::new(Some(Box::new(callback))),
        })
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.callback
            .lock()
            .is_ok_and(|callback| callback.is_some())
    }

    pub(crate) fn finish(&self) {
        let callback = self
            .callback
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(callback) = callback {
            // No observer, registry or lifetime guard lives across the callback.
            // Notification failure must not poison the vault's active mutex.
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback)).is_err() {
                eprintln!("[vault] active-session end notification panicked");
            }
        }
    }
}

impl Drop for SessionEndSignal {
    fn drop(&mut self) {
        self.finish();
    }
}

#[derive(Default)]
pub(super) struct SessionEndSignals {
    watchers: Mutex<Vec<Weak<SessionEndSignal>>>,
}

impl SessionEndSignals {
    pub(super) fn watch(&self, callback: impl FnOnce() + Send + 'static) -> Arc<SessionEndSignal> {
        let signal = SessionEndSignal::new(callback);
        let mut watchers = self
            .watchers
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        watchers.retain(|watcher| watcher.strong_count() != 0);
        watchers.push(Arc::downgrade(&signal));
        signal
    }

    pub(super) fn finish(&self) {
        let watchers = std::mem::take(
            &mut *self
                .watchers
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
        );
        for watcher in watchers {
            if let Some(signal) = watcher.upgrade() {
                signal.finish();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn finished_or_dropped_observers_do_not_accumulate_owned_callbacks() {
        let signals = SessionEndSignals::default();
        let finished = Arc::new(AtomicUsize::new(0));
        for _ in 0..1000 {
            let count = Arc::clone(&finished);
            let signal = signals.watch(move || {
                count.fetch_add(1, Ordering::Relaxed);
            });
            signal.finish();
            drop(signal);
        }
        assert_eq!(finished.load(Ordering::Relaxed), 1000);
        assert_eq!(signals.watchers.lock().unwrap().len(), 1);
        signals.finish();
        assert!(signals.watchers.lock().unwrap().is_empty());
        assert_eq!(finished.load(Ordering::Relaxed), 1000);
    }
}
