//! Host adapters over the in-memory `VaultSession`: implement the KeePassHttp
//! `BridgeHost` and KeePassRPC `RpcHost` traits, plus the pure helper
//! functions they use (RPC/bridge tree building, URL matching, write-path
//! field application). Extracted from `vault.rs`.

use crate::bridge::{BridgeHost, BridgeLogin};
use crate::rpc::{
    merge_urls, write_custom_fields, write_password, write_username, RpcDatabase, RpcError,
    RpcGroup, RpcGroupRef, RpcHost, RpcLogin, RpcLoginWrite,
};
use crate::util::url_host;
use crate::vault::{
    entry_match_urls, kprpc_matches_url, parse_entry_id, recycle_bin_id, VaultSession,
    FIELD_PASSWORD, FIELD_TITLE, FIELD_URL, FIELD_USERNAME, ROOT_GROUP_NAME,
};
use keepass::db::{EntryId, EntryMut, GroupId, GroupRef, Value};
use keepass::Database;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Browser bridge (KeePassHttp) host
// ---------------------------------------------------------------------------

impl BridgeHost for VaultSession {
    fn is_open(&self) -> bool {
        self.is_open()
    }

    fn client_key(&self, id: &str) -> Option<Vec<u8>> {
        self.bridge_keys.get(id).cloned()
    }

    fn register_client(&mut self, id: &str, key: Vec<u8>) {
        self.bridge_keys.insert(id.to_owned(), key);
    }

    fn list_clients(&self) -> Vec<String> {
        self.bridge_keys.keys().cloned().collect()
    }

    fn remove_client(&mut self, id: &str) -> bool {
        self.bridge_keys.remove(id).is_some()
    }

    fn logins_for(&self, url: &str, submit_url: Option<&str>) -> Vec<BridgeLogin> {
        let Ok(db) = self.require_db() else {
            return Vec::new();
        };
        let bin_id = recycle_bin_id(db);
        let mut out = Vec::new();
        collect_bridge_logins(db.root(), bin_id, url, submit_url, &mut out);
        out
    }

    fn db_hash(&self) -> String {
        let Ok(db) = self.require_db() else {
            return String::new();
        };
        bridge_db_hash(db)
    }

    fn set_login(
        &mut self,
        login: &str,
        password: &str,
        url: &str,
        uuid: Option<&str>,
    ) -> Result<(), String> {
        let uuid = uuid.unwrap_or_default();
        {
            let db = self.require_db_mut()?;
            let id = parse_entry_id(uuid)?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            entry.set(FIELD_USERNAME, Value::unprotected(login.to_owned()));
            entry.set(FIELD_PASSWORD, Value::protected(password.to_owned()));
            entry.set(FIELD_URL, Value::unprotected(url.to_owned()));
        }
        self.mark_dirty();
        Ok(())
    }

    fn create_login(&mut self, login: &str, password: &str, url: &str) -> Result<(), String> {
        let title = bridge_entry_title(url);
        {
            let db = self.require_db_mut()?;
            let mut root = db.root_mut();
            let mut entry = root.add_entry();
            entry.set(FIELD_TITLE, Value::unprotected(title));
            entry.set(FIELD_USERNAME, Value::unprotected(login.to_owned()));
            entry.set(FIELD_PASSWORD, Value::protected(password.to_owned()));
            entry.set(FIELD_URL, Value::unprotected(url.to_owned()));
            entry.set_icon_none();
        }
        self.mark_dirty();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// KeePassRPC host
// ---------------------------------------------------------------------------

impl RpcHost for VaultSession {
    fn is_open(&self) -> bool {
        self.is_open()
    }

    /// The 32-byte SRP-derived session key for a Kee client username. Held
    /// only in memory; wiped by `close()` along with the master key.
    fn rpc_key(&self, username: &str) -> Option<Vec<u8>> {
        self.rpc_keys.get(username).cloned()
    }

    fn register_rpc_key(&mut self, username: &str, key: Vec<u8>) {
        self.rpc_keys.insert(username.to_owned(), key);
    }

    fn database(&self) -> Option<RpcDatabase> {
        let db = self.require_db().ok()?;
        let bin_id = recycle_bin_id(db);
        let file_name = self
            .path
            .as_deref()
            .map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p))
            .unwrap_or_default()
            .to_owned();
        Some(RpcDatabase {
            name: db
                .meta
                .database_name
                .clone()
                .unwrap_or_else(|| file_name.clone()),
            file_name,
            icon_image_data: String::new(),
            root: build_rpc_group(db.root(), bin_id, ROOT_GROUP_NAME, ""),
            active: true,
        })
    }

    fn find_logins(
        &self,
        urls: &[String],
        uuid: Option<&str>,
        free_text: Option<&str>,
        username: Option<&str>,
    ) -> Vec<RpcLogin> {
        let Ok(db) = self.require_db() else {
            return Vec::new();
        };
        let bin_id = recycle_bin_id(db);
        let filter = RpcLoginFilter {
            urls,
            uuid,
            free_text,
            username,
        };
        let mut out = Vec::new();
        collect_rpc_logins(db.root(), bin_id, &filter, ROOT_GROUP_NAME, "", &mut out);
        out
    }

    fn add_login(
        &mut self,
        login: &RpcLoginWrite,
        parent_uuid: &str,
    ) -> Result<RpcLogin, RpcError> {
        if !self.is_open() {
            return Err(RpcError::Locked);
        }
        // Resolve the parent group up front (immutable pass): unknown or
        // invalid uuids fall back to the root group, mirroring the plugin's
        // `AddLogin`; a parent inside the recycle bin is rejected like the
        // plugin's `GetRootPwGroup`.
        let parent_id = {
            let db = self.require_db().map_err(rpc_write_error)?;
            let bin_id = recycle_bin_id(db);
            Uuid::parse_str(parent_uuid)
                .ok()
                .map(GroupId::from_uuid)
                .filter(|id| find_rpc_group_id(db.root(), *id, bin_id))
        };
        let created_uuid = {
            let db = self.require_db_mut().map_err(rpc_write_error)?;
            let mut parent_group = match parent_id {
                Some(id) => match db.group_mut(id) {
                    Some(group) => group,
                    None => db.root_mut(),
                },
                None => db.root_mut(),
            };
            let mut entry = parent_group.add_entry();
            apply_login_write(&mut entry, login, &login.urls.join(" "));
            entry.set_icon_none();
            entry.id().uuid().to_string()
        };
        self.mark_dirty();
        // The extension assumes a successful AddLogin/UpdateLogin is durable
        // (KeePassRPC persists after every write); nothing in the desktop UI
        // saves on its behalf, so flush here.
        persist_after_rpc_write(self)?;
        rpc_login_by_uuid(self, &created_uuid)
            .ok_or(RpcError::InvalidMessage("新建条目读取失败".to_owned()))
    }

    fn update_login(
        &mut self,
        login: &RpcLoginWrite,
        old_uuid: &str,
        url_merge_mode: u8,
    ) -> Result<RpcLogin, RpcError> {
        if !self.is_open() {
            return Err(RpcError::Locked);
        }
        let id = parse_entry_id(old_uuid).map_err(|_| RpcError::EntryNotFound)?;
        // Resolve + merge URLs on the immutable snapshot first.
        let merged_urls = {
            let db = self.require_db().map_err(rpc_write_error)?;
            let bin_id = recycle_bin_id(db);
            let current = match find_rpc_entry_urls(db.root(), id, bin_id, false) {
                FindEntryOutcome::NotFound => return Err(RpcError::EntryNotFound),
                FindEntryOutcome::InRecycleBin => return Err(RpcError::InRecycleBin),
                FindEntryOutcome::Found(urls) => urls,
            };
            merge_urls(&current, &login.urls, url_merge_mode)
        };
        {
            let db = self.require_db_mut().map_err(rpc_write_error)?;
            let mut entry = db.entry_mut(id).ok_or(RpcError::EntryNotFound)?;
            // `edit_tracking` snapshots the pre-edit entry into its history on
            // drop — the KDBX equivalent of the plugin's `CreateBackup`.
            entry.edit_tracking(|tracked| {
                let mut this = tracked.as_mut();
                apply_login_write(&mut this, login, &merged_urls.join(" "));
            });
        }
        self.mark_dirty();
        persist_after_rpc_write(self)?;
        rpc_login_by_uuid(self, old_uuid).ok_or(RpcError::EntryNotFound)
    }
}

/// Persist the vault right after a browser-originated write (Add/UpdateLogin)
/// so the change survives a restart even when the desktop UI never saves.
fn persist_after_rpc_write(session: &mut VaultSession) -> Result<(), RpcError> {
    session
        .save()
        .map_err(|e| RpcError::InvalidMessage(format!("保存失败: {e}")))?;
    Ok(())
}

/// Read one entry by uuid as an `RpcLogin` (recycle bin skipped, like the
/// read paths); the plugin returns the updated entry the same way.
fn rpc_login_by_uuid(session: &VaultSession, uuid: &str) -> Option<RpcLogin> {
    let db = session.require_db().ok()?;
    let bin_id = recycle_bin_id(db);
    let filter = RpcLoginFilter {
        urls: &[],
        uuid: Some(uuid),
        free_text: None,
        username: None,
    };
    let mut out = Vec::new();
    collect_rpc_logins(db.root(), bin_id, &filter, ROOT_GROUP_NAME, "", &mut out);
    out.into_iter().next()
}

/// FindLogins filter criteria (mirrors the KeePassRPC parameter list).
struct RpcLoginFilter<'a> {
    urls: &'a [String],
    uuid: Option<&'a str>,
    free_text: Option<&'a str>,
    username: Option<&'a str>,
}

/// Build the full group tree DTO, root included. The recycle bin subtree is
/// excluded so credentials in it stay invisible to browsers.
fn build_rpc_group(
    group: GroupRef<'_>,
    bin_id: Option<GroupId>,
    title: &str,
    parent_path: &str,
) -> RpcGroup {
    let path = if parent_path.is_empty() {
        title.to_owned()
    } else {
        format!("{parent_path}/{title}")
    };
    RpcGroup {
        uuid: group.id().uuid().to_string(),
        title: title.to_owned(),
        path: path.clone(),
        icon_image_data: String::new(),
        entries: group
            .entries()
            .map(|entry| {
                let urls: Vec<String> = entry_match_urls(&entry);
                RpcLogin {
                    uuid: entry.id().uuid().to_string(),
                    title: entry.get_title().unwrap_or_default().to_owned(),
                    username: entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
                    // childLightEntries carry no credentials; keep them out of
                    // the tree snapshot to avoid secrets entering the browser.
                    password: String::new(),
                    urls,
                    http_realm: String::new(),
                    icon_image_data: String::new(),
                    parent_group: RpcGroupRef {
                        uuid: group.id().uuid().to_string(),
                        title: title.to_owned(),
                        path: path.clone(),
                        icon_image_data: String::new(),
                    },
                    match_accuracy: 1,
                }
            })
            .collect(),
        children: group
            .groups()
            .filter(|g| bin_id != Some(g.id()))
            .map(|g| {
                let name = g.name.clone();
                build_rpc_group(g, bin_id, &name, &path)
            })
            .collect(),
    }
}

/// Depth-first scan for KeePassRPC logins. Matching follows the extension's
/// semantics: any URL-host match, exact uuid, or title/username substring
/// (`freeText`), plus username filter; the recycle bin is skipped.
fn collect_rpc_logins(
    group: GroupRef<'_>,
    bin_id: Option<GroupId>,
    filter: &RpcLoginFilter<'_>,
    group_title: &str,
    parent_path: &str,
    out: &mut Vec<RpcLogin>,
) {
    if bin_id == Some(group.id()) {
        return;
    }
    let group_path = if parent_path.is_empty() {
        group_title.to_owned()
    } else {
        format!("{parent_path}/{group_title}")
    };
    for entry in group.entries() {
        let entry_urls = entry_match_urls(&entry);
        let by_url = filter.urls.iter().any(|u| kprpc_matches_url(&entry, u));
        let by_uuid = filter
            .uuid
            .is_some_and(|id| entry.id().uuid().to_string() == id);
        let title = entry.get_title().unwrap_or_default();
        let entry_username = entry.get(FIELD_USERNAME).unwrap_or_default();
        let by_text = filter
            .free_text
            .is_some_and(|t| !t.is_empty() && (title.contains(t) || entry_username.contains(t)));
        let by_username = filter
            .username
            .is_some_and(|u| !u.is_empty() && entry_username.contains(u));
        if by_url || by_uuid || by_text || by_username {
            out.push(RpcLogin {
                uuid: entry.id().uuid().to_string(),
                title: title.to_owned(),
                username: entry_username.to_owned(),
                password: entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned(),
                urls: entry_urls,
                http_realm: String::new(),
                icon_image_data: String::new(),
                parent_group: RpcGroupRef {
                    uuid: group.id().uuid().to_string(),
                    title: group_title.to_owned(),
                    path: group_path.clone(),
                    icon_image_data: String::new(),
                },
                match_accuracy: if by_url { 3 } else { 1 },
            });
        }
    }
    for child in group.groups() {
        let child_title = child.name.clone();
        collect_rpc_logins(child, bin_id, filter, &child_title, &group_path, out);
    }
}

/// Title for entries created by the browser bridge: the URL host, or the raw
/// URL when it has no parseable host.
fn bridge_entry_title(url: &str) -> String {
    let host = url_host(url).unwrap_or_default();
    if host.is_empty() {
        url.trim().to_owned()
    } else {
        host
    }
}

/// Map an internal vault error (e.g. "vault is locked") to a JSON-RPC error.
fn rpc_write_error(err: String) -> RpcError {
    RpcError::InvalidMessage(err)
}

/// True when `id` resolves to a group reachable from `group` without crossing
/// the recycle bin (the bin subtree is skipped, like every read path).
/// References only flow downward, so recursion stays borrow-safe.
fn find_rpc_group_id(group: GroupRef<'_>, id: GroupId, bin_id: Option<GroupId>) -> bool {
    if bin_id == Some(group.id()) {
        return false;
    }
    if group.id() == id {
        return true;
    }
    for child in group.groups() {
        if find_rpc_group_id(child, id, bin_id) {
            return true;
        }
    }
    false
}

/// Outcome of resolving an entry during the write pass.
enum FindEntryOutcome {
    NotFound,
    /// Found, but inside the recycle bin subtree (SecPivot rejects the write).
    InRecycleBin,
    /// Found outside the recycle bin, with its current URL list.
    Found(Vec<String>),
}

/// Resolve an entry by id and read its URL list (space-separated `URL` field).
/// References only flow downward, so recursion stays borrow-safe.
fn find_rpc_entry_urls(
    group: GroupRef<'_>,
    id: EntryId,
    bin_id: Option<GroupId>,
    in_bin: bool,
) -> FindEntryOutcome {
    let in_bin = in_bin || bin_id == Some(group.id());
    if let Some(entry) = group.entry(id) {
        let urls = entry_match_urls(&entry);
        if in_bin {
            FindEntryOutcome::InRecycleBin
        } else {
            FindEntryOutcome::Found(urls)
        }
    } else {
        for child in group.groups() {
            match find_rpc_entry_urls(child, id, bin_id, in_bin) {
                FindEntryOutcome::NotFound => {}
                outcome => return outcome,
            }
        }
        FindEntryOutcome::NotFound
    }
}

/// Apply Kee's `Entry` DTO to a destination entry (the plugin's
/// `setPwEntryFromEntry`, adapted to KDBX strings): title and the URL list
/// (space-joined so the read path sees every URL), first password field →
/// Password, all username fields → UserName (last wins), remaining fields →
/// custom strings named `displayName` (fallback `name`).
fn apply_login_write(entry: &mut EntryMut<'_>, login: &RpcLoginWrite, urls: &str) {
    entry.set(FIELD_TITLE, Value::unprotected(login.title.clone()));
    entry.set(FIELD_URL, Value::unprotected(urls.to_owned()));
    entry.set(FIELD_USERNAME, Value::unprotected(write_username(login)));
    entry.set(FIELD_PASSWORD, Value::protected(write_password(login)));
    for (name, value) in write_custom_fields(login) {
        entry.set(name, Value::unprotected(value));
    }
}

/// Depth-first scan for bridge logins matching the request URL (or its
/// submit URL). The recycle bin subtree is skipped entirely.
fn collect_bridge_logins(
    group: GroupRef<'_>,
    bin_id: Option<GroupId>,
    url: &str,
    submit_url: Option<&str>,
    out: &mut Vec<BridgeLogin>,
) {
    if bin_id == Some(group.id()) {
        return;
    }
    let url = url.to_lowercase();
    let submit_url = submit_url.map(str::to_lowercase);
    for entry in group.entries() {
        let matches = kprpc_matches_url(&entry, &url)
            || submit_url
                .as_deref()
                .is_some_and(|s| kprpc_matches_url(&entry, s));
        if matches {
            out.push(BridgeLogin {
                uuid: entry.id().uuid().to_string(),
                name: entry.get_title().unwrap_or_default().to_owned(),
                login: entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
                password: entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned(),
            });
        }
    }
    for child in group.groups() {
        collect_bridge_logins(child, bin_id, url.as_str(), submit_url.as_deref(), out);
    }
}

/// KeePassHttp database hash: SHA1 of (root uuid bytes + recycle-bin uuid
/// bytes), hex-encoded, as a change signal for browser extensions.
fn bridge_db_hash(db: &Database) -> String {
    use crate::crypto::{hex, sha1_bytes};
    let mut data = Vec::with_capacity(20);
    data.extend_from_slice(db.root().id().uuid().as_bytes());
    if let Some(bin) = db.meta.recyclebin_uuid {
        data.extend_from_slice(bin.as_bytes());
    }
    hex(&sha1_bytes(&data))
}
