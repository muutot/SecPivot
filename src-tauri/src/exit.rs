//! Preserve the main window until native save/discard/cancel finishes.

use crate::{
    config::ConfigStore,
    vault::{VaultSession, VaultSessions},
};
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Mutex,
};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

#[derive(Default)]
pub(crate) struct ExitGate(AtomicU8);
impl ExitGate {
    fn begin(&self) -> bool {
        self.0
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    fn approve(&self) {
        self.0.store(2, Ordering::Release);
    }
    fn consume_approval(&self) -> bool {
        self.0
            .compare_exchange(2, 0, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    fn reset(&self) {
        self.0.store(0, Ordering::Release);
    }
}

struct Attempt {
    app: tauri::AppHandle,
    approved: bool,
}
impl Drop for Attempt {
    fn drop(&mut self) {
        if !self.approved {
            self.app.state::<ExitGate>().reset();
        }
    }
}

pub(crate) fn handle_request(
    app: &tauri::AppHandle,
    api: &tauri::ExitRequestApi,
    code: Option<i32>,
) {
    let gate = app.state::<ExitGate>();
    if gate.consume_approval() {
        // An open may finish after the close transaction. Such a new session
        // must pass the same close gate instead of exiting under its feet.
        if let Ok(_active) = app.state::<Mutex<VaultSession>>().lock() {
            if app
                .state::<VaultSessions>()
                .close_session_ids()
                .is_ok_and(|ids| ids.is_empty())
            {
                return;
            }
        }
    }
    api.prevent_exit();
    if !gate.begin() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut attempt = Attempt {
            app: app.clone(),
            approved: false,
        };
        match crate::commands::vault_close::close_workspace(&app, false).await {
            Ok(()) => {
                app.state::<ExitGate>().approve();
                attempt.approved = true;
                app.exit(code.unwrap_or(0));
            }
            Err(error) => {
                // Also refresh a renderer that survived an earlier successful
                // save in a multi-database close whose later save failed.
                let _ = app.emit("browser-vault-changed", ());
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
                if error.starts_with(crate::vault::close::CANCELLED)
                    || error.starts_with(crate::vault::close::BUSY)
                {
                    return;
                }
                let english = app
                    .state::<ConfigStore>()
                    .get()
                    .is_ok_and(|cfg| cfg.general.language == "en");
                let message = if error.starts_with(crate::vault::close::CHANGED) {
                    if english {
                        "Databases changed while closing. Sessions remain open; review your changes and try again.".into()
                    } else {
                        "关闭期间数据库发生变化，已保留会话，请检查修改后重试。".into()
                    }
                } else if english {
                    format!("Could not exit; databases remain open.\n{error}")
                } else {
                    format!("无法退出，数据库仍保持打开。\n{error}")
                };
                let activity = crate::commands::vault_close::DialogActivity::new(app.clone());
                let mut dialog = app
                    .dialog()
                    .message(message)
                    .title("SecPivot")
                    .kind(MessageDialogKind::Error);
                if let Some(window) = app.get_webview_window("main") {
                    dialog = dialog.parent(&window);
                }
                let (send, receive) = tokio::sync::oneshot::channel();
                dialog.show_with_result(move |_| {
                    let _ = send.send(());
                });
                let _ = receive.await;
                drop(activity);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_requests_wait_and_cancel_or_failure_allows_retry() {
        let gate = ExitGate::default();
        assert!(!gate.consume_approval());
        for _ in 0..3 {
            assert!(gate.begin());
            assert!(!gate.begin());
            assert!(!gate.consume_approval());
            gate.reset();
        }
        assert!(gate.begin());
        gate.approve();
        assert!(!gate.begin());
        assert!(gate.consume_approval());
        assert!(!gate.consume_approval());
        assert!(gate.begin());
    }
}
