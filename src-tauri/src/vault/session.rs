//! `VaultSession` lifecycle: open/create/close/save/change-key/save-as plus
//! the lock-free prepare/persist handoff internals (extracted from mod.rs).

use super::helpers::{
    apply_cipher, apply_compression, apply_kdf, wipe_secret_bytes, wipe_secret_string,
};
use super::persist::{
    persist_change, persist_save, prepare_local_create, prepare_local_open, prepare_remote_create,
    prepare_remote_open, read_keyfile, SaveJob, SaveTarget,
};
use super::serialize::{build_group_tree, custom_data_entries, icon_to_data_url, now_iso};
use super::*;
use crate::remote::{RemoteStorage, REMOTE_URI_PREFIX};
use keepass::config::{CompressionConfig, KdfConfig, OuterCipherConfig};
use keepass::Database;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

impl VaultSession {
    pub fn is_open(&self) -> bool {
        self.db.is_some()
    }

    /// Cheap per-tab summary for the tab bar: `(path, file name, dirty)`.
    pub fn tab_summary(&self) -> Option<(String, String, bool)> {
        let path = self.path.as_ref()?;
        let file_name = Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        Some((path.clone(), file_name, self.dirty))
    }

    /// Decrypt and open an existing `.kdbx`, optionally with a keyfile.
    pub fn open(
        &mut self,
        path: &Path,
        password: &str,
        keyfile: Option<&Path>,
    ) -> Result<VaultState, String> {
        let (db, keyfile_bytes) = prepare_local_open(path, password, keyfile)?;
        self.adopt_local(db, path, password, keyfile_bytes)
    }

    /// Create an empty vault with the given KDF / cipher / compression and
    /// persist it immediately. A keyfile may be attached as a second factor.
    pub fn create(
        &mut self,
        path: &Path,
        password: &str,
        kdf: &str,
        cipher: &str,
        compression: &str,
        keyfile: Option<&Path>,
    ) -> Result<VaultState, String> {
        let (db, keyfile_bytes) =
            prepare_local_create(path, password, kdf, cipher, compression, keyfile)?;
        self.adopt_local(db, path, password, keyfile_bytes)
    }

    /// Open an existing vault stored on S3. `key` is the object key (e.g.
    /// `vaults/private.kdbx`); the display path becomes `s3://<key>`.
    #[allow(clippy::too_many_arguments)]
    pub fn open_remote(
        &mut self,
        storage: Arc<dyn RemoteStorage>,
        key: &str,
        password: &str,
        keyfile: Option<&Path>,
        mode: RemoteMode,
        local_dir: &Path,
        backup_count: usize,
        backup_template: &str,
    ) -> Result<VaultState, String> {
        let (db, keyfile_bytes, key) = prepare_remote_open(
            storage.clone(),
            key,
            password,
            keyfile,
            mode,
            local_dir,
            backup_count,
            backup_template,
        )?;
        self.adopt_remote(
            db,
            storage,
            &key,
            password,
            keyfile_bytes,
            mode,
            local_dir,
            backup_count,
            backup_template,
        )
    }

    /// Create an empty vault and upload it to S3 immediately.
    #[allow(clippy::too_many_arguments)]
    pub fn create_remote(
        &mut self,
        storage: Arc<dyn RemoteStorage>,
        key: &str,
        password: &str,
        kdf: &str,
        cipher: &str,
        compression: &str,
        keyfile: Option<&Path>,
        mode: RemoteMode,
        local_dir: &Path,
        backup_count: usize,
        backup_template: &str,
    ) -> Result<VaultState, String> {
        let (db, keyfile_bytes, key) = prepare_remote_create(
            storage.clone(),
            key,
            password,
            kdf,
            cipher,
            compression,
            keyfile,
            mode,
            local_dir,
            backup_count,
            backup_template,
        )?;
        self.adopt_remote(
            db,
            storage,
            &key,
            password,
            keyfile_bytes,
            mode,
            local_dir,
            backup_count,
            backup_template,
        )
    }

    pub fn close(&mut self) {
        self.close_impl(true);
    }

    /// Lock the vault but keep the KeePassRPC SRP session keys in memory, so a
    /// freshly unlocked vault is reused by the Kee extension without a new
    /// side-channel password (official KeePassRPC behavior, opt-in via
    /// `rpc.keep_session_after_lock`). Master password, keyfile, and
    /// KeePassHttp bridge keys are still wiped — only the RPC keys survive,
    /// and the server answers "locked" for data requests while the vault is
    /// closed.
    pub fn close_keeping_rpc_session(&mut self) {
        self.close_impl(false);
    }

    fn close_impl(&mut self, wipe_rpc_keys: bool) {
        self.path = None;
        // Wipe secret material before dropping the buffers: setting `None`
        // alone leaves the master password and keyfile contents on the heap.
        if let Some(mut password) = self.password.take() {
            wipe_secret_string(&mut password);
        }
        if let Some(mut keyfile) = self.keyfile.take() {
            wipe_secret_bytes(&mut keyfile);
        }
        for (_, mut key) in self.bridge_keys.drain() {
            wipe_secret_bytes(&mut key);
        }
        if wipe_rpc_keys {
            for (_, mut key) in self.rpc_keys.drain() {
                wipe_secret_bytes(&mut key);
            }
        }
        self.db = None;
        self.dirty = false;
        self.modified_at.clear();
        self.remote = None;
        self.cached_snapshot = None;
        self.pending_autotype_window = None;
    }

    pub fn state(&mut self) -> Result<Option<VaultState>, String> {
        if !self.is_open() {
            return Ok(None);
        }
        Ok(Some(self.snapshot()?))
    }

    /// Read the open database's storage settings (KDF, cipher, compression,
    /// history cap, recycle-bin flag). `None` when no vault is open.
    pub fn database_settings(&self) -> Result<Option<DatabaseSettings>, String> {
        if !self.is_open() {
            return Ok(None);
        }
        let db = self.require_db()?;
        let kdf = match &db.config.kdf_config {
            KdfConfig::Aes { .. } => "Aes",
            KdfConfig::Argon2 { .. } => "Argon2",
            KdfConfig::Argon2id { .. } => "Argon2id",
            _ => "Unknown",
        };
        let cipher = match &db.config.outer_cipher_config {
            OuterCipherConfig::AES256 => "Aes256",
            OuterCipherConfig::Twofish => "Twofish",
            OuterCipherConfig::ChaCha20 => "ChaCha20",
            _ => "Unknown",
        };
        let compression = match &db.config.compression_config {
            CompressionConfig::None => "None",
            CompressionConfig::GZip => "Gzip",
            _ => "Unknown",
        };
        Ok(Some(DatabaseSettings {
            kdf: kdf.to_owned(),
            cipher: cipher.to_owned(),
            compression: compression.to_owned(),
            history_max_items: db.meta.history_max_items.map(|value| value as i64),
            history_max_size: db.meta.history_max_size.map(|value| value as i64),
            recycle_bin_enabled: db.meta.recyclebin_enabled.unwrap_or(true),
            entry_templates_group: db.meta.entry_templates_group.map(|uuid| uuid.to_string()),
        }))
    }

    /// Apply a partial update to database-level settings (history cap and
    /// recycle-bin flag). Absent fields are kept; `null` resets to default.
    pub fn update_database_settings(
        &mut self,
        patch: &DatabaseSettingsPatch,
    ) -> Result<VaultState, String> {
        // Meta flags first so the re-encrypt clone below carries them.
        {
            let db = self.require_db_mut()?;
            if let Some(history_max_items) = patch.history_max_items {
                db.meta.history_max_items = history_max_items.map(|value| value as isize);
            }
            if let Some(recycle_bin_enabled) = patch.recycle_bin_enabled {
                db.meta.recyclebin_enabled = recycle_bin_enabled;
            }
            if let Some(history_max_size) = patch.history_max_size {
                db.meta.history_max_size = history_max_size.map(|value| value as isize);
            }
            if let Some(entry_templates_group) = &patch.entry_templates_group {
                db.meta.entry_templates_group = match entry_templates_group {
                    Some(uuid) => {
                        Some(Uuid::parse_str(uuid).map_err(|_| "模板分组 UUID 无效".to_owned())?)
                    }
                    None => None,
                };
            }
        }
        let storage_changed =
            patch.kdf.is_some() || patch.cipher.is_some() || patch.compression.is_some();
        if !storage_changed {
            self.mark_dirty();
            return self.snapshot_without_icons();
        }

        // Clone the database, apply the new storage config, and re-encrypt it
        // with the same master key before touching the live session (a failed
        // write leaves the open session unchanged).
        let (db, target, revision) = self.prepare_change()?;
        let mut db = db;
        if let Some(kdf) = &patch.kdf {
            apply_kdf(&mut db, kdf)?;
        }
        if let Some(cipher) = &patch.cipher {
            apply_cipher(&mut db, cipher)?;
        }
        if let Some(compression) = &patch.compression {
            apply_compression(&mut db, compression)?;
        }
        let password = self.require_password()?.to_owned();
        let keyfile = self.keyfile.clone();
        persist_change(&db, &password, keyfile.as_deref(), &target)?;

        {
            let live = self.require_db_mut()?;
            if let Some(kdf) = &patch.kdf {
                apply_kdf(live, kdf)?;
            }
            if let Some(cipher) = &patch.cipher {
                apply_cipher(live, cipher)?;
            }
            if let Some(compression) = &patch.compression {
                apply_compression(live, compression)?;
            }
        }
        self.complete_save(revision)
    }

    pub fn save(&mut self) -> Result<VaultState, String> {
        let job = self.prepare_save()?;
        let revision = job.revision;
        persist_save(job)?;
        self.complete_save(revision)
    }

    /// Re-encrypt and persist the vault with a new master key (password
    /// and/or keyfile). The session continues with the new key.
    pub fn change_master_key(
        &mut self,
        password: &str,
        keyfile: Option<&Path>,
    ) -> Result<VaultState, String> {
        let keyfile_bytes = read_keyfile(keyfile)?;
        let (db, target, revision) = self.prepare_change()?;
        persist_change(&db, password, keyfile_bytes.as_deref(), &target)?;
        self.complete_change(password.to_owned(), keyfile_bytes, revision)
    }

    /// Save As: persist the current database (same master key) to a new local
    /// path, then switch the session to that path. On success a remote
    /// session becomes a plain local one — later saves go to the new file,
    /// never back to S3. On failure the session is left untouched.
    pub fn save_as(&mut self, path: &Path) -> Result<VaultState, String> {
        let job = self.prepare_save_as(path)?;
        let revision = job.revision;
        persist_save(job)?;
        self.complete_save_as(path.to_path_buf(), revision)
    }
}

impl VaultSession {
    // -- internals ----------------------------------------------------------

    /// Locked half of `open`/`create`: adopt a decrypted database prepared
    /// outside the lock. Cheap — no KDF, no file I/O.
    pub(crate) fn adopt_local(
        &mut self,
        db: Database,
        path: &Path,
        password: &str,
        keyfile: Option<Vec<u8>>,
    ) -> Result<VaultState, String> {
        self.replace(db, path, password, keyfile);
        self.snapshot()
    }

    /// Locked half of `open_remote`/`create_remote`: adopt a decrypted
    /// database prepared outside the lock. Cheap — no KDF, no network I/O.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn adopt_remote(
        &mut self,
        db: Database,
        storage: Arc<dyn RemoteStorage>,
        key: &str,
        password: &str,
        keyfile: Option<Vec<u8>>,
        mode: RemoteMode,
        local_dir: &Path,
        backup_count: usize,
        backup_template: &str,
    ) -> Result<VaultState, String> {
        self.remote = Some(RemoteTarget {
            storage,
            key: key.to_owned(),
            mode,
            local_dir: local_dir.to_path_buf(),
            backup_count,
            backup_template: backup_template.to_owned(),
        });
        self.path = Some(format!("{REMOTE_URI_PREFIX}{key}"));
        self.password = Some(password.to_owned());
        self.keyfile = keyfile;
        self.db = Some(db);
        self.dirty = false;
        self.modified_at = now_iso();
        self.cached_snapshot = None;
        self.snapshot()
    }

    /// Replace the session with a freshly opened/created local database. Any
    /// remote target from a previous session is dropped: saving a local vault
    /// must never upload to a stale S3 target.
    fn replace(&mut self, db: Database, path: &Path, password: &str, keyfile: Option<Vec<u8>>) {
        self.path = Some(path.to_string_lossy().into_owned());
        self.password = Some(password.to_owned());
        self.keyfile = keyfile;
        self.db = Some(db);
        self.dirty = false;
        self.modified_at = now_iso();
        self.remote = None;
        self.cached_snapshot = None;
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
        self.revision += 1;
        self.modified_at = now_iso();
    }

    pub(crate) fn require_db(&self) -> Result<&Database, String> {
        self.db.as_ref().ok_or_else(|| "数据库未打开".to_owned())
    }

    pub(crate) fn require_db_mut(&mut self) -> Result<&mut Database, String> {
        self.db.as_mut().ok_or_else(|| "数据库未打开".to_owned())
    }

    fn require_path(&self) -> Result<&str, String> {
        self.path
            .as_deref()
            .ok_or_else(|| "数据库未打开".to_owned())
    }

    fn require_password(&self) -> Result<&str, String> {
        self.password
            .as_deref()
            .ok_or_else(|| "数据库未打开".to_owned())
    }

    pub(crate) fn snapshot(&mut self) -> Result<VaultState, String> {
        self.build_snapshot(true)
    }

    /// Mutation result: rebuild the tree but omit the custom-icon image
    /// payload (`custom_icons: None`), so favorites/expansion/CRUD no longer
    /// re-transmit every favicon over IPC. The renderer caches icons from the
    /// last authoritative snapshot and merges them back locally.
    pub(crate) fn snapshot_without_icons(&mut self) -> Result<VaultState, String> {
        self.build_snapshot(false)
    }

    fn build_snapshot(&mut self, include_icons: bool) -> Result<VaultState, String> {
        // Rebuild the tree only when the database changed; repeated full
        // reads (e.g. get_vault_state polling) reuse the cached snapshot.
        // Light mutation snapshots bypass the cache because the revision just
        // changed, and they must never serve as the full-state cache.
        if include_icons {
            if let Some((revision, cached)) = &self.cached_snapshot {
                if *revision == self.revision {
                    return Ok(cached.clone());
                }
            }
        }
        let db = self.require_db()?;
        let path = self.require_path()?;
        let state = VaultState {
            path: path.to_owned(),
            file_name: Path::new(path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(path)
                .to_owned(),
            root: build_group_tree(db),
            dirty: self.dirty,
            modified_at: self.modified_at.clone(),
            revision: self.revision,
            custom_icons: include_icons.then(|| {
                db.iter_all_custom_icons()
                    .map(|icon| (icon.id().uuid().to_string(), icon_to_data_url(&icon.data)))
                    .collect()
            }),
            meta_custom_data: custom_data_entries(&db.meta.custom_data),
            database_name: db.meta.database_name.clone(),
            database_description: db.meta.database_description.clone(),
        };
        if include_icons {
            self.cached_snapshot = Some((self.revision, state.clone()));
        }
        Ok(state)
    }

    /// Capture everything `persist_save` needs. Cheap (no KDF, no I/O): a
    /// database clone plus the target info, so the slow work can run outside
    /// the session lock.
    pub(crate) fn prepare_save(&self) -> Result<SaveJob, String> {
        let (db, target, revision) = self.prepare_change()?;
        Ok(SaveJob {
            db,
            password: self.require_password()?.to_owned(),
            keyfile: self.keyfile.clone(),
            target,
            revision,
        })
    }

    /// Locked half of `save_as`: capture the database clone, master key and
    /// explicit new local path. Cheap — no KDF, no disk I/O, so the heavy
    /// re-encrypt + write can run outside the lock.
    pub(crate) fn prepare_save_as(&self, path: &Path) -> Result<SaveJob, String> {
        let db = self.require_db()?.clone();
        Ok(SaveJob {
            db,
            password: self.require_password()?.to_owned(),
            keyfile: self.keyfile.clone(),
            target: SaveTarget::Local(path.to_path_buf()),
            revision: self.revision,
        })
    }

    /// Locked completion of `save_as`: switch the session target to the new
    /// local path (dropping any remote target) and mark clean unless edits
    /// landed while the save ran. Only called after the persist succeeded.
    pub(crate) fn complete_save_as(
        &mut self,
        path: PathBuf,
        revision: u64,
    ) -> Result<VaultState, String> {
        self.path = Some(path.to_string_lossy().into_owned());
        self.remote = None;
        if self.revision == revision {
            self.dirty = false;
            self.modified_at = now_iso();
            self.cached_snapshot = None;
        }
        self.snapshot()
    }

    /// Cheap snapshot of the database and save target, used when the caller
    /// supplies a fresh master key (`change_master_key`). Returns the current
    /// edit revision so the completion step can detect concurrent edits.
    pub(crate) fn prepare_change(&self) -> Result<(Database, SaveTarget, u64), String> {
        let db = self.require_db()?.clone();
        let target = match &self.remote {
            Some(remote) => SaveTarget::Remote {
                storage: remote.storage.clone(),
                key: remote.key.clone(),
                mode: remote.mode,
                local_dir: remote.local_dir.clone(),
                backup_count: remote.backup_count,
                backup_template: remote.backup_template.clone(),
            },
            None => SaveTarget::Local(PathBuf::from(self.require_path()?.to_owned())),
        };
        Ok((db, target, self.revision))
    }

    /// Locked completion of `save`: mark the session clean (unless edits
    /// landed while the save ran) and re-snapshot.
    pub(crate) fn complete_save(&mut self, revision: u64) -> Result<VaultState, String> {
        if self.revision == revision {
            self.dirty = false;
            self.modified_at = now_iso();
            self.cached_snapshot = None;
        }
        self.snapshot()
    }

    /// Locked completion of `change_master_key`: adopt the new credentials
    /// and re-snapshot. Only called after the persist succeeded.
    pub(crate) fn complete_change(
        &mut self,
        password: String,
        keyfile: Option<Vec<u8>>,
        revision: u64,
    ) -> Result<VaultState, String> {
        self.password = Some(password);
        self.keyfile = keyfile;
        if self.revision == revision {
            self.dirty = false;
            self.modified_at = now_iso();
            self.cached_snapshot = None;
        }
        self.snapshot()
    }
}
