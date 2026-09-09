//! Favicon download command: per-host HTTPS fetch (WinINET system proxy),
//! 512 KiB cap, concurrency-limited fan-out, write-back as custom icons
//! (extracted from commands.rs).

use crate::config::ConfigStore;
use crate::vault;
use crate::vault::{VaultSession, VaultSessions};
use regex::Regex;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::Emitter;
use tokio::sync::Notify;
use url::Url;

/// Cooperative cancel signal for the in-flight `download_favicons` run.
/// `flag` persists the cancel request so tasks spawned after the `Notify`
/// wake still observe it; `notify` wakes tasks already awaiting.
pub(crate) struct FaviconCancel {
    pub notify: Arc<Notify>,
    pub flag: Arc<std::sync::atomic::AtomicBool>,
}

impl Default for FaviconCancel {
    fn default() -> Self {
        Self {
            notify: Arc::new(Notify::new()),
            flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

#[tauri::command]
pub(crate) fn cancel_favicons(cancel: tauri::State<'_, FaviconCancel>) -> Result<(), String> {
    cancel.flag.store(true, std::sync::atomic::Ordering::SeqCst);
    cancel.notify.notify_waiters();
    Ok(())
}

// ---------------------------------------------------------------------------
// Download Favicons (KeePass-style: fetch per host, store as custom icons)
// ---------------------------------------------------------------------------

/// Build the favicon HTTP client. Windows follows the WinINET system proxy
/// (`ProxyEnable`/`ProxyServer` in the Internet Settings registry hive, the
/// same source .NET/KeePass uses); reqwest's `system-proxy` feature only
/// reads environment variables, which is why KeePass can reach hosts that
/// SecPivot could not. The proxy is applied to both https targets (the fast
/// path) and the http fallback (sites without TLS), matching WinINET's
/// default proxy behavior; a scheme-less `host:port` proxy thus serves both.
/// Other platforms rely on the env-var proxy instead.
///
/// The timeout is generous (20 s) on purpose: the first TLS handshake
/// through a proxy frequently takes ~5-10 s, and a tight timeout kills the
/// first request while the retry on the warm connection succeeds. The
/// User-Agent is a bare browser-compatible token so WAFs that reject
/// unknown/client bot agents still serve the icon.
fn build_favicon_client() -> Option<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)");
    if let Some(proxy) = wininet_https_proxy() {
        if let Ok(proxy) = reqwest::Proxy::all(&proxy) {
            builder = builder.proxy(proxy);
        } else if let Ok(proxy) = reqwest::Proxy::https(proxy) {
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
#[cfg(any(windows, test))]
pub(crate) fn parse_proxy_server(raw: &str) -> Option<String> {
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

/// Size cap for a downloaded icon (ICO/PNG/SVG body).
const ICON_CAP: usize = 512 * 1024;
/// Size cap for a site's HTML page read to locate `<link rel="icon">`.
const PAGE_CAP: usize = 512 * 1024;

/// Stream a URL body with a byte cap, aborting oversized or endless responses
/// without buffering them fully (and skipping the transfer immediately when
/// the announced `Content-Length` already exceeds the cap). Returns the body
/// of a 2xx, non-empty response; every other outcome is logged and returns
/// `None`. `cancel`/`flag` are the cooperative signal from `cancel_favicons`,
/// checked before and across every await so "结束等待" aborts without waiting
/// for the timeout.
async fn fetch_bytes(
    client: &reqwest::Client,
    url: &str,
    cap: usize,
    cancel: Arc<Notify>,
    flag: Arc<std::sync::atomic::AtomicBool>,
) -> Option<Vec<u8>> {
    if flag.load(std::sync::atomic::Ordering::SeqCst) {
        return None;
    }
    let mut response = tokio::select! {
        res = client.get(url).send() => match res {
            Ok(response) => response,
            Err(e) => {
                eprintln!("[favicon] 请求 {url} 失败: {e:#}");
                return None;
            }
        },
        _ = cancel.notified() => {
            eprintln!("[favicon] 已取消 {url}");
            return None;
        }
    };
    if !response.status().is_success() {
        eprintln!("[favicon] {url} 返回 {}", response.status());
        return None;
    }
    if let Some(len) = response.content_length() {
        if len > cap as u64 {
            eprintln!("[favicon] {url} 超过 {cap} 字节上限 (Content-Length {len})");
            return None;
        }
    }
    let mut body = Vec::new();
    let mut total = 0usize;
    loop {
        let chunk = tokio::select! {
            res = response.chunk() => match res {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(e) => {
                    eprintln!("[favicon] 读取 {url} 响应失败: {e}");
                    return None;
                }
            },
            _ = cancel.notified() => {
                eprintln!("[favicon] 已取消 {url} 读取");
                return None;
            }
        };
        total += chunk.len();
        if total >= cap {
            eprintln!("[favicon] {url} 超过 {cap} 字节上限 (已读取 {total} 字节)");
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    if body.is_empty() {
        eprintln!("[favicon] {url} 返回空内容");
        return None;
    }
    Some(body)
}

/// Sniff the response body as a real image (ICO/CUR, PNG, JPEG, GIF, BMP,
/// RIFF/WebP, SVG) so a 200-with-HTML soft-404 is rejected instead of being
/// stored as a garbage "icon", and the next candidate is tried. Mirrors the
/// media-type sniffing the renderer uses to build icon data URLs
/// (`icon_to_data_url` in `vault/serialize.rs`).
pub(crate) fn looks_like_image(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) // ICO
        || bytes.starts_with(&[0x00, 0x00, 0x02, 0x00]) // CUR
        || bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) // PNG
        || bytes.starts_with(&[0xFF, 0xD8, 0xFF]) // JPEG
        || bytes.starts_with(b"GIF8") // GIF
        || bytes.starts_with(b"BM") // BMP
        // WebP: `RIFF....WEBP`
        || (bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP")
        || bytes.starts_with(b"<svg") // SVG
        || bytes.starts_with(b"\xEF\xBB\xBF<svg") // UTF-8 BOM SVG
        || bytes.starts_with(b"<?xml") // SVG with XML prolog
}

/// Cheap guess whether a fetched body is HTML text worth scanning for
/// `<link rel="icon">`; binary payloads (or a soft-404 image in place of the
/// page) are rejected without a regex pass.
fn is_html_page(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let head = String::from_utf8_lossy(head);
    let lower = head.trim_start().to_ascii_lowercase();
    lower.starts_with("<!doctype html")
        || lower.starts_with("<html")
        || lower.starts_with("<head")
        || lower.contains("<link")
}

/// Extract an attribute like `rel="icon"`, `href='/a.png'` or `href=/a.png`
/// (double-quoted, single-quoted, or unquoted) from a tag string.
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let pattern = regex::escape(name);
    let re = Regex::new(&format!(
        r#"(?i)\b{pattern}\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#
    ))
    .ok()?;
    let caps = re.captures(tag)?;
    caps.get(1)
        .or_else(|| caps.get(2))
        .or_else(|| caps.get(3))
        .map(|m| m.as_str().to_owned())
}

/// Collect the favicon URLs an HTML page declares via `<link rel="icon">`
/// (including `shortcut icon`, `apple-touch-icon`, `mask-icon`), in priority
/// order: plain favicon links first, Apple-touch-icons last. Each href is
/// resolved against `base_url` (absolute, scheme-relative, root-relative and
/// bare relative forms); non-http(s) (e.g. `data:`) and unresolvable hrefs
/// are dropped. This mirrors how the built-in KeePass downloader finds icons,
/// which is where most real sites declare them (`/favicon.ico` at the root is
/// the exception these days).
pub(crate) fn favicon_link_urls(html: &str, base_url: &str) -> Vec<String> {
    static TAGS: OnceLock<Regex> = OnceLock::new();
    let tags = TAGS.get_or_init(|| Regex::new(r#"(?is)<link\b[^>]*>"#).unwrap());
    if !is_html_page(html.as_bytes()) {
        return Vec::new();
    }
    let Ok(base) = Url::parse(base_url) else {
        return Vec::new();
    };
    let mut direct = Vec::new();
    let mut apple = Vec::new();
    for m in tags.find_iter(html) {
        let tag = m.as_str();
        let Some(rel) = attr_value(tag, "rel") else {
            continue;
        };
        let rel = rel.to_ascii_lowercase();
        let is_apple = rel
            .split_whitespace()
            .any(|token| token.starts_with("apple-touch-icon"));
        if !is_apple && !rel.split_whitespace().any(|token| token.contains("icon")) {
            continue;
        }
        let Some(href) = attr_value(tag, "href") else {
            continue;
        };
        let Ok(resolved) = base.join(href.trim()) else {
            continue;
        };
        match resolved.scheme() {
            "http" | "https" => {}
            _ => continue,
        }
        (if is_apple { &mut apple } else { &mut direct }).push(resolved.into());
    }
    direct.extend(apple);
    direct
}

/// Fetch a favicon for `host`, trying in order: the well-known root paths
/// (`/favicon.ico`, `/favicon.png`) over https, then the site's https page
/// (`https://{host}/`) for the `<link rel="icon">` it declares, then the same
/// two passes over http for sites without TLS. Each candidate must return
/// actual image bytes (magic-sniffed) so a soft-404 HTML page or a replaced
/// placeholder is skipped; every failure reason is logged to stderr (full
/// error chain) so server-side diagnosis is possible without changing the
/// renderer contract. Returns `None` when no candidate yields an image.
async fn fetch_favicon(
    client: &reqwest::Client,
    host: &str,
    cancel: Arc<Notify>,
    flag: Arc<std::sync::atomic::AtomicBool>,
) -> Option<Vec<u8>> {
    if flag.load(std::sync::atomic::Ordering::SeqCst) {
        return None;
    }
    for scheme in ["https", "http"] {
        let base = format!("{scheme}://{host}/");
        let mut tried: std::collections::HashSet<String> = std::collections::HashSet::new();
        for path in ["favicon.ico", "favicon.png"] {
            let url = format!("{base}{path}");
            tried.insert(url.clone());
            if let Some(bytes) =
                fetch_bytes(client, &url, ICON_CAP, cancel.clone(), flag.clone()).await
            {
                if looks_like_image(&bytes) {
                    return Some(bytes);
                }
                eprintln!("[favicon] {url} 不是图片内容，尝试下一个候选");
            }
        }
        if let Some(page) = fetch_bytes(client, &base, PAGE_CAP, cancel.clone(), flag.clone()).await
        {
            for url in favicon_link_urls(&String::from_utf8_lossy(&page), &base) {
                if tried.insert(url.clone()) {
                    if let Some(bytes) =
                        fetch_bytes(client, &url, ICON_CAP, cancel.clone(), flag.clone()).await
                    {
                        if looks_like_image(&bytes) {
                            return Some(bytes);
                        }
                        eprintln!("[favicon] {url} 不是图片内容，尝试下一个候选");
                    }
                }
            }
        }
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
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    config: tauri::State<'_, ConfigStore>,
    cancel_state: tauri::State<'_, FaviconCancel>,
    session_id: Option<String>,
    uuids: Option<Vec<String>>,
) -> Result<vault::FaviconReport, String> {
    let (session_id, jobs) =
        {
            let mut active = session.lock().map_err(|_| {
                eprintln!("[favicon] 数据库锁已损坏");
                "数据库锁已损坏".to_owned()
            })?;
            vaults.with_resolved_session_mut(&mut active, session_id.as_deref(), |target| {
                match &uuids {
                    Some(selected) if !selected.is_empty() => {
                        target.favicon_jobs_selected(selected).map_err(|e| {
                            eprintln!("[favicon] 收集选中条目图标任务失败: {e}");
                            e
                        })
                    }
                    _ => target.favicon_jobs().map_err(|e| {
                        eprintln!("[favicon] 收集图标任务失败: {e}");
                        e
                    }),
                }
            })?
        };
    let total = jobs.len();
    let mut done = 0usize;
    let concurrency = config
        .get()
        .map(|cfg| cfg.favicon.concurrency.max(1) as usize)
        .unwrap_or(8);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    // One client per command keeps a single connection pool for every host.
    // `reqwest::Client::clone` is cheap (Arc-backed), while rebuilding it per
    // host discards warm TLS/proxy connections and repeats proxy setup.
    // Reset cancel flag for this run; previous run's cancel must not poison the next.
    cancel_state
        .flag
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let cancel = cancel_state.notify.clone();
    let cancel_flag = cancel_state.flag.clone();
    let client = build_favicon_client();
    let mut set = tokio::task::JoinSet::new();
    for job in &jobs {
        let host = job.host.clone();
        let semaphore = semaphore.clone();
        let client = client.clone();
        let cancel = cancel.clone();
        let cancel_flag = cancel_flag.clone();
        set.spawn(async move {
            let host = host;
            // Cooperative cancel: semaphore wait also abortable so queued jobs
            // do not block "结束等待".
            let _permit = tokio::select! {
                p = semaphore.acquire_owned() => p.ok(),
                _ = cancel.notified() => None,
            };
            if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
                return (host, None);
            }
            let bytes = match client.as_ref() {
                Some(client) => fetch_favicon(client, &host, cancel, cancel_flag).await,
                None => {
                    eprintln!("[favicon] 构建 HTTP 客户端失败 ({host})");
                    None
                }
            };
            (host, bytes)
        });
    }
    let mut fetched: Vec<vault::FaviconFetch> = Vec::new();
    let mut cancelled = false;
    while !set.is_empty() {
        tokio::select! {
            result = set.join_next() => {
                let Some(result) = result else { break; };
                if let Ok((host, Some(bytes))) = result {
                    fetched.push(vault::FaviconFetch { host, bytes });
                }
                done += 1;
                let _ = app.emit(
                    "favicon-progress",
                    vault::FaviconProgress {
                        session_id: session_id.clone(),
                        done,
                        total,
                    },
                );
            }
            _ = cancel.notified() => {
                eprintln!("[favicon] 收到取消信号，中止剩余下载");
                set.abort_all();
                cancelled = true;
                // Drain aborted tasks so JoinSet is empty.
                while set.join_next().await.is_some() {}
                break;
            }
        }
    }
    if cancelled {
        let _ = app.emit(
            "favicon-progress",
            vault::FaviconProgress {
                session_id: session_id.clone(),
                done,
                total,
            },
        );
    }
    let downloaded = fetched.len();
    let attempted = jobs.len();
    let auto_save = config
        .get()
        .map(|cfg| cfg.favicon.auto_save)
        .unwrap_or(false);
    if auto_save {
        // Mutate + capture the save job under the lock, then run KDF +
        // serialization + transport off the async worker, then complete
        // under the lock again — the same split as `save_vault`. Calling
        // `session.save()` directly here would run the remote transport's
        // `Runtime::block_on` on a tokio worker thread, where it panics;
        // that panic would unwind through the MutexGuard and poison the
        // session mutex, bricking every later command with "数据库锁已损坏"
        // (see the note in `list_objects_async`).
        let _persistence = vaults.acquire_persistence_async().await?;
        let job = {
            let mut active = session.lock().map_err(|_| {
                eprintln!("[favicon] 数据库锁已损坏");
                "数据库锁已损坏".to_owned()
            })?;
            vaults.with_session_mut(&mut active, Some(&session_id), |target| {
                target.apply_favicons(&jobs, fetched).map_err(|e| {
                    eprintln!("[favicon] 写入图标失败: {e}");
                    e
                })?;
                target.prepare_save(false).map_err(|e| {
                    eprintln!("[favicon] 准备保存失败: {e}");
                    e
                })
            })?
        };
        let revision = job.revision;
        let persisted = tauri::async_runtime::spawn_blocking(move || vault::persist_save(job))
            .await
            .map_err(|e| format!("图标保存任务异常: {e}"))?;
        match persisted {
            Ok(new_hash) => {
                let mut active = session.lock().map_err(|_| {
                    eprintln!("[favicon] 数据库锁已损坏");
                    "数据库锁已损坏".to_owned()
                })?;
                vaults
                    .with_session_mut(&mut active, Some(&session_id), |target| {
                        target.complete_save(revision, new_hash)
                    })
                    .map_err(|e| {
                        eprintln!("[favicon] 完成保存失败: {e}");
                        e
                    })?;
            }
            Err(e) => {
                if !e.starts_with(vault::REMOTE_CHANGED_MARKER) {
                    if let Ok(mut active) = session.lock() {
                        let _ = vaults.with_session_mut(&mut active, Some(&session_id), |target| {
                            target.note_save_failure();
                            Ok(())
                        });
                    }
                }
                eprintln!("[favicon] 保存数据库失败: {e}");
                return Err(e);
            }
        }
    } else {
        // Manual-save mode (default): apply the icons to the open session
        // only. `apply_favicons` marks the session dirty when bytes were
        // written, so the tab shows "unsaved" until the user saves; nothing
        // touches the disk or the remote.
        let mut active = session.lock().map_err(|_| {
            eprintln!("[favicon] 数据库锁已损坏");
            "数据库锁已损坏".to_owned()
        })?;
        vaults.with_session_mut(&mut active, Some(&session_id), |target| {
            target.apply_favicons(&jobs, fetched).map_err(|e| {
                eprintln!("[favicon] 写入图标失败: {e}");
                e
            })
        })?;
    }
    Ok(vault::FaviconReport {
        attempted,
        downloaded,
    })
}
