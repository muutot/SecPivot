//! Native close prompt and workspace adapter; no renderer is required to save
//! safely on tray quit or window close.

use crate::{
    config::ConfigStore,
    vault::{
        close::{CloseDecision, ClosePrompt, CloseTarget},
        VaultSession, VaultSessions,
    },
};
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::{
    DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult,
};

pub(crate) async fn prompt(
    app: tauri::AppHandle,
    request: ClosePrompt,
) -> Result<CloseDecision, String> {
    let _activity = DialogActivity::new(app.clone());
    let english = app.state::<ConfigStore>().get()?.general.language == "en";
    let (save, discard, cancel, text) = if english {
        (
            "Save",
            "Discard",
            "Cancel",
            format!("Save changes to “{}” before closing?", request.file_name),
        )
    } else {
        (
            "保存",
            "放弃",
            "取消",
            format!("关闭“{}”前是否保存修改？", request.file_name),
        )
    };
    #[allow(unused_mut)] // Desktop sets the parent; mobile has no parent API.
    let mut dialog = app
        .dialog()
        .message(text)
        .title("SecPivot")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNoCancelCustom(
            save.into(),
            discard.into(),
            cancel.into(),
        ));
    #[cfg(desktop)]
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        dialog = dialog.parent(&window);
    }
    let (send, receive) = tokio::sync::oneshot::channel();
    dialog.show_with_result(move |result| {
        let _ = send.send(result);
    });
    Ok(decision(
        receive.await.unwrap_or(MessageDialogResult::Cancel),
        save,
        discard,
    ))
}

/// Native dialogs borrow focus; they must not trigger focus-loss locking.
/// The lease can move into a dialog callback and contains no vault data.
pub(crate) struct DialogActivity(tauri::AppHandle);
impl DialogActivity {
    pub(crate) fn new(app: tauri::AppHandle) -> Self {
        let _ = app.emit("vault-close-dialog", true);
        Self(app)
    }
}
impl Drop for DialogActivity {
    fn drop(&mut self) {
        let _ = self.0.emit("vault-close-dialog", false);
    }
}

fn decision(result: MessageDialogResult, save: &str, discard: &str) -> CloseDecision {
    match result {
        MessageDialogResult::Yes => CloseDecision::Save,
        MessageDialogResult::No => CloseDecision::Discard,
        MessageDialogResult::Custom(label) if label == save => CloseDecision::Save,
        MessageDialogResult::Custom(label) if label == discard => CloseDecision::Discard,
        _ => CloseDecision::Cancel,
    }
}

pub(crate) async fn close_workspace(app: &tauri::AppHandle, keep_rpc: bool) -> Result<(), String> {
    #[cfg(desktop)]
    crate::platform::autotype::cancel_replay();
    let config = app.state::<ConfigStore>().get()?;
    crate::vault::close::close_with_save(
        &app.state::<VaultSessions>(),
        &app.state::<Mutex<VaultSession>>(),
        CloseTarget::All { keep_rpc },
        config.security.auto_save_on_close,
        |request| prompt(app.clone(), request),
    )
    .await?;
    super::vault::cleanup_closed_workspace(app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_results_require_explicit_save_or_discard_and_default_to_cancel() {
        for (save, discard) in [("Save", "Discard"), ("保存", "放弃")] {
            assert!(decision(MessageDialogResult::Yes, save, discard) == CloseDecision::Save);
            assert!(
                decision(MessageDialogResult::Custom(save.into()), save, discard)
                    == CloseDecision::Save
            );
            assert!(decision(MessageDialogResult::No, save, discard) == CloseDecision::Discard);
            assert!(
                decision(MessageDialogResult::Custom(discard.into()), save, discard)
                    == CloseDecision::Discard
            );
            for result in [
                MessageDialogResult::Cancel,
                MessageDialogResult::Ok,
                MessageDialogResult::Custom("unknown".into()),
            ] {
                assert!(decision(result, save, discard) == CloseDecision::Cancel);
            }
        }
    }
}
