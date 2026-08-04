// Tauri IPC command handlers, grouped by domain. Thin wrappers around the
// backend services; passwords and keys never cross IPC.

use crate::autotype;
use crate::bridge;
use crate::bridge::BridgeHost;
use crate::bridge_server;
use crate::clipboard;
use crate::config;
use crate::config::ConfigStore;
use crate::credential;
use crate::focus;
use crate::register_global_hotkey;
use crate::remote::{local_storage_dir, make_storage, RemoteObject};
use crate::rpc;
use crate::rpc_server;
use crate::shield;
use crate::vault;
use crate::vault::{
    EntryInput, EntryPatch, GroupInput, HistoryVersion, RemoteMode, SecurityReport, TotpCode,
    VaultSession, VaultState,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use zeroize::Zeroize;

/// CSV/XML import cap (8 MiB): guards the read-text command against oversized
/// files; the `.csv`/`.xml` extension whitelist stops arbitrary file
/// exfiltration.
const MAX_TEXT_IMPORT_BYTES: u64 = 8 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Config commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub(crate) fn get_config(
    store: tauri::State<'_, ConfigStore>,
) -> Result<config::AppConfig, String> {
    store.get()
}

/// Trim a stored backup template, falling back to the default when empty.
fn normalize_backup_template(template: &str) -> String {
    let trimmed = template.trim();
    if trimmed.is_empty() {
        vault::DEFAULT_BACKUP_TEMPLATE.to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[tauri::command]
pub(crate) fn set_config(
    store: tauri::State<'_, ConfigStore>,
    app: tauri::AppHandle,
    config: config::AppConfig,
) -> Result<config::AppConfig, String> {
    let saved = store.set(config)?;
    register_global_hotkey(&app, &saved.keyboard.auto_type_global);
    sync_bridge(&app, &saved);
    sync_rpc(&app, &saved);
    Ok(saved)
}

/// Start or stop the loopback bridge to match `bridge.enabled`; failures are
/// logged, never fatal (the app stays usable without browser integration).
pub(crate) fn sync_bridge(app: &tauri::AppHandle, config: &config::AppConfig) {
    let state = app.state::<bridge_server::BridgeState>();
    if config.bridge.enabled {
        if let Err(e) = state.start(app) {
            eprintln!("bridge: {e}");
        }
    } else {
        state.stop();
    }
}

/// Start or stop the loopback KeePassRPC server to match `rpc.enabled`;
/// failures are logged, never fatal (the app stays usable without Kee).
pub(crate) fn sync_rpc(app: &tauri::AppHandle, config: &config::AppConfig) {
    let state = app.state::<rpc_server::RpcState>();
    if config.rpc.enabled {
        if let Err(e) = state.start(app) {
            eprintln!("rpc: {e}");
        }
    } else {
        state.stop();
    }
}

// ---------------------------------------------------------------------------
// Browser bridge commands (KeePassHttp)
// ---------------------------------------------------------------------------

/// Whether the loopback server is currently listening.
#[derive(serde::Serialize)]
pub(crate) struct BridgeStatus {
    running: bool,
    port: u16,
    error: Option<String>,
}
#[tauri::command]
pub(crate) fn bridge_status(
    state: tauri::State<'_, bridge_server::BridgeState>,
) -> Result<BridgeStatus, String> {
    Ok(BridgeStatus {
        running: state.running(),
        port: bridge::BRIDGE_PORT,
        error: state.last_error(),
    })
}

/// Whether the KeePassRPC loopback server is currently listening.
#[derive(serde::Serialize)]
pub(crate) struct RpcStatus {
    running: bool,
    port: u16,
    error: Option<String>,
}

#[tauri::command]
pub(crate) fn rpc_status(
    state: tauri::State<'_, rpc_server::RpcState>,
) -> Result<RpcStatus, String> {
    Ok(RpcStatus {
        running: state.running(),
        port: rpc::RPC_PORT,
        error: state.last_error(),
    })
}

/// Authorized browser clients of the open session (id only — never keys).
#[tauri::command]
pub(crate) fn bridge_clients(
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<Vec<String>, String> {
    Ok(session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .list_clients())
}

/// Deauthorize one browser client; returns the remaining list.
#[tauri::command]
pub(crate) fn bridge_remove_client(
    session: tauri::State<'_, Mutex<VaultSession>>,
    id: String,
) -> Result<Vec<String>, String> {
    let mut session = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
    if !session.remove_client(&id) {
        return Err("未找到该客户端".to_owned());
    }
    Ok(session.list_clients())
}

/// Answer a pending browser-association approval from the settings UI.
#[tauri::command]
pub(crate) fn bridge_approve(
    board: tauri::State<'_, bridge_server::ApprovalBoard>,
    token: String,
    allowed: bool,
) -> Result<(), String> {
    if board.decide(&token, allowed) {
        Ok(())
    } else {
        Err("审批已过期或不存在".to_owned())
    }
}

// ---------------------------------------------------------------------------
// Credential-store commands (Windows Hello quick unlock)
// ---------------------------------------------------------------------------

/// Store the master password for a vault path in the OS credential store.
#[tauri::command]
pub(crate) fn remember_credential(path: String, mut password: String) -> Result<(), String> {
    let result = credential::remember(&path, &password);
    password.zeroize();
    result
}

/// Fetch the stored master password for a vault path, if any.
#[tauri::command]
pub(crate) fn get_saved_credential(path: String) -> Result<Option<String>, String> {
    credential::get(&path)
}

/// Remove the stored master password for a vault path.
#[tauri::command]
pub(crate) fn clear_saved_credential(path: String) -> Result<(), String> {
    credential::forget(&path)
}

// ---------------------------------------------------------------------------
// Vault commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub(crate) fn open_vault(
    app: tauri::AppHandle,
    config: tauri::State<'_, ConfigStore>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
    mut password: String,
    keyfile: Option<String>,
) -> Result<VaultState, String> {
    // Slow work (file read, KDF, parse) runs outside the session lock; only
    // the state adoption is locked.
    let prepared = vault::prepare_local_open(
        Path::new(&path),
        &password,
        keyfile.as_deref().map(Path::new),
    );
    let result = match prepared {
        Ok((db, keyfile_bytes)) => {
            let mut session = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            let result = session.adopt_local(db, Path::new(&path), &password, keyfile_bytes);
            drop(session);
            result
        }
        Err(e) => Err(e),
    };
    password.zeroize();
    if result.is_ok() {
        apply_capture_guard(&app, &config);
    }
    result
}

/// Apply the screen-capture guard only when the user enabled it in settings.
fn apply_capture_guard(app: &tauri::AppHandle, config: &ConfigStore) {
    let Ok(cfg) = config.get() else { return };
    if cfg.security.screen_capture_guard {
        shield::set_capture_guard(app, true);
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_vault(
    app: tauri::AppHandle,
    config: tauri::State<'_, ConfigStore>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
    mut password: String,
    kdf: String,
    cipher: String,
    compression: String,
    keyfile: Option<String>,
) -> Result<VaultState, String> {
    // Slow work (KDF, serialization, file write) runs outside the session
    // lock; only the state adoption is locked.
    let prepared = vault::prepare_local_create(
        Path::new(&path),
        &password,
        &kdf,
        &cipher,
        &compression,
        keyfile.as_deref().map(Path::new),
    );
    let result = match prepared {
        Ok((db, keyfile_bytes)) => {
            let mut session = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            let result = session.adopt_local(db, Path::new(&path), &password, keyfile_bytes);
            drop(session);
            result
        }
        Err(e) => Err(e),
    };
    password.zeroize();
    if result.is_ok() {
        apply_capture_guard(&app, &config);
    }
    result
}

#[tauri::command]
pub(crate) fn close_vault(
    app: tauri::AppHandle,
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<(), String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .close();
    shield::set_capture_guard(&app, false);
    Ok(())
}

#[tauri::command]
pub(crate) fn get_vault_state(
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<Option<VaultState>, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .state()
}

#[tauri::command]
pub(crate) fn save_vault(
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<VaultState, String> {
    // Capture a cheap job under the lock, then run KDF + serialization +
    // transport outside it, then mark clean under the lock again.
    let job = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .prepare_save()?;
    let revision = job.revision;
    vault::persist_save(job)?;
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .complete_save(revision)
}

#[tauri::command]
pub(crate) fn save_vault_as(
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .save_as(Path::new(&path))
}

#[tauri::command]
pub(crate) fn change_master_key(
    session: tauri::State<'_, Mutex<VaultSession>>,
    password: String,
    keyfile: Option<String>,
) -> Result<VaultState, String> {
    // Keyfile read, KDF and persistence all happen without the session lock.
    let keyfile_bytes = vault::read_keyfile(keyfile.as_deref().map(Path::new))?;
    let (db, target, revision) = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .prepare_change()?;
    let persisted = vault::persist_change(&db, &password, keyfile_bytes.as_deref(), &target);
    let result = match persisted {
        Ok(()) => session
            .lock()
            .map_err(|_| "数据库锁已损坏".to_owned())?
            .complete_change(password, keyfile_bytes, revision),
        Err(e) => Err(e),
    };
    result
}

#[tauri::command]
pub(crate) fn add_entry(
    session: tauri::State<'_, Mutex<VaultSession>>,
    input: EntryInput,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .add_entry(&input)
}

#[tauri::command]
pub(crate) fn update_entry(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    input: EntryInput,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .update_entry(&uuid, &input)
}

#[tauri::command]
pub(crate) fn update_entries(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuids: Vec<String>,
    patch: EntryPatch,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .update_entries(&uuids, &patch)
}

#[tauri::command]
pub(crate) fn delete_entry(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .delete_entry(&uuid)
}

#[tauri::command]
pub(crate) fn move_entry(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    group_uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .move_entry(&uuid, &group_uuid)
}

#[tauri::command]
pub(crate) fn delete_entries(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuids: Vec<String>,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .delete_entries(&uuids)
}

#[tauri::command]
pub(crate) fn get_entry_history(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<Vec<HistoryVersion>, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .get_entry_history(&uuid)
}

#[tauri::command]
pub(crate) fn restore_entry_version(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    index: usize,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .restore_entry_version(&uuid, index)
}

#[tauri::command]
pub(crate) fn restore_entry(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .restore_entry(&uuid)
}

#[tauri::command]
pub(crate) fn save_attachment(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    name: String,
    dest: String,
) -> Result<(), String> {
    // Extract under the lock, write the file outside it.
    let data = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .attachment_data(&uuid, &name)?;
    vault::write_attachment_file(&data, &dest)
}

#[tauri::command]
pub(crate) fn totp_code(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<TotpCode, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .totp_code(&uuid)
}

#[tauri::command]
pub(crate) fn toggle_favorite(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .toggle_favorite(&uuid)
}

/// Resolve and replay a KeePass-style auto-type sequence for an entry.
/// Executes on a background thread; returns once parsing succeeds.
///
/// The main window is minimized first so keystrokes land in the window the
/// user switches to during the replay delay, never in KeyVault itself.
#[tauri::command]
pub(crate) fn auto_type(
    app: tauri::AppHandle,
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    sequence: String,
) -> Result<(), String> {
    let (ctx, expanded) = {
        let session = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        let ctx = session.autotype_context(&uuid)?;
        let expanded = session.expand_autotype_sequence(&sequence)?;
        (ctx, expanded)
    };
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.minimize();
    }
    autotype::run_sequence(&expanded, &ctx).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Download Favicons (KeePass-style: fetch per host, store as custom icons)
// ---------------------------------------------------------------------------

/// Build the favicon HTTP client. Windows follows the WinINET system proxy
/// (`ProxyEnable`/`ProxyServer` in the Internet Settings registry hive, the
/// same source .NET/KeePass uses); reqwest's `system-proxy` feature only
/// reads environment variables, which is why KeePass can reach hosts that
/// KeyVault could not. Other platforms rely on the env-var proxy instead.
///
/// The timeout is generous (20 s) on purpose: the first TLS handshake
/// through a proxy frequently takes ~5-10 s, and a tight timeout kills the
/// first request while the retry on the warm connection succeeds.
fn build_favicon_client() -> Option<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("KeyVault/0.1");
    if let Some(proxy) = wininet_https_proxy() {
        if let Ok(proxy) = reqwest::Proxy::https(proxy) {
            builder = builder.proxy(proxy);
        }
    }
    builder.build().ok()
}

/// Windows system proxy for https targets, as `http://host:port`. Returns
/// `None` when the system proxy is disabled or cannot be parsed.
#[cfg(windows)]
fn wininet_https_proxy() -> Option<String> {
    use std::ptr;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
        RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
    };

    fn u16z(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let mut hkey: HKEY = ptr::null_mut();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            u16z("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings").as_ptr(),
            0,
            KEY_QUERY_VALUE,
            &mut hkey,
        )
    };
    if status != 0 {
        return None;
    }
    let mut enabled: u32 = 0;
    let mut len = std::mem::size_of::<u32>() as u32;
    let ok = unsafe {
        RegGetValueW(
            hkey,
            ptr::null(),
            u16z("ProxyEnable").as_ptr(),
            RRF_RT_REG_DWORD,
            ptr::null_mut(),
            &mut enabled as *mut u32 as *mut _,
            &mut len,
        )
    };
    if ok != 0 || enabled == 0 {
        unsafe { RegCloseKey(hkey) };
        return None;
    }
    let mut buf = [0u16; 1024];
    len = (buf.len() * 2) as u32;
    let ok = unsafe {
        RegGetValueW(
            hkey,
            ptr::null(),
            u16z("ProxyServer").as_ptr(),
            RRF_RT_REG_SZ,
            ptr::null_mut(),
            buf.as_mut_ptr() as *mut _,
            &mut len,
        )
    };
    unsafe { RegCloseKey(hkey) };
    if ok != 0 {
        return None;
    }
    let raw = String::from_utf16_lossy(&buf[..len as usize / 2]);
    parse_proxy_server(raw.trim_end_matches('\0')).map(|p| format!("http://{p}"))
}

/// Parse a WinINET `ProxyServer` value: plain `host:port`, scheme-qualified
/// `http=host:port;https=host:port;…`, or default-plus-`secure=` form
/// `host:port;secure=host:port`. Returns the proxy for https traffic.
fn parse_proxy_server(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }
    let picked = if parts
        .iter()
        .any(|p| p.starts_with("https=") || p.starts_with("http="))
    {
        parts
            .iter()
            .find_map(|part| part.strip_prefix("https="))
            .or_else(|| parts.iter().find_map(|part| part.strip_prefix("http=")))
    } else if parts.iter().any(|p| p.starts_with("secure=")) {
        parts.iter().find_map(|part| part.strip_prefix("secure="))
    } else if !parts[0].contains('=') {
        Some(parts[0])
    } else {
        None
    };
    picked
        .map(|value| value.strip_prefix("http://").unwrap_or(value))
        .map(str::to_owned)
}

#[cfg(not(windows))]
fn wininet_https_proxy() -> Option<String> {
    None
}

/// Fetch `https://{host}/favicon.ico` (then `/favicon.png`), with a 20-second
/// timeout and a 512 KiB size cap. Returns `None` when nothing is served;
/// every failure reason is logged to stderr (full error chain) so server-side
/// diagnosis is possible without changing the renderer contract.
async fn fetch_favicon(host: &str) -> Option<Vec<u8>> {
    let client = match build_favicon_client() {
        Some(client) => client,
        None => {
            eprintln!("[favicon] 构建 HTTP 客户端失败 ({host})");
            return None;
        }
    };
    for path in ["/favicon.ico", "/favicon.png"] {
        let url = format!("https://{host}{path}");
        let response = match client.get(&url).send().await {
            Ok(response) => response,
            Err(e) => {
                eprintln!("[favicon] 请求 {url} 失败: {e:#}");
                continue;
            }
        };
        if !response.status().is_success() {
            eprintln!("[favicon] {url} 返回 {}", response.status());
            continue;
        }
        let bytes = match response.bytes().await {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("[favicon] 读取 {url} 响应失败: {e}");
                continue;
            }
        };
        if bytes.is_empty() {
            eprintln!("[favicon] {url} 返回空内容");
            continue;
        }
        if bytes.len() >= 512 * 1024 {
            eprintln!("[favicon] {url} 超过 512 KiB 上限 ({} 字节)", bytes.len());
            continue;
        }
        return Some(bytes.to_vec());
    }
    None
}

/// Download favicons for the given entry URLs (or every entry when `uuids`
/// is empty/None) and write them back into the database as custom icons
/// (persisted immediately). Only the listed entries receive icons.
///
/// Emits `favicon-progress` (`{ done, total }`) after each host finishes so
/// the renderer can show a progress dialog.
///
/// Hosts are fetched concurrently, capped by the configurable
/// `favicon.concurrency` (default 8) so a large database cannot open
/// hundreds of simultaneous tunnels through the system proxy.
#[tauri::command]
pub(crate) async fn download_favicons(
    app: tauri::AppHandle,
    session: tauri::State<'_, Mutex<VaultSession>>,
    config: tauri::State<'_, ConfigStore>,
    uuids: Option<Vec<String>>,
) -> Result<vault::FaviconReport, String> {
    let jobs = {
        let session = session.lock().map_err(|_| {
            eprintln!("[favicon] 数据库锁已损坏");
            "数据库锁已损坏".to_owned()
        })?;
        match &uuids {
            Some(selected) if !selected.is_empty() => {
                session.favicon_jobs_selected(selected).map_err(|e| {
                    eprintln!("[favicon] 收集选中条目图标任务失败: {e}");
                    e
                })?
            }
            _ => session.favicon_jobs().map_err(|e| {
                eprintln!("[favicon] 收集图标任务失败: {e}");
                e
            })?,
        }
    };
    let total = jobs.len();
    let mut done = 0usize;
    let concurrency = config
        .get()
        .map(|cfg| cfg.favicon.concurrency.max(1) as usize)
        .unwrap_or(8);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let mut set = tokio::task::JoinSet::new();
    for job in &jobs {
        let host = job.host.clone();
        let semaphore = semaphore.clone();
        set.spawn(async move {
            let host = host;
            // A closed semaphore (only on shutdown) degrades to unlimited
            // concurrency instead of failing the download.
            let _permit = semaphore.acquire_owned().await.ok();
            (host.clone(), fetch_favicon(&host).await)
        });
    }
    let mut fetched: Vec<vault::FaviconFetch> = Vec::new();
    while let Some(result) = set.join_next().await {
        if let Ok((host, Some(bytes))) = result {
            fetched.push(vault::FaviconFetch { host, bytes });
        }
        done += 1;
        let _ = app.emit("favicon-progress", vault::FaviconProgress { done, total });
    }
    let downloaded = fetched.len();
    let report = {
        let mut session = session.lock().map_err(|_| {
            eprintln!("[favicon] 数据库锁已损坏");
            "数据库锁已损坏".to_owned()
        })?;
        session.apply_favicons(&jobs, fetched).map_err(|e| {
            eprintln!("[favicon] 写入图标失败: {e}");
            e
        })?;
        session.save().map_err(|e| {
            eprintln!("[favicon] 保存数据库失败: {e}");
            e
        })?;
        vault::FaviconReport {
            attempted: jobs.len(),
            downloaded,
        }
    };
    Ok(report)
}

// ---------------------------------------------------------------------------
// TCATO (two-channel auto-type overlay)
// ---------------------------------------------------------------------------

/// UUID of the entry the TCATO overlay currently targets; never the password.
pub(crate) struct TcatoTarget(pub(crate) Mutex<Option<String>>);

/// Lightweight info shown in the TCATO overlay; secrets never leave the
/// backend, so the password itself is only reported as a boolean.
#[derive(serde::Serialize)]
pub(crate) struct TcatoInfo {
    title: String,
    username: String,
    has_password: bool,
}

const TCATO_WINDOW_LABEL: &str = "tcato";

/// Open (or focus) the small always-on-top overlay that sends one channel of
/// credentials to the window in focus without simulated key presses.
#[tauri::command]
pub(crate) fn open_tcato_overlay(
    app: tauri::AppHandle,
    session: tauri::State<'_, Mutex<VaultSession>>,
    target: tauri::State<'_, TcatoTarget>,
    uuid: String,
) -> Result<(), String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .autotype_context(&uuid)?;
    let mut slot = target.0.lock().map_err(|_| "覆盖层状态已损坏".to_owned())?;
    *slot = Some(uuid);
    drop(slot);
    if let Some(window) = app.get_webview_window(TCATO_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        &app,
        TCATO_WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("TCATO 两通道填充")
    .inner_size(360.0, 190.0)
    .min_inner_size(360.0, 190.0)
    .resizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .initialization_script("window.location.hash = '#/tcato';")
    .build()
    .map_err(|e| format!("无法打开 TCATO 窗口: {e}"))?;
    Ok(())
}

/// Info for the overlay UI: entry title and which channels are available.
#[tauri::command]
pub(crate) fn tcato_state(
    session: tauri::State<'_, Mutex<VaultSession>>,
    target: tauri::State<'_, TcatoTarget>,
) -> Result<Option<TcatoInfo>, String> {
    let uuid = target
        .0
        .lock()
        .map_err(|_| "覆盖层状态已损坏".to_owned())?
        .clone();
    let Some(uuid) = uuid else { return Ok(None) };
    let ctx = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .autotype_context(&uuid)?;
    Ok(Some(TcatoInfo {
        title: ctx.title,
        username: ctx.username,
        has_password: !ctx.password.is_empty(),
    }))
}

/// Send one channel (`username` or `password`) to the window in focus.
#[tauri::command]
pub(crate) fn tcato_send(
    session: tauri::State<'_, Mutex<VaultSession>>,
    target: tauri::State<'_, TcatoTarget>,
    channel: String,
) -> Result<(), String> {
    let uuid = target
        .0
        .lock()
        .map_err(|_| "覆盖层状态已损坏".to_owned())?
        .clone()
        .ok_or_else(|| "TCATO 覆盖层尚未指定条目".to_owned())?;
    let ctx = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .autotype_context(&uuid)?;
    let text = match channel.as_str() {
        "username" => ctx.username,
        "password" => ctx.password,
        _ => return Err("无效的 TCATO 通道".to_owned()),
    };
    focus::send_text_to_foreground(&text)
}

/// Close the TCATO overlay.
#[tauri::command]
pub(crate) fn close_tcato_overlay(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(TCATO_WINDOW_LABEL) {
        let _ = window.close();
    }
}

#[tauri::command]
pub(crate) fn add_group(
    session: tauri::State<'_, Mutex<VaultSession>>,
    input: GroupInput,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .add_group(&input)
}

#[tauri::command]
pub(crate) fn rename_group(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    name: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .rename_group(&uuid, &name)
}

#[tauri::command]
pub(crate) fn delete_group(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .delete_group(&uuid)
}

#[tauri::command]
pub(crate) fn restore_group(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .restore_group(&uuid)
}

#[tauri::command]
pub(crate) fn empty_recycle_bin(
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .empty_recycle_bin()
}

/// Fetch one entry's password on demand; passwords are never part of `VaultState`.
#[tauri::command]
pub(crate) fn get_entry_password(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<String, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .get_entry_password(&uuid)
}

/// Fetch one entry's TOTP seed on demand; seeds are never part of `VaultState`.
#[tauri::command]
pub(crate) fn get_entry_totp(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<Option<String>, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .get_entry_totp(&uuid)
}

/// Server-side security analysis; no passwords leave the session.
#[tauri::command]
pub(crate) fn security_report(
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<SecurityReport, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .security_report()
}

/// Export all entries as CSV to a user-picked path (passwords resolved server-side).
#[tauri::command]
pub(crate) fn export_csv(
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
) -> Result<(), String> {
    // Build the payload under the lock, write the file outside it.
    let content = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .export_csv_content()?;
    vault::write_csv_file(&path, &content)
}

/// Read a UTF-8 text file from a user-picked path (CSV / KeePass XML import).
/// Only `.csv` and `.xml` files are accepted: the command must never serve as
/// an arbitrary local file reader (e.g. for config.json, credentials, or other
/// vaults).
#[tauri::command]
pub(crate) fn read_text_file(path: String) -> Result<String, String> {
    let path = Path::new(&path);
    let allowed = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("csv") || e.eq_ignore_ascii_case("xml"));
    if !allowed {
        return Err("仅支持导入 .csv 或 .xml 文件".to_owned());
    }
    let meta = std::fs::metadata(path).map_err(|e| format!("读取文件失败: {e}"))?;
    if meta.len() > MAX_TEXT_IMPORT_BYTES {
        return Err(format!(
            "导入文件过大 (最大 {} MiB)",
            MAX_TEXT_IMPORT_BYTES / 1024 / 1024
        ));
    }
    std::fs::read_to_string(path).map_err(|e| format!("读取文件失败: {e}"))
}

// ---------------------------------------------------------------------------
// Clipboard commands
// ---------------------------------------------------------------------------

/// Current clipboard text (or `null` when it holds non-text content).
/// Used by the scheduled wipe to avoid destroying text the user copied in
/// another app after ours.
#[tauri::command]
pub(crate) fn clipboard_read_text() -> Result<Option<String>, String> {
    clipboard::read_clipboard_text()
}

/// Empty the clipboard. The frontend calls this only after verifying the
/// clipboard still holds our own text (or on lock with `clearOnLock`).
#[tauri::command]
pub(crate) fn clipboard_clear() -> Result<(), String> {
    clipboard::clear_clipboard()
}

// ---------------------------------------------------------------------------
// S3 remote commands
// ---------------------------------------------------------------------------

/// List `.kdbx` files under the active profile's prefix (newest key first).
/// `profile` is the index into `remoteProfiles`; settings (including the
/// decrypted credentials) are resolved from `ConfigStore`, never sent over IPC.
#[tauri::command]
pub(crate) async fn s3_list_objects(
    profile: usize,
    config: tauri::State<'_, ConfigStore>,
) -> Result<Vec<RemoteObject>, String> {
    let cfg = config.remote_settings(profile)?;
    crate::remote::list_objects_async(cfg).await
}

/// Download a vault from S3 and open it. `mode` is `"memory"` (upload back
/// only) or `"local"` (also mirror locally under
/// `Storage/remote/<sanitized profile name>`).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn open_remote_vault(
    session: tauri::State<'_, Mutex<VaultSession>>,
    app: tauri::AppHandle,
    config: tauri::State<'_, ConfigStore>,
    profile: usize,
    key: String,
    password: String,
    keyfile: Option<String>,
    mode: String,
) -> Result<VaultState, String> {
    let (profile_name, cfg) = config.remote_profile(profile)?;
    let storage = make_storage(&cfg)?;
    let mode = RemoteMode::parse(&mode)?;
    let local_dir = local_storage_dir(&app, &profile_name)?;
    let backup_count = cfg.backup_count.clamp(0, 10) as usize;
    let backup_template = normalize_backup_template(&cfg.backup_template);
    let keyfile_path = keyfile.map(PathBuf::from);
    let local_dir_for_network = local_dir.clone();
    let storage_for_network = storage.clone();
    let backup_template_for_network = backup_template.clone();
    // Network download, KDF and parse run without the session lock, off the
    // async worker thread: the remote transport blocks on its own runtime, which
    // panics on a runtime worker (the command future would abort and the
    // invoke would never resolve — the UI would stay on "正在加载…").
    let (prepared, mut password) = tauri::async_runtime::spawn_blocking(move || {
        let result = vault::prepare_remote_open(
            storage_for_network,
            &key,
            &password,
            keyfile_path.as_deref(),
            mode,
            &local_dir_for_network,
            backup_count,
            &backup_template_for_network,
        );
        (result, password)
    })
    .await
    .map_err(|e| format!("远程打开任务异常: {e}"))?;
    let result = match prepared {
        Ok((db, keyfile_bytes, key)) => {
            let mut session = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            let result = session.adopt_remote(
                db,
                storage,
                &key,
                &password,
                keyfile_bytes,
                mode,
                &local_dir,
                cfg.backup_count.clamp(0, 10) as usize,
                &backup_template,
            );
            drop(session);
            result
        }
        Err(e) => Err(e),
    };
    password.zeroize();
    if result.is_ok() {
        apply_capture_guard(&app, &config);
    }
    result
}

/// Create an empty vault and upload it to S3 immediately.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_remote_vault(
    session: tauri::State<'_, Mutex<VaultSession>>,
    app: tauri::AppHandle,
    config: tauri::State<'_, ConfigStore>,
    profile: usize,
    key: String,
    password: String,
    kdf: String,
    cipher: String,
    compression: String,
    keyfile: Option<String>,
    mode: String,
) -> Result<VaultState, String> {
    let (profile_name, cfg) = config.remote_profile(profile)?;
    let storage = make_storage(&cfg)?;
    let mode = RemoteMode::parse(&mode)?;
    let local_dir = local_storage_dir(&app, &profile_name)?;
    let backup_count = cfg.backup_count.clamp(0, 10) as usize;
    let backup_template = normalize_backup_template(&cfg.backup_template);
    let keyfile_path = keyfile.map(PathBuf::from);
    let local_dir_for_network = local_dir.clone();
    let storage_for_network = storage.clone();
    let backup_template_for_network = backup_template.clone();
    // KDF, serialization, upload and local mirror run without the lock, off
    // the async worker thread (the remote transport must not block a runtime
    // worker — see the comment in `open_remote_vault`).
    let (prepared, mut password) = tauri::async_runtime::spawn_blocking(move || {
        let result = vault::prepare_remote_create(
            storage_for_network,
            &key,
            &password,
            &kdf,
            &cipher,
            &compression,
            keyfile_path.as_deref(),
            mode,
            &local_dir_for_network,
            backup_count,
            &backup_template_for_network,
        );
        (result, password)
    })
    .await
    .map_err(|e| format!("远程创建任务异常: {e}"))?;
    let result = match prepared {
        Ok((db, keyfile_bytes, key)) => {
            let mut session = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            let result = session.adopt_remote(
                db,
                storage,
                &key,
                &password,
                keyfile_bytes,
                mode,
                &local_dir,
                cfg.backup_count.clamp(0, 10) as usize,
                &backup_template,
            );
            drop(session);
            result
        }
        Err(e) => Err(e),
    };
    password.zeroize();
    if result.is_ok() {
        apply_capture_guard(&app, &config);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_file(dir: &TempDir, name: &str, content: &str) -> String {
        let path = dir.path().join(name);
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn read_text_file_accepts_csv_and_xml_and_rejects_others() {
        let dir = TempDir::new().unwrap();
        let csv = write_file(&dir, "import.csv", "title,username,password\n");
        assert_eq!(read_text_file(csv).unwrap(), "title,username,password\n");

        let xml = write_file(&dir, "vault.kdbx.xml", "<KeePassFile/>");
        assert_eq!(read_text_file(xml).unwrap(), "<KeePassFile/>");

        let txt = write_file(&dir, "notes.txt", "secret local text");
        let err = read_text_file(txt).unwrap_err();
        assert!(err.contains(".csv"), "unexpected error: {err}");

        let no_ext = write_file(&dir, "config", "{}");
        assert!(read_text_file(no_ext).unwrap_err().contains(".csv"));
    }

    #[test]
    fn read_text_file_rejects_missing_path() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nope.csv").to_string_lossy().into_owned();
        assert!(read_text_file(missing).unwrap_err().contains("失败"));
    }

    #[test]
    fn parse_proxy_server_handles_wininet_forms() {
        assert_eq!(
            parse_proxy_server("127.0.0.1:51400").as_deref(),
            Some("127.0.0.1:51400")
        );
        assert_eq!(
            parse_proxy_server("host:8080;secure=10.0.0.1:8443").as_deref(),
            Some("10.0.0.1:8443")
        );
        assert_eq!(
            parse_proxy_server("http=127.0.0.1:7890;https=127.0.0.1:7891").as_deref(),
            Some("127.0.0.1:7891")
        );
        assert_eq!(
            parse_proxy_server("https=proxy.local:3128").as_deref(),
            Some("proxy.local:3128")
        );
        assert_eq!(
            parse_proxy_server("ftp=ftp.local:21;http=127.0.0.1:8080").as_deref(),
            Some("127.0.0.1:8080")
        );
        assert_eq!(
            parse_proxy_server("http://127.0.0.1:51400").as_deref(),
            Some("127.0.0.1:51400")
        );
        assert_eq!(parse_proxy_server("").as_deref(), None);
        assert_eq!(parse_proxy_server("ftp=ftp.local:21").as_deref(), None);
    }
}
