//! Native binary import path: no plaintext archive/rows/attachments in IPC.
use super::with_vault_session;
use crate::vault::{self, OnePuxCounts, OnePuxImport, VaultSession, VaultSessions, VaultState};
use serde::Serialize;
use std::{path::Path, sync::Mutex};
use zeroize::Zeroizing;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnePuxImportResult {
    state: VaultState,
    counts: OnePuxCounts,
}

fn validate_path(path: &Path, file_name: Option<&str>) -> Result<(), String> {
    let name = if vault::file_io::is_document_uri(path) {
        Path::new(file_name.ok_or("无法确定文档类型，请重新选择 1PUX 文件")?)
    } else {
        path
    };
    if !name
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("1pux"))
    {
        return Err("仅支持导入 .1pux 文件".into());
    }
    Ok(())
}

fn prepare_file(path: &Path, file_name: Option<&str>) -> Result<OnePuxImport, String> {
    validate_path(path, file_name)?;
    let bytes = Zeroizing::new(
        vault::file_io::read_with_limit(path, "1PUX 文件", 64 * 1024 * 1024)
            .map_err(|_| "无法读取 1PUX 文件或文件超过 64 MiB 上限")?,
    );
    vault::prepare_1pux_import(&bytes)
}

fn complete(
    vaults: &VaultSessions,
    session: &Mutex<VaultSession>,
    session_id: &str,
    target: &str,
    plan: OnePuxImport,
) -> Result<OnePuxImportResult, String> {
    let counts = plan.counts();
    let state = with_vault_session(vaults, session, Some(session_id), |session| {
        session.import_1pux(plan, target)
    })?;
    Ok(OnePuxImportResult { state, counts })
}

/// Require a stable originating session, check it before file I/O, then resolve
/// it again on completion. A tab switch never redirects import to the new tab.
#[tauri::command]
pub(crate) async fn import_1pux_file(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: String,
    path: String,
    file_name: Option<String>,
    target_group_uuid: Option<String>,
) -> Result<OnePuxImportResult, String> {
    import_for_session(
        vaults.inner(),
        session.inner(),
        session_id,
        path,
        file_name,
        target_group_uuid,
    )
    .await
}

async fn import_for_session(
    vaults: &VaultSessions,
    session: &Mutex<VaultSession>,
    session_id: String,
    path: String,
    file_name: Option<String>,
    target_group_uuid: Option<String>,
) -> Result<OnePuxImportResult, String> {
    let target = target_group_uuid.unwrap_or_else(|| vault::ROOT_GROUP_UUID.to_owned());
    with_vault_session(vaults, session, Some(&session_id), |session| {
        session.validate_1pux_target(&target)
    })?;
    let plan = tauri::async_runtime::spawn_blocking(move || {
        prepare_file(Path::new(&path), file_name.as_deref())
    })
    .await
    .map_err(|_| "1PUX 导入任务异常；未导入任何内容")??;
    complete(vaults, session, &session_id, &target, plan)
}

#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;
