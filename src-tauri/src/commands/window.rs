//! Explicit main-window actions. Exit always goes through the native close gate.

use tauri::Manager;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum WindowAction {
    Minimize,
    MinimizeToTray,
    Exit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_action_wire_values_are_explicit_and_reject_other_actions() {
        assert!(matches!(
            serde_json::from_str::<WindowAction>(r#""minimize""#).unwrap(),
            WindowAction::Minimize
        ));
        assert!(matches!(
            serde_json::from_str::<WindowAction>(r#""minimizeToTray""#).unwrap(),
            WindowAction::MinimizeToTray
        ));
        assert!(matches!(
            serde_json::from_str::<WindowAction>(r#""exit""#).unwrap(),
            WindowAction::Exit
        ));
        for value in [
            r#""none""#,
            r#""lock""#,
            r#""MinimizeToTray""#,
            r#""unknown""#,
            "null",
            "1",
            "{}",
        ] {
            assert!(serde_json::from_str::<WindowAction>(value).is_err());
        }
    }
}

#[tauri::command]
pub(crate) fn perform_window_action(
    window: tauri::WebviewWindow,
    action: WindowAction,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("WINDOW_ACTION_UNAVAILABLE".into());
    }
    match action {
        WindowAction::Minimize => window.minimize().map_err(|e| e.to_string()),
        WindowAction::MinimizeToTray => crate::minimize_main_to_tray(window.app_handle()),
        WindowAction::Exit => {
            // app.exit emits ExitRequested; the existing save/discard/cancel
            // transaction keeps the window alive until closure is approved.
            window.app_handle().exit(0);
            Ok(())
        }
    }
}
