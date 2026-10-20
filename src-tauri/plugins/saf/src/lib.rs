//! Android Storage Access Framework picker with persistable URI grants.
//!
//! `tauri-plugin-dialog` opens documents with `ACTION_GET_CONTENT`, which grants
//! read access only and cannot be persisted. SecPivot needs both: a recent file
//! must reopen after a restart, and Save As must be able to write the picked
//! document. This plugin uses `ACTION_OPEN_DOCUMENT` / `ACTION_CREATE_DOCUMENT`
//! plus `takePersistableUriPermission` and returns the provider's display name.

use serde::Serialize;
use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Runtime,
};

#[cfg(target_os = "android")]
use serde::Deserialize;
#[cfg(target_os = "android")]
use tauri::{plugin::PluginHandle, Manager};

#[cfg(target_os = "android")]
#[derive(Debug, Deserialize)]
struct PickedDocument {
    #[serde(default)]
    uri: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafDocument {
    pub uri: String,
    pub name: Option<String>,
}

#[cfg(target_os = "android")]
struct Saf<R: Runtime>(PluginHandle<R>);

#[tauri::command]
async fn open_document<R: Runtime>(
    app: AppHandle<R>,
    mime_types: Option<Vec<String>>,
) -> Result<Option<SafDocument>, String> {
    pick(
        app,
        "openDocument",
        serde_json::json!({ "mimeTypes": mime_types }),
    )
    .await
}

#[tauri::command]
async fn create_document<R: Runtime>(
    app: AppHandle<R>,
    file_name: String,
    mime_types: Option<Vec<String>>,
) -> Result<Option<SafDocument>, String> {
    pick(
        app,
        "createDocument",
        serde_json::json!({ "fileName": file_name, "mimeTypes": mime_types }),
    )
    .await
}

async fn pick<R: Runtime>(
    app: AppHandle<R>,
    command: &'static str,
    args: serde_json::Value,
) -> Result<Option<SafDocument>, String> {
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, command, args);
        Err("当前平台不支持系统文件选择器".into())
    }

    #[cfg(target_os = "android")]
    {
        let handle = app.state::<Saf<R>>().0.clone();
        let picked = handle
            .run_mobile_plugin_async::<PickedDocument>(command, args)
            .await
            .map_err(|e| e.to_string())?;
        Ok(picked.uri.map(|uri| SafDocument {
            uri,
            name: picked.name,
        }))
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("saf")
        .invoke_handler(tauri::generate_handler![open_document, create_document])
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            {
                let handle = api.register_android_plugin("com.secpivot.saf", "SafPlugin")?;
                app.manage(Saf(handle));
            }
            #[cfg(not(target_os = "android"))]
            let _ = (app, api);
            Ok(())
        })
        .build()
}
