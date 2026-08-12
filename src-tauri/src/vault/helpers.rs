//! Free functions shared by the vault session: entry/group id parsing,
//! recycle-bin helpers, OTP field resolution, auto-type match walking, and
//! database write / KDF / cipher / compression application (extracted from
//! mod.rs).

use super::{
    AES_KDF_ROUNDS, ARGON2_ITERATIONS, ARGON2_MEMORY_KIB, ARGON2_PARALLELISM, FIELD_HMAC_OTP,
    FIELD_KPRPC_CONFIG, FIELD_NOTES, FIELD_OTP, FIELD_PASSWORD, FIELD_STEAM_OTP,
    FIELD_STEAM_OTP_ALT, FIELD_TIME_OTP, FIELD_URL, FIELD_USERNAME, RESERVED_FIELDS,
    ROOT_GROUP_UUID,
};
use crate::crypto::otp;
use crate::platform::autotype;
use crate::util::url_host;
use keepass::config::{CompressionConfig, KdfConfig, OuterCipherConfig};
use keepass::db::{Entry, EntryId, GroupId, GroupRef};
use keepass::{Database, DatabaseKey};
use std::io::Cursor;
use std::path::Path;
use uuid::Uuid;
use zeroize::Zeroize;
/// Wipe a secret `String` in place, then drop it (buffer is zeroed before
/// the heap allocation is freed). Best-effort: only this owned copy is
/// cleared — copies made by the OS, IPC, or `DatabaseKey` internals are
/// outside our control (`keepass` zeroizes its own key material on drop).
pub(crate) fn wipe_secret_string(secret: &mut String) {
    secret.zeroize();
}

/// Wipe a secret byte buffer in place (see `wipe_secret_string`).
pub(crate) fn wipe_secret_bytes(secret: &mut Vec<u8>) {
    secret.zeroize();
}

/// Combine password and/or keyfile into a `DatabaseKey`. At least one
/// component must be present.
pub(crate) fn build_database_key(
    password: &str,
    keyfile: Option<&[u8]>,
) -> Result<DatabaseKey, String> {
    if password.is_empty() && keyfile.is_none() {
        return Err("主密码不能为空".to_owned());
    }
    let mut key = DatabaseKey::new();
    if !password.is_empty() {
        key = key.with_password(password);
    }
    if let Some(bytes) = keyfile {
        let mut cursor = std::io::Cursor::new(bytes);
        key = key
            .with_keyfile(&mut cursor)
            .map_err(|e| format!("读取密钥文件失败: {e}"))?;
    }
    Ok(key)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn parse_entry_id(s: &str) -> Result<EntryId, String> {
    Uuid::parse_str(s)
        .map(EntryId::from_uuid)
        .map_err(|_| format!("无效的条目 UUID: {s}"))
}

pub(crate) fn parse_group_id(s: &str) -> Result<GroupId, String> {
    Uuid::parse_str(s)
        .map(GroupId::from_uuid)
        .map_err(|_| format!("无效的分组 UUID: {s}"))
}

/// The recycle bin group id, when the database has one.
pub(crate) fn recycle_bin_id(db: &Database) -> Option<GroupId> {
    db.meta.recyclebin_uuid.map(GroupId::from_uuid)
}

/// Minimum match accuracy for a KeePassRPC entry (the plugin stores it as a
/// pair of booleans; the constructor maps them back to the enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum MatchAccuracy {
    /// `blockHostnameOnlyMatch`: only URL-exact / close matches count.
    Exact,
    /// `blockDomainOnlyMatch`: hostname-and-port matches count.
    Hostname,
    /// default (both flags false): registrable-domain matches count.
    #[default]
    Domain,
}

/// Parsed KeePassRPC per-entry config (`KPRPC JSON` custom field), the full
/// v1 shape Kee writes. Missing or malformed fields degrade to empty lists /
/// `Domain` accuracy, so the entry still matches on its primary URL.
#[derive(Debug, Default, Clone)]
pub(crate) struct KprpcConfig {
    /// Additional URLs that should match the entry (`altURLs`).
    pub alt_urls: Vec<String>,
    /// URLs that block the entry from matching (`blockedURLs`).
    pub blocked_urls: Vec<String>,
    /// Regular expressions that match the entry (`regExURLs`).
    pub regex_urls: Vec<String>,
    /// Regular expressions that block the entry (`regExBlockedURLs`).
    pub regex_blocked_urls: Vec<String>,
    pub accuracy: MatchAccuracy,
}

fn json_strings(value: &serde_json::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|u| u.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn json_bool(value: &serde_json::Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

/// Parse the full KeePassRPC per-entry config from the `KPRPC JSON` field.
/// Malformed JSON degrades to defaults; the entry then matches via its
/// primary URL only at `Domain` accuracy.
pub(crate) fn kprpc_config(entry: &keepass::db::EntryRef<'_>) -> KprpcConfig {
    let Some(raw) = entry.get(FIELD_KPRPC_CONFIG) else {
        return KprpcConfig::default();
    };
    let value: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return KprpcConfig::default(),
    };
    let hostname_only = json_bool(&value, "blockHostnameOnlyMatch");
    let domain_only = json_bool(&value, "blockDomainOnlyMatch");
    let accuracy = if hostname_only {
        MatchAccuracy::Exact
    } else if domain_only {
        MatchAccuracy::Hostname
    } else {
        MatchAccuracy::Domain
    };
    KprpcConfig {
        alt_urls: json_strings(&value, "altURLs"),
        blocked_urls: json_strings(&value, "blockedURLs"),
        regex_urls: json_strings(&value, "regExURLs"),
        regex_blocked_urls: json_strings(&value, "regExBlockedURLs"),
        accuracy,
    }
}

/// 3-tier URL match mirroring KeePassRPC's `BestMatchAccuracyForAnyURL`
/// (abridged: no path/port wildcards). Returns `true` when the request URL is
/// at least as similar as `min` requires.
/// - `Exact`: the two URLs are equal discounting scheme/query/port-noise.
/// - `Hostname`: same host:port (subdomains never spill).
/// - `Domain`: same host, or one is a subdomain of the other — or, when
///   `registrable` is set (config `rpc.matchByRegistrableDomain`), the same
///   registrable domain (PSL), so sibling hosts under one domain match.
fn url_matches_accuracy(
    entry_url: &str,
    request_url: &str,
    min: MatchAccuracy,
    registrable: bool,
) -> bool {
    let entry_host = url_host(entry_url).unwrap_or_default();
    let request_host = url_host(request_url).unwrap_or_default();
    if entry_host.is_empty() || request_host.is_empty() {
        return false;
    }
    let host_equal = entry_host == request_host;
    let subdomain_cover = request_host.ends_with(&format!(".{entry_host}"))
        || entry_host.ends_with(&format!(".{request_host}"));
    match min {
        MatchAccuracy::Exact => {
            // Best/close match: identical normalized hosts, plus the same path.
            host_equal && strip_query(entry_url) == strip_query(request_url)
        }
        MatchAccuracy::Hostname => host_equal,
        MatchAccuracy::Domain => {
            if registrable {
                registrable_domain(&entry_host) == registrable_domain(&request_host)
            } else {
                host_equal || subdomain_cover
            }
        }
    }
}

/// Path part of a URL (host + port + path + query, no scheme), lower-cased, so
/// two URLs sharing a host and path compare at `Exact` accuracy.
fn strip_query(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let mut end = rest.len();
    if let Some(i) = rest.find('?') {
        end = end.min(i);
    }
    if let Some(i) = rest.find('#') {
        end = end.min(i);
    }
    rest[..end].to_ascii_lowercase()
}

/// Multi-label public suffixes that must not be stripped to the last label,
/// so the registrable domain is the third label from the end (e.g. `co.uk`).
const MULTI_LABEL_SUFFIX: [&str; 41] = [
    "ac.uk", "co.uk", "gov.uk", "org.uk", "net.uk", "me.uk", "ltd.uk", "plc.uk", "com.cn",
    "net.cn", "org.cn", "gov.cn", "edu.cn", "com.hk", "com.mo", "com.tw", "co.jp", "ne.jp",
    "or.jp", "ac.jp", "go.jp", "com.sg", "com.my", "com.vn", "co.za", "org.za", "net.za", "com.au",
    "net.au", "org.au", "co.nz", "com.br", "com.mx", "co.kr", "or.kr", "ne.kr", "ac.in", "gov.in",
    "co.in", "net.in", "org.in",
];

/// Registrable domain of a `host`, mirroring KeePassRPC's PSL-based `Domain`
/// match: the public-suffix label(s) plus one more label. Used when the app
/// is configured to match by registrable domain, so `account.aliyun.com` and
/// `passport.aliyun.com` share `aliyun.com`. Unknown suffixes fall back to the
/// last two labels; already-short hosts are returned unchanged.
fn registrable_domain(host: &str) -> String {
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() <= 2 {
        return host.to_owned();
    }
    let suffix = labels[labels.len() - 2..].join(".");
    if MULTI_LABEL_SUFFIX.contains(&suffix.as_str()) {
        labels[labels.len() - 3..].join(".")
    } else {
        // Most hosts: last two labels are the registrable domain.
        suffix
    }
}

/// Compile a user regex; malformed patterns simply never match (Kee ignores
/// them too rather than failing the whole login lookup).
fn regex_matches(pattern: &str, text: &str) -> bool {
    regex::Regex::new(pattern)
        .ok()
        .is_some_and(|re| re.is_match(text))
}

/// Whether a request URL falls under a blocked URL: the blocked host blocks
/// itself and everything below it (`blog.example.com` blocks that host only,
/// never the parent `example.com`). Mirrors KeePassRPC's blocked-URL
/// semantics where the stored rule is the "at-or-under" boundary.
fn host_at_or_below(blocked_url: &str, request_url: &str) -> bool {
    let blocked_host = url_host(blocked_url).unwrap_or_default();
    let request_host = url_host(request_url).unwrap_or_default();
    if blocked_host.is_empty() || request_host.is_empty() {
        return false;
    }
    request_host == blocked_host || request_host.ends_with(&format!(".{blocked_host}"))
}

/// Whether `request_url` matches the entry under its full KeePassRPC rules:
/// blocked lists take precedence, then regex match URLs, then the host-tier
/// match over the primary URL + `altURLs`. This is the single source of truth
/// for bridge, RPC, and auto-type URL matching. `registrable` selects whether
/// the Domain tier compares registrable domains (KeePassRPC's PSL behavior)
/// or strict host/subdomain.
pub(crate) fn kprpc_matches_url(
    entry: &keepass::db::EntryRef<'_>,
    request_url: &str,
    registrable: bool,
) -> bool {
    let cfg = kprpc_config(entry);
    // Blocked first: either list vetoes the match regardless of anything else.
    if cfg
        .blocked_urls
        .iter()
        .any(|b| host_at_or_below(b, request_url))
        || cfg
            .regex_blocked_urls
            .iter()
            .any(|r| regex_matches(r, request_url))
    {
        return false;
    }
    // A regex match URL wins even if the host tier would miss.
    if cfg.regex_urls.iter().any(|r| regex_matches(r, request_url)) {
        return true;
    }
    // Otherwise the amount of host/path similarity decides under `accuracy`.
    let mut urls: Vec<String> = entry_primary_url(entry)
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    urls.extend(cfg.alt_urls.iter().cloned());
    urls.iter()
        .any(|u| url_matches_accuracy(u, request_url, cfg.accuracy, registrable))
}

/// The entry's effective primary URL for matching: the `OverrideURL` field
/// when set, else the `URL` field (KeePass semantics — `OverrideURL` replaces
/// `URL` for auto-type window detection and browser/bridge matching).
pub(crate) fn entry_primary_url<'a>(entry: &'a keepass::db::EntryRef<'_>) -> &'a str {
    entry
        .override_url
        .as_deref()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| entry.get(FIELD_URL).unwrap_or_default())
}

/// Every URL an entry exposes to browser-bridge and auto-type matching (for
/// DTO `url` lists and title scoring): the effective primary URL
/// (`OverrideURL` when set, else the `URL` field, space-separated) plus any
/// KeePassRPC `altURLs`. Entries edited in Kee match their alternative URLs
/// too; empty entries (no URL, no altURLs) never match.
pub(crate) fn entry_match_urls(entry: &keepass::db::EntryRef<'_>) -> Vec<String> {
    let mut urls: Vec<String> = entry_primary_url(entry)
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    urls.extend(kprpc_config(entry).alt_urls);
    urls
}

/// Depth-first scan of `group`'s subtree for a `{REF:...}` match. First
/// matching entry wins (KeePass semantics); the recycle bin is skipped.
pub(crate) fn walk_ref_match(
    group: GroupRef<'_>,
    bin_id: Option<GroupId>,
    spec: autotype::RefSpec<'_>,
    out: &mut Option<String>,
) {
    if bin_id == Some(group.id()) {
        return;
    }
    // KeePass: groups with searching disabled contribute no entries to
    // searches; descendants each carry their own flag, so keep walking.
    let searchable = group.enable_searching != Some(false);
    let needle = spec.text.to_lowercase();
    for entry in group.entries() {
        if !searchable {
            break;
        }
        let matched = match spec.search.to_ascii_uppercase().as_str() {
            "T" => entry
                .get_title()
                .unwrap_or_default()
                .to_lowercase()
                .contains(&needle),
            "U" => entry
                .get(FIELD_USERNAME)
                .unwrap_or_default()
                .to_lowercase()
                .contains(&needle),
            "P" => entry
                .get(FIELD_PASSWORD)
                .unwrap_or_default()
                .to_lowercase()
                .contains(&needle),
            "A" => entry
                .get(FIELD_URL)
                .unwrap_or_default()
                .to_lowercase()
                .contains(&needle),
            "N" => entry
                .get(FIELD_NOTES)
                .unwrap_or_default()
                .to_lowercase()
                .contains(&needle),
            "I" => entry
                .id()
                .uuid()
                .to_string()
                .replace('-', "")
                .to_lowercase()
                .contains(&needle.replace('-', "")),
            "O" => entry.fields.keys().any(|name| {
                !RESERVED_FIELDS.contains(&name.as_str())
                    && name.to_lowercase() == spec.text.to_lowercase()
            }),
            _ => false,
        };
        if matched {
            let value = match spec.field.to_ascii_uppercase().as_str() {
                "T" => entry.get_title().unwrap_or_default().to_owned(),
                "U" => entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
                "P" => entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned(),
                "A" => entry.get(FIELD_URL).unwrap_or_default().to_owned(),
                "N" => entry.get(FIELD_NOTES).unwrap_or_default().to_owned(),
                "I" => entry.id().uuid().to_string(),
                _ => return,
            };
            *out = Some(value);
            return;
        }
    }
    for child in group.groups() {
        walk_ref_match(child, bin_id, spec, out);
    }
}

/// Depth-first scan of `group`'s subtree for an auto-type match; the recycle
/// bin subtree is skipped entirely.
pub(crate) fn walk_match(
    group: GroupRef<'_>,
    bin_id: Option<GroupId>,
    window_title: &str,
    best: &mut Option<(i32, String)>,
) {
    if bin_id == Some(group.id()) {
        return;
    }
    // KeePass: groups with searching disabled contribute no entries to
    // auto-type matching; descendants each carry their own flag.
    let searchable = group.enable_searching != Some(false);
    for entry in group.entries() {
        if !searchable {
            break;
        }
        let mut score = 0;
        // Blocked lists veto the entry regardless of the window title.
        let cfg = kprpc_config(&entry);
        let blocked = cfg.blocked_urls.iter().any(|b| {
            let host = url_host(b).unwrap_or_default();
            !host.is_empty() && window_title.contains(&host)
        }) || cfg
            .regex_blocked_urls
            .iter()
            .any(|r| regex_matches(r, window_title));
        if blocked {
            continue;
        }
        // A regex match URL counts as a URL hit against the raw title.
        let url_hit = cfg
            .regex_urls
            .iter()
            .any(|r| regex_matches(r, window_title))
            || entry_match_urls(&entry).iter().any(|u| {
                let host = url_host(u).unwrap_or_default();
                !host.is_empty() && window_title.contains(&host)
            });
        if url_hit {
            score += 2;
        }
        let title = entry.get_title().unwrap_or_default().to_lowercase();
        if !title.is_empty() && window_title.contains(&title) {
            score += 1;
        }
        if score > 0 && best.as_ref().is_none_or(|(s, _)| score > *s) {
            *best = Some((score, entry.id().uuid().to_string()));
        }
    }
    for child in group.groups() {
        walk_match(child, bin_id, window_title, best);
    }
}

/// Same matching semantics as `walk_match`, but collects every scoring entry
/// (not just the best) so the global-hotkey handler can offer a picker when
/// several entries match the focused window.
pub(crate) fn walk_match_candidates(
    group: GroupRef<'_>,
    bin_id: Option<GroupId>,
    window_title: &str,
    out: &mut Vec<(i32, String)>,
) {
    if bin_id == Some(group.id()) {
        return;
    }
    let searchable = group.enable_searching != Some(false);
    for entry in group.entries() {
        if !searchable {
            break;
        }
        let mut score = 0;
        let cfg = kprpc_config(&entry);
        let blocked = cfg.blocked_urls.iter().any(|b| {
            let host = url_host(b).unwrap_or_default();
            !host.is_empty() && window_title.contains(&host)
        }) || cfg
            .regex_blocked_urls
            .iter()
            .any(|r| regex_matches(r, window_title));
        if blocked {
            continue;
        }
        let url_hit = cfg
            .regex_urls
            .iter()
            .any(|r| regex_matches(r, window_title))
            || entry_match_urls(&entry).iter().any(|u| {
                let host = url_host(u).unwrap_or_default();
                !host.is_empty() && window_title.contains(&host)
            });
        if url_hit {
            score += 2;
        }
        let title = entry.get_title().unwrap_or_default().to_lowercase();
        if !title.is_empty() && window_title.contains(&title) {
            score += 1;
        }
        if score > 0 {
            out.push((score, entry.id().uuid().to_string()));
        }
    }
    for child in group.groups() {
        walk_match_candidates(child, bin_id, window_title, out);
    }
}

/// Return the recycle bin group id, creating the group under root on first use.
pub(crate) fn ensure_recycle_bin(db: &mut Database) -> Result<GroupId, String> {
    if let Some(id) = recycle_bin_id(db) {
        if db.group(id).is_some() {
            return Ok(id);
        }
    }
    let bin_id = {
        let mut root = db.root_mut();
        let mut bin = root.add_group();
        bin.name = "回收站".to_owned();
        bin.id()
    };
    db.meta.recyclebin_uuid = Some(bin_id.uuid());
    Ok(bin_id)
}

/// Whether `group_id` is `ancestor` itself or nested inside it.
pub(crate) fn group_contains(db: &Database, ancestor: GroupId, group_id: GroupId) -> bool {
    let mut current = Some(group_id);
    while let Some(id) = current {
        if id == ancestor {
            return true;
        }
        current = db
            .group(id)
            .and_then(|group| group.parent().map(|p| p.id()));
    }
    false
}

/// Map the virtual `"root"` id to the DB root group id, validating the rest.
pub(crate) fn resolve_group_id(db: &Database, uuid: &str) -> Result<GroupId, String> {
    if uuid == ROOT_GROUP_UUID {
        return Ok(db.root().id());
    }
    let id = parse_group_id(uuid)?;
    if db.group(id).is_some() {
        Ok(id)
    } else {
        Err("目标分组不存在".to_owned())
    }
}

/// Find the first non-empty OTP seed field on an entry, in KeeOtp priority
/// order: `HmacOtp` (HOTP), `SteamOtp`/`steam` (Steam Guard), then the TOTP
/// forms `otp`/`TimeOtp`. Returns the field name and its raw value.
fn entry_otp_field(entry: &Entry) -> Option<(&'static str, &str)> {
    const ORDER: [&str; 5] = [
        FIELD_HMAC_OTP,
        FIELD_STEAM_OTP,
        FIELD_STEAM_OTP_ALT,
        FIELD_TIME_OTP,
        FIELD_OTP,
    ];
    for name in ORDER {
        if let Some(value) = entry.get(name) {
            if !value.is_empty() {
                return Some((name, value));
            }
        }
    }
    None
}

/// Whether the entry carries any OTP seed (TOTP, HOTP or Steam). Used by the
/// snapshot builder in `vault_serialize`.
pub(crate) fn entry_has_otp(entry: &Entry) -> bool {
    entry_otp_field(entry).is_some()
}

/// Resolve an entry's OTP seed into a computation spec, picking the parser by
/// the field that actually holds the seed.
pub(crate) fn parse_entry_otp_spec(entry: &Entry) -> Result<otp::OtpSpec, String> {
    let (field, value) = entry_otp_field(entry).ok_or_else(|| "该条目没有 OTP 种子".to_owned())?;
    match field {
        FIELD_HMAC_OTP => otp::parse_hotp_seed(value),
        FIELD_STEAM_OTP | FIELD_STEAM_OTP_ALT => otp::parse_steam_seed(value),
        _ => otp::parse_totp_seed(value),
    }
}

pub(crate) fn otp_kind_name(kind: otp::OtpKind) -> &'static str {
    match kind {
        otp::OtpKind::Totp => "totp",
        otp::OtpKind::Hotp => "hotp",
        otp::OtpKind::Steam => "steam",
    }
}

pub(crate) fn classify_open_error<E: std::fmt::Display>(e: E) -> String {
    let msg = format!("{e}");
    let lower = msg.to_lowercase();
    if lower.contains("hmac")
        || lower.contains("mac")
        || lower.contains("decrypt")
        || lower.contains("padding")
        || lower.contains("key")
    {
        "无法打开数据库: 密码或密钥文件错误".to_owned()
    } else {
        format!(
            "无法打开数据库: {msg}；若文件损坏，可在 KeePass 中导出为 XML 后通过「导入 XML」恢复"
        )
    }
}

/// Result of a cheap header probe of a vault file (no decryption attempted).
pub(crate) struct VaultProbe {
    pub kind: &'static str,
    pub note: String,
}

/// KeePass signature bytes: `03 D9 A2 9A` + version marker `67 FB 4B B5`
/// (KDBX) or `65 FB 4B B5` (legacy KDB).
const KDBX_SIGNATURE: [u8; 8] = [0x03, 0xD9, 0xA2, 0x9A, 0x67, 0xFB, 0x4B, 0xB5];
const KDB_SIGNATURE: [u8; 8] = [0x03, 0xD9, 0xA2, 0x9A, 0x65, 0xFB, 0x4B, 0xB5];

/// Inspect a file's header and size without attempting decryption: classifies
/// it as `kdbx`, `kdb` or `unknown`. Missing/unreadable files are errors.
pub(crate) fn probe_vault(path: &Path) -> Result<VaultProbe, String> {
    let data = std::fs::read(path).map_err(|e| format!("无法读取数据库文件: {e}"))?;
    let head: [u8; 8] = data
        .get(..8)
        .and_then(|slice| slice.try_into().ok())
        .unwrap_or_default();
    let size = data.len() as u64;
    if head == KDBX_SIGNATURE {
        Ok(VaultProbe {
            kind: "kdbx",
            note: format!("KeePass 数据库文件（KDBX，大小 {size} 字节）"),
        })
    } else if head == KDB_SIGNATURE {
        Ok(VaultProbe {
            kind: "kdb",
            note: format!("KeePass 1.x 数据库（KDB，暂不支持打开，大小 {size} 字节）"),
        })
    } else {
        Ok(VaultProbe {
            kind: "unknown",
            note: format!(
                "不是 KeePass 数据库文件（缺少 KDBX 头，大小 {size} 字节），可能已损坏或选择了错误文件"
            ),
        })
    }
}

pub(crate) fn save_database(db: &Database, path: &Path, key: DatabaseKey) -> Result<(), String> {
    let mut buffer = Vec::new();
    db.save(&mut Cursor::new(&mut buffer), key)
        .map_err(|e| format!("序列化数据库失败: {e}"))?;
    write_database_bytes(path, &buffer)
}

/// Atomic write of already-serialized KDBX bytes (local vault save).
pub(crate) fn write_database_bytes(path: &Path, buffer: &[u8]) -> Result<(), String> {
    crate::util::atomic_write(path, buffer, "数据库")
}

pub(crate) fn apply_kdf(db: &mut Database, kdf: &str) -> Result<(), String> {
    db.config.kdf_config = match kdf {
        "Argon2id" => KdfConfig::Argon2id {
            iterations: ARGON2_ITERATIONS,
            memory: u64::from(ARGON2_MEMORY_KIB) * 1024,
            parallelism: ARGON2_PARALLELISM,
            version: argon2::Version::Version13,
        },
        "Argon2" => KdfConfig::Argon2 {
            iterations: ARGON2_ITERATIONS,
            memory: u64::from(ARGON2_MEMORY_KIB) * 1024,
            parallelism: ARGON2_PARALLELISM,
            version: argon2::Version::Version13,
        },
        "Aes" => KdfConfig::Aes {
            rounds: AES_KDF_ROUNDS,
        },
        other => {
            return Err(format!(
                "kdf 取值 {other:?} 不受支持 (可用: Argon2id / Argon2 / Aes)"
            ))
        }
    };
    Ok(())
}

pub(crate) fn apply_cipher(db: &mut Database, cipher: &str) -> Result<(), String> {
    db.config.outer_cipher_config = match cipher {
        "Aes256" => OuterCipherConfig::AES256,
        "ChaCha20" => OuterCipherConfig::ChaCha20,
        other => {
            return Err(format!(
                "cipher 取值 {other:?} 不受支持 (可用: Aes256 / ChaCha20)"
            ))
        }
    };
    Ok(())
}

pub(crate) fn apply_compression(db: &mut Database, compression: &str) -> Result<(), String> {
    db.config.compression_config = match compression {
        "None" => CompressionConfig::None,
        "Gzip" => CompressionConfig::GZip,
        other => {
            return Err(format!(
                "compression 取值 {other:?} 不受支持 (可用: None / Gzip)"
            ))
        }
    };
    Ok(())
}
