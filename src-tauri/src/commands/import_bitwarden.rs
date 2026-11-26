//! Native Bitwarden JSON selection carries a path/display name, never plaintext through IPC.
use super::with_vault_session;
use crate::vault::{
    self, BitwardenCounts, BitwardenImport, VaultSession, VaultSessions, VaultState,
};
use serde::Serialize;
use std::{
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BitwardenImportResult {
    state: VaultState,
    counts: BitwardenCounts,
}

fn prepare_file(path: &Path, file_name: Option<&str>) -> Result<BitwardenImport, String> {
    let name = if vault::file_io::is_document_uri(path) {
        Path::new(file_name.ok_or("无法确定文档类型，请重新选择 Bitwarden JSON 文件")?)
    } else {
        path
    };
    if !name
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("json"))
    {
        return Err("仅支持导入 .json 文件".into());
    }
    let bytes = Zeroizing::new(
        vault::file_io::read_with_limit(path, "Bitwarden JSON 文件", 64 * 1024 * 1024)
            .map_err(|_| "无法读取 Bitwarden JSON 文件或文件超过 64 MiB 上限")?,
    );
    vault::prepare_bitwarden_import(&bytes)
}

fn complete(
    vaults: &VaultSessions,
    session: &Mutex<VaultSession>,
    session_id: &str,
    target: &str,
    lifetime: Uuid,
    plan: BitwardenImport,
) -> Result<BitwardenImportResult, String> {
    let counts = plan.counts();
    let state = with_vault_session(vaults, session, Some(session_id), |session| {
        session.import_bitwarden(plan, target, lifetime)
    })?;
    Ok(BitwardenImportResult { state, counts })
}

async fn import_for_session(
    vaults: &VaultSessions,
    session: &Mutex<VaultSession>,
    session_id: String,
    path: String,
    file_name: Option<String>,
    target_group_uuid: Option<String>,
) -> Result<BitwardenImportResult, String> {
    static WORKER: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let target = target_group_uuid.unwrap_or_else(|| vault::ROOT_GROUP_UUID.to_owned());
    let lifetime = with_vault_session(vaults, session, Some(&session_id), |session| {
        session.native_import_lifetime(&target)
    })?;
    let permit = WORKER
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(1)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| "Bitwarden JSON 导入任务正在进行，请稍后重试")?;
    let plan = tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        prepare_file(Path::new(&path), file_name.as_deref())
    })
    .await
    .map_err(|_| "Bitwarden JSON 导入任务异常；未导入任何内容")??;
    complete(vaults, session, &session_id, &target, lifetime, plan)
}

#[tauri::command]
pub(crate) async fn import_bitwarden_file(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: String,
    path: String,
    file_name: Option<String>,
    target_group_uuid: Option<String>,
) -> Result<BitwardenImportResult, String> {
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

#[cfg(test)]
#[path = "import_bitwarden_tests.rs"]
mod tests;
