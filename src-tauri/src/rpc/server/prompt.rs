//! A connection owns its visible SRP attempt, but never its plaintext password.

use crate::vault::{SessionEndSignal, VaultSessions};
use std::sync::Arc;

pub(super) struct PromptLease(Arc<SessionEndSignal>);

impl PromptLease {
    #[cfg(test)]
    pub(super) fn from_signal(signal: Arc<SessionEndSignal>) -> Self {
        Self(signal)
    }
    #[cfg(test)]
    pub(super) fn new(request_id: String, on_end: impl Fn(&str) + Send + 'static) -> Self {
        Self(SessionEndSignal::new(move || on_end(&request_id)))
    }

    pub(super) fn watch(
        vaults: &VaultSessions,
        request_id: String,
        on_end: impl Fn(&str) + Send + 'static,
    ) -> Self {
        Self(vaults.watch_active_end(move || on_end(&request_id)))
    }

    pub(super) fn is_active(&self) -> bool {
        self.0.is_pending()
    }
}

impl Drop for PromptLease {
    fn drop(&mut self) {
        self.0.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::server::Conn;
    use std::sync::{Arc, Mutex};

    fn lease(id: &str, ended: &Arc<Mutex<Vec<String>>>) -> PromptLease {
        let ended = Arc::clone(ended);
        PromptLease::new(id.to_owned(), move |id| {
            ended.lock().unwrap().push(id.into())
        })
    }

    #[test]
    fn connection_exit_and_unwind_end_the_owned_prompt_once() {
        let ended = Arc::new(Mutex::new(Vec::new()));
        {
            let mut conn = Conn::default();
            conn.prompt = Some(lease("one", &ended));
        }
        let result = std::panic::catch_unwind({
            let ended = Arc::clone(&ended);
            move || {
                let mut conn = Conn::default();
                conn.prompt = Some(lease("two", &ended));
                panic!("synthetic connection panic");
            }
        });
        assert!(result.is_err());
        assert_eq!(*ended.lock().unwrap(), ["one", "two"]);
    }
}
