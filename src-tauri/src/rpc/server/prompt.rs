//! A connection owns its visible SRP attempt, but never its plaintext password.

pub(super) struct PromptLease {
    request_id: String,
    on_end: Box<dyn Fn(&str) + Send>,
}

impl PromptLease {
    pub(super) fn new(request_id: String, on_end: impl Fn(&str) + Send + 'static) -> Self {
        Self {
            request_id,
            on_end: Box::new(on_end),
        }
    }
}

impl Drop for PromptLease {
    fn drop(&mut self) {
        (self.on_end)(&self.request_id);
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
