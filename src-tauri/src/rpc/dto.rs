//! RPC wire shapes + host abstraction (KeePassRPC v1 camelCase DTOs).
//! Contents extracted from `rpc::mod.rs`; see its module doc for the protocol.
use serde::{Deserialize, Serialize};

use super::PROTOCOL_VERSION;
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Envelope {
    pub protocol: String,
    pub srp: Option<SrpMessage>,
    pub key: Option<KeyMessage>,
    pub jsonrpc: Option<JsonRpcFrame>,
    pub error: Option<ErrorMessage>,
    pub version: u32,
    pub features: Option<Vec<String>>,
    pub client_type_id: Option<String>,
    pub client_display_name: Option<String>,
    pub client_display_description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SrpMessage {
    pub stage: Option<String>,
    #[serde(rename = "I")]
    pub i: Option<String>,
    #[serde(rename = "A")]
    pub a: Option<String>,
    pub s: Option<String>,
    #[serde(rename = "B")]
    pub b: Option<String>,
    #[serde(rename = "M")]
    pub m: Option<String>,
    #[serde(rename = "M2")]
    pub m2: Option<String>,
    pub security_level: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct KeyMessage {
    pub username: Option<String>,
    pub sc: Option<String>,
    pub cc: Option<String>,
    pub cr: Option<String>,
    pub sr: Option<String>,
    pub security_level: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ErrorMessage {
    pub code: String,
    pub message_params: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JsonRpcFrame {
    pub message: String,
    pub iv: String,
    pub hmac: String,
}

impl Envelope {
    /// Server-side setup envelope (`protocol: "setup"`).
    pub fn setup() -> Self {
        Self {
            protocol: "setup".to_owned(),
            version: PROTOCOL_VERSION,
            ..Default::default()
        }
    }

    /// Server-side jsonrpc envelope carrying an encrypted frame.
    pub fn jsonrpc(frame: JsonRpcFrame) -> Self {
        Self {
            protocol: "jsonrpc".to_owned(),
            jsonrpc: Some(frame),
            version: PROTOCOL_VERSION,
            ..Default::default()
        }
    }
}
// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum RpcError {
    /// Malformed envelope/frame �?answer `INVALID_MESSAGE`.
    InvalidMessage(String),
    /// Authentication failed �?answer `AUTH_FAILED`.
    AuthFailed,
    /// Method not implemented yet �?answer a JSON-RPC error.
    Unsupported(String),
    /// Vault is not open �?answer a JSON-RPC error.
    Locked,
    /// `oldLoginUUID` did not resolve to an entry (KeePassRPC's exception).
    EntryNotFound,
    /// Target entry lives in the recycle bin; SecPivot policy rejects the
    /// write (Kee's read paths never surface recycled entries, so this is
    /// unreachable through the extension and purely defense-in-depth).
    InRecycleBin,
}
// ---------------------------------------------------------------------------
// Host abstraction + DTOs (camelCase wire shapes)
// ---------------------------------------------------------------------------

/// One credential the bridge hands to the browser; plaintext exists only
/// inside an encrypted JSON-RPC frame.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcLogin {
    pub uuid: String,
    pub title: String,
    pub username: String,
    pub password: String,
    pub urls: Vec<String>,
    pub http_realm: String,
    pub icon_image_data: String,
    pub parent_group: RpcGroupRef,
    /// KeePassRPC MatchAccuracy: 1 = Best (exact URL), 3 = HostnameAndPort.
    pub match_accuracy: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcGroupRef {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub icon_image_data: String,
}

#[derive(Debug, Clone)]
pub struct RpcGroup {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub icon_image_data: String,
    pub entries: Vec<RpcLogin>,
    pub children: Vec<RpcGroup>,
}

#[derive(Debug, Clone)]
pub struct RpcDatabase {
    pub name: String,
    pub file_name: String,
    pub icon_image_data: String,
    pub root: RpcGroup,
    pub active: bool,
}

/// The vault-session subset the RPC bridge needs. Implemented by `VaultSession`
/// (vault.rs) so dispatch stays independent of vault internals.
pub trait RpcHost {
    fn is_open(&self) -> bool;
    /// Raw 32-byte session key for a client username (wiped on lock).
    fn rpc_key(&self, username: &str) -> Option<Vec<u8>>;
    fn register_rpc_key(&mut self, username: &str, key: Vec<u8>);
    fn database(&self) -> Option<RpcDatabase>;
    /// Find logins by URL host match (recycle bin skipped), uuid, or
    /// free-text/username search �?one or more criteria may be active.
    fn find_logins(
        &self,
        urls: &[String],
        uuid: Option<&str>,
        free_text: Option<&str>,
        username: Option<&str>,
    ) -> Vec<RpcLogin>;
    /// Create an entry under `parent_uuid` (empty or unresolvable �?root
    /// group) and return it. Implements KeePassRPC `AddLogin`.
    fn add_login(&mut self, login: &RpcLoginWrite, parent_uuid: &str)
        -> Result<RpcLogin, RpcError>;
    /// Update the entry identified by `old_uuid` with `login`, merging URLs
    /// per `url_merge_mode` (KeePassRPC `UpdateLogin` + `MergeEntries`).
    fn update_login(
        &mut self,
        login: &RpcLoginWrite,
        old_uuid: &str,
        url_merge_mode: u8,
    ) -> Result<RpcLogin, RpcError>;
}
// ---------------------------------------------------------------------------
// Write-path shapes (Kee 4.0.7 `Entry.toKPRPCEntryDTO` + `Field` DTOs)
// ---------------------------------------------------------------------------

/// One `formFieldList` item sent by Kee. Types mirror the extension's
/// `FormFieldTypeDTO`: `FFTusername` / `FFTpassword` / `FFTtext` /
/// `FFTradio` / `FFTcheckbox` / `FFTselect`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RpcFieldWrite {
    pub id: String,
    pub name: String,
    pub display_name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub value: String,
    pub page: i64,
}

/// Entry data sent by Kee's `AddLogin`/`UpdateLogin` (v1 `Entry` DTO). The
/// username/password live inside `formFieldList`; extra fields are mapped by
/// name, mirroring the plugin's `setPwEntryFromEntry`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RpcLoginWrite {
    pub title: String,
    #[serde(rename = "uRLs")]
    pub urls: Vec<String>,
    #[serde(rename = "hTTPRealm")]
    pub http_realm: String,
    pub icon_image_data: String,
    pub form_field_list: Vec<RpcFieldWrite>,
}

/// KeePassRPC username convention: every `FFTusername` field maps to `UserName`
/// (the plugin writes all of them, last one wins).
pub fn write_username(login: &RpcLoginWrite) -> String {
    login
        .form_field_list
        .iter()
        .rev()
        .find(|f| f.field_type == "FFTusername")
        .map(|f| f.value.clone())
        .unwrap_or_default()
}

/// KeePassRPC password convention: the first `FFTpassword` field maps to
/// `Password`; later ones fall through to custom fields.
pub fn write_password(login: &RpcLoginWrite) -> String {
    login
        .form_field_list
        .iter()
        .find(|f| f.field_type == "FFTpassword")
        .map(|f| f.value.clone())
        .unwrap_or_default()
}

/// Extra form fields (everything but the consumed username/password fields),
/// named `displayName` (fallback `name`), mirroring `setPwEntryFromEntry`.
pub fn write_custom_fields(login: &RpcLoginWrite) -> Vec<(String, String)> {
    let mut seen_password = false;
    let mut out = Vec::new();
    for field in &login.form_field_list {
        match field.field_type.as_str() {
            "FFTusername" => {}
            "FFTpassword" if !seen_password => {
                seen_password = true;
            }
            _ => {
                let name = if field.display_name.is_empty() {
                    field.name.clone()
                } else {
                    field.display_name.clone()
                };
                if !name.is_empty() {
                    out.push((name, field.value.clone()));
                }
            }
        }
    }
    out
}
// ---------------------------------------------------------------------------
// URL merging (KeePassRPC `MergeEntries` / `MergeInNewURLs` semantics)
// ---------------------------------------------------------------------------

/// C# `MergeInNewURLs(destURLs, sourceURLs)`: source URLs are walked backwards
/// and inserted at the front when missing; the source's primary URL is promoted
/// to the front when already present (so it stays the primary match).
fn merge_in_new_urls(dest: &mut Vec<String>, source: &[String]) {
    for i in (0..source.len()).rev() {
        let url = &source[i];
        if let Some(pos) = dest.iter().position(|d| d == url) {
            if i == 0 {
                dest.remove(pos);
                dest.insert(0, url.clone());
            }
        } else {
            dest.insert(0, url.clone());
        }
    }
}

/// Apply KeePassRPC `MergeEntries` URL merging to a destination URL list
/// (`[primary, alt...]`) with a source list (`[primary, alt...]`). Modes mirror
/// the plugin's `urlMergeMode`:
/// 1 = merge source URLs in (old URLs kept, new ones first, still matchable);
/// 2 = delete the old primary URL, then merge;
/// 3 = keep old URLs, append only source URLs not already present;
/// 4 = leave URLs unchanged;
/// 5 = replace the whole list with the source URLs.
/// Unknown modes behave like 4 (the plugin's switch has no default).
pub fn merge_urls(dest: &[String], source: &[String], mode: u8) -> Vec<String> {
    let mut dest = dest.to_vec();
    match mode {
        1 => merge_in_new_urls(&mut dest, source),
        2 => {
            if !dest.is_empty() {
                dest.remove(0);
            }
            merge_in_new_urls(&mut dest, source);
        }
        3 => {
            for url in source {
                if !dest.contains(url) {
                    dest.push(url.clone());
                }
            }
        }
        4 => {}
        5 => dest = source.to_vec(),
        _ => {}
    }
    dest
}
