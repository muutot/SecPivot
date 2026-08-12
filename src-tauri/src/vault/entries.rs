//! Entry and group CRUD on the open `VaultSession`, including history and the
//! recycle bin (extracted from mod.rs).

use super::helpers::{
    ensure_recycle_bin, entry_has_otp, group_contains, parse_entry_id, parse_group_id,
    recycle_bin_id, resolve_group_id,
};
use super::serialize::{
    apply_patch_fields, attachment_size, custom_data_entries, decode_attachments,
    delete_history_entry, format_iso, history_cap, sync_attachments, sync_custom_fields,
    trim_entry_history, write_fields, AttachmentPayload,
};
use super::*;
use keepass::db::{
    AutoType, AutoTypeAssociation, DataTransferObfuscation, EntryId, GroupId, Icon, Times, Value,
};
use std::collections::HashMap;

fn normalize_optional_sequence(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

impl VaultSession {
    pub fn add_entry(&mut self, input: &EntryInput) -> Result<VaultState, String> {
        // Decode all attachment payloads before touching the database so a
        // bad payload aborts the whole mutation (no half-applied entry).
        let payloads = decode_attachments(&input.attachments)?;
        {
            let db = self.require_db_mut()?;
            let mut group = if input.group_uuid == ROOT_GROUP_UUID {
                db.root_mut()
            } else {
                let group_id = parse_group_id(&input.group_uuid)?;
                db.group_mut(group_id)
                    .ok_or_else(|| "目标分组不存在".to_owned())?
            };
            let mut entry = group.add_entry();
            write_fields(&mut entry, input);
            sync_custom_fields(&mut entry, &input.custom_fields);
            sync_attachments(&mut entry, &input.attachments, &payloads);
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Add many entries in one transaction. Every attachment payload is
    /// decoded up-front so a bad payload aborts the whole batch (no partial
    /// import), then all entries are inserted under a single database borrow,
    /// dirty flag and snapshot — the CSV/XML importer calls this once instead
    /// of round-tripping one IPC per row.
    pub fn add_entries(&mut self, inputs: &[EntryInput]) -> Result<VaultState, String> {
        if inputs.is_empty() {
            return self.snapshot_without_icons();
        }
        let payloads: Vec<Vec<AttachmentPayload>> = inputs
            .iter()
            .map(|input| decode_attachments(&input.attachments))
            .collect::<Result<_, _>>()?;
        {
            let db = self.require_db_mut()?;
            for (input, payload) in inputs.iter().zip(&payloads) {
                let mut group = if input.group_uuid == ROOT_GROUP_UUID {
                    db.root_mut()
                } else {
                    let group_id = parse_group_id(&input.group_uuid)?;
                    db.group_mut(group_id)
                        .ok_or_else(|| "目标分组不存在".to_owned())?
                };
                let mut entry = group.add_entry();
                write_fields(&mut entry, input);
                sync_custom_fields(&mut entry, &input.custom_fields);
                sync_attachments(&mut entry, &input.attachments, payload);
            }
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    pub fn update_entry(&mut self, uuid: &str, input: &EntryInput) -> Result<VaultState, String> {
        let id = parse_entry_id(uuid)?;
        let target_group = resolve_group_id(self.require_db()?, &input.group_uuid)?;
        // Decode attachment payloads up-front; a decode failure must not
        // leave a half-applied update (fields written, history snapshotted).
        let payloads = decode_attachments(&input.attachments)?;
        let cap = {
            let db = self.require_db()?;
            history_cap(&db.meta)
        };
        {
            let db = self.require_db_mut()?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            if entry.parent_mut().id() != target_group {
                entry
                    .move_to(target_group)
                    .map_err(|e| format!("移动条目失败: {e}"))?;
            }
            {
                // Snapshots the pre-change state into the entry's history on drop.
                let mut tracked = entry.track_changes();
                {
                    let mut current = tracked.as_mut();
                    write_fields(&mut current, input);
                    sync_custom_fields(&mut current, &input.custom_fields);
                    sync_attachments(&mut current, &input.attachments, &payloads);
                }
                tracked.times.last_modification = Some(Times::now());
            }
            trim_entry_history(&mut entry, cap);
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Update the entry's matching/quality flags without touching its stored
    /// fields (same pattern as `update_entry_autotype`). `override_url` is
    /// tri-state: absent keeps the current value, an empty string clears it,
    /// a non-empty string sets it (trimmed). `quality_check` sets the KDBX
    /// per-entry password-quality flag when present.
    pub fn update_entry_flags(
        &mut self,
        uuid: &str,
        override_url: Option<String>,
        quality_check: Option<bool>,
        foreground_color: Option<String>,
    ) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let id = parse_entry_id(uuid)?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            if let Some(value) = override_url {
                let value = value.trim();
                entry.override_url = if value.is_empty() {
                    None
                } else {
                    Some(value.to_owned())
                };
            }
            if let Some(check) = quality_check {
                entry.quality_check = check;
            }
            if let Some(color) = foreground_color {
                entry.foreground_color = super::serialize::parse_color(Some(color.as_str()));
            }
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Apply one partial patch to several entries in a single transaction.
    /// Only the fields present in the patch are written; the rest of each
    /// entry (including the password) is left untouched. All uuids are
    /// validated up-front so a bad id aborts the whole batch before any
    /// entry is modified; the snapshot is built once afterwards.
    pub fn update_entries(
        &mut self,
        uuids: &[String],
        patch: &EntryPatch,
    ) -> Result<VaultState, String> {
        if uuids.is_empty() {
            return self.snapshot_without_icons();
        }
        let ids: Vec<EntryId> = uuids
            .iter()
            .map(|uuid| parse_entry_id(uuid))
            .collect::<Result<_, _>>()?;
        let cap = {
            let db = self.require_db()?;
            history_cap(&db.meta)
        };
        {
            let db = self.require_db_mut()?;
            for id in &ids {
                db.entry(*id).ok_or_else(|| "条目不存在".to_owned())?;
            }
            for id in ids {
                let mut entry = db.entry_mut(id).expect("validated entry must exist");
                {
                    let mut tracked = entry.track_changes();
                    {
                        let mut current = tracked.as_mut();
                        apply_patch_fields(&mut current, patch);
                    }
                    tracked.times.last_modification = Some(Times::now());
                }
                trim_entry_history(&mut entry, cap);
            }
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Move an entry into another group (used by drag-and-drop).
    pub fn move_entry(&mut self, uuid: &str, group_uuid: &str) -> Result<VaultState, String> {
        let id = parse_entry_id(uuid)?;
        let target = resolve_group_id(self.require_db()?, group_uuid)?;
        {
            let db = self.require_db_mut()?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            if entry.parent_mut().id() != target {
                entry
                    .move_to(target)
                    .map_err(|e| format!("移动条目失败: {e}"))?;
            }
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Move several entries into the recycle bin (or permanently delete them
    /// when they are already inside the recycle bin).
    pub fn delete_entries(&mut self, uuids: &[String]) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let bin_id = ensure_recycle_bin(db)?;
            for uuid in uuids {
                let id = parse_entry_id(uuid)?;
                let in_bin = {
                    let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
                    entry.parent().id() == bin_id
                };
                if in_bin {
                    let entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
                    entry.remove();
                } else {
                    let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
                    if entry.get(FIELD_ORIGINAL_GROUP).is_none() {
                        let original = entry.parent_mut().id().uuid().to_string();
                        entry.set(FIELD_ORIGINAL_GROUP, Value::unprotected(original));
                    }
                    entry
                        .move_to(bin_id)
                        .map_err(|e| format!("移入回收站失败: {e}"))?;
                }
            }
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Move an entry to the recycle bin (or permanently delete it when it is
    /// already inside the recycle bin).
    pub fn delete_entry(&mut self, uuid: &str) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let id = parse_entry_id(uuid)?;
            let bin_id = ensure_recycle_bin(db)?;
            let in_bin = {
                let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
                entry.parent().id() == bin_id
            };
            if in_bin {
                let entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
                entry.remove();
            } else {
                let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
                if entry.get(FIELD_ORIGINAL_GROUP).is_none() {
                    let original = entry.parent_mut().id().uuid().to_string();
                    entry.set(FIELD_ORIGINAL_GROUP, Value::unprotected(original));
                }
                entry
                    .move_to(bin_id)
                    .map_err(|e| format!("移入回收站失败: {e}"))?;
            }
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// List the historical snapshots of an entry, newest first. Passwords are
    /// intentionally excluded from the payload — the renderer restores by
    /// index, and the plaintext must not leave the backend.
    pub fn get_entry_history(&self, uuid: &str) -> Result<Vec<HistoryVersion>, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        let Some(history) = entry.history.as_ref() else {
            return Ok(Vec::new());
        };
        let count = history.get_entries().len();
        Ok((0..count)
            .filter_map(|index| entry.historical(index).map(|h| (index, h)))
            .map(|(index, historical)| HistoryVersion {
                index,
                modified: historical.times.last_modification.map(format_iso),
                title: historical.get_title().unwrap_or_default().to_owned(),
                username: historical
                    .get(FIELD_USERNAME)
                    .unwrap_or_default()
                    .to_owned(),
                url: historical.get(FIELD_URL).unwrap_or_default().to_owned(),
                notes: historical.get(FIELD_NOTES).unwrap_or_default().to_owned(),
                expires: match historical.times.expires {
                    Some(true) => historical.times.expiry.map(format_iso),
                    _ => None,
                },
                has_totp: entry_has_otp(&historical),
                icon: match historical.icon() {
                    Some(Icon::BuiltIn(id)) => Some(*id as u32),
                    _ => None,
                },
                custom_icon: match historical.icon() {
                    Some(Icon::Custom(id)) => Some(id.uuid().to_string()),
                    _ => None,
                },
                tags: if historical.tags.is_empty() {
                    None
                } else {
                    Some(historical.tags.join(", "))
                },
                color: historical
                    .background_color
                    .as_ref()
                    .map(ToString::to_string),
                favorite: historical.get(FIELD_FAVORITE) == Some(FIELD_FAVORITE_TRUE),
                quality_check: historical.quality_check,
                custom_data: custom_data_entries(&historical.custom_data),
                custom_fields: {
                    let mut fields: Vec<CustomField> = historical
                        .fields
                        .iter()
                        .filter(|(name, _)| {
                            !name.is_empty() && !RESERVED_FIELDS.contains(&name.as_str())
                        })
                        .map(|(name, value)| CustomField {
                            name: name.clone(),
                            value: value.get().clone(),
                            protected: value.is_protected(),
                        })
                        .collect();
                    fields.sort_by(|a, b| a.name.cmp(&b.name));
                    fields
                },
                attachments: historical
                    .attachments_named()
                    .filter_map(|(name, attachment)| {
                        attachment_size(&attachment).map(|size| AttachmentInfo {
                            name: name.to_owned(),
                            size,
                        })
                    })
                    .collect(),
            })
            .collect())
    }

    /// Byte-size breakdown of everything the entry stores: its own field text,
    /// attachment data, and all historical snapshots (fields + attachments).
    /// Uses in-memory sizes, so protected values count like any other field.
    pub fn get_entry_storage(&self, uuid: &str) -> Result<EntryStorage, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;

        let fields = entry
            .fields
            .values()
            .map(|value| value.get().len())
            .sum::<usize>();
        let attachments = entry
            .attachments_named()
            .map(|(_, attachment)| attachment.data.get().len())
            .sum::<usize>();
        let history = match &entry.history {
            None => 0,
            Some(h) => {
                let count = h.get_entries().len();
                (0..count)
                    .filter_map(|index| entry.historical(index))
                    .map(|historical| {
                        let f = historical
                            .fields
                            .values()
                            .map(|value| value.get().len())
                            .sum::<usize>();
                        let a = historical
                            .attachments_named()
                            .filter_map(|(_, attachment)| attachment_size(&attachment))
                            .sum::<usize>();
                        f + a
                    })
                    .sum()
            }
        };
        Ok(EntryStorage {
            fields,
            attachments,
            history,
            total: fields + attachments + history,
        })
    }

    /// Overwrite an entry with a historical snapshot. The current state is
    /// itself pushed into the history first, so the restore can be undone.
    pub fn restore_entry_version(
        &mut self,
        uuid: &str,
        index: usize,
    ) -> Result<VaultState, String> {
        let id = parse_entry_id(uuid)?;
        let version = {
            let db = self.require_db()?;
            let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
            let history = entry
                .history
                .as_ref()
                .ok_or_else(|| "该条目没有历史版本".to_owned())?;
            history
                .get_entries()
                .get(index)
                .ok_or_else(|| "历史版本不存在".to_owned())?
                .clone()
        };
        {
            let cap = {
                let db = self.require_db()?;
                history_cap(&db.meta)
            };
            let db = self.require_db_mut()?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            {
                let mut tracked = entry.track_changes();
                {
                    let mut current = tracked.as_mut();
                    current.fields.clear();
                    for (name, value) in &version.fields {
                        current.fields.insert(name.clone(), value.clone());
                    }
                    current.tags = version.tags.clone();
                    current.times.expiry = version.times.expiry;
                    current.times.expires = version.times.expires;
                    match version.icon() {
                        Some(Icon::BuiltIn(icon_id)) => current.set_icon_builtin(*icon_id),
                        _ => current.set_icon_none(),
                    }
                }
                tracked.times.last_modification = Some(Times::now());
            }
            trim_entry_history(&mut entry, cap);
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Permanently delete one historical snapshot from an entry's history.
    /// The current state is untouched and no new snapshot is created.
    pub fn delete_entry_history(&mut self, uuid: &str, index: usize) -> Result<VaultState, String> {
        let id = parse_entry_id(uuid)?;
        {
            let db = self.require_db_mut()?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            if !delete_history_entry(&mut entry, index) {
                return Err("历史版本不存在".to_owned());
            }
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Restore a recycled entry to its original group (or root when the
    /// original group no longer exists).
    pub fn restore_entry(&mut self, uuid: &str) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let id = parse_entry_id(uuid)?;
            let bin_id = recycle_bin_id(db).ok_or_else(|| "回收站不存在".to_owned())?;
            let (in_bin, original_group) = {
                let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
                (
                    entry.parent().id() == bin_id,
                    entry
                        .get(FIELD_ORIGINAL_GROUP)
                        .map(|value| value.to_owned())
                        .and_then(|uuid| parse_group_id(&uuid).ok()),
                )
            };
            if !in_bin {
                return Err("只有回收站中的条目可以恢复".to_owned());
            }
            let target = match original_group {
                Some(group_id) if db.group(group_id).is_some() => group_id,
                _ => db.root().id(),
            };
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            entry
                .move_to(target)
                .map_err(|e| format!("恢复条目失败: {e}"))?;
            entry.fields.remove(FIELD_ORIGINAL_GROUP);
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Delete a group: move the whole subtree to the recycle bin, or
    /// permanently delete it when it is already inside the recycle bin.
    pub fn delete_group(&mut self, uuid: &str) -> Result<VaultState, String> {
        if uuid == ROOT_GROUP_UUID {
            return Err("不能删除根分组".to_owned());
        }
        {
            let db = self.require_db_mut()?;
            let id = parse_group_id(uuid)?;
            let bin_id = ensure_recycle_bin(db)?;
            if id == bin_id {
                return Err("请先清空或移动回收站内容,再删除回收站".to_owned());
            }
            if group_contains(db, bin_id, id) {
                let group = db.group_mut(id).ok_or_else(|| "分组不存在".to_owned())?;
                group.remove();
            } else {
                let mut group = db.group_mut(id).ok_or_else(|| "分组不存在".to_owned())?;
                group
                    .move_to(bin_id)
                    .map_err(|e| format!("移入回收站失败: {e}"))?;
            }
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Restore a recycled group back to the root.
    pub fn restore_group(&mut self, uuid: &str) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let id = parse_group_id(uuid)?;
            let bin_id = recycle_bin_id(db).ok_or_else(|| "回收站不存在".to_owned())?;
            if !group_contains(db, bin_id, id) {
                return Err("只有回收站中的分组可以恢复".to_owned());
            }
            if id == bin_id {
                return Err("回收站本身不能恢复".to_owned());
            }
            let root_id = db.root().id();
            let mut group = db.group_mut(id).ok_or_else(|| "分组不存在".to_owned())?;
            group
                .move_to(root_id)
                .map_err(|e| format!("恢复分组失败: {e}"))?;
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Permanently delete every entry and group inside the recycle bin,
    /// keeping the empty recycle bin group itself.
    pub fn empty_recycle_bin(&mut self) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let bin_id = recycle_bin_id(db).ok_or_else(|| "回收站不存在".to_owned())?;
            let (entries, children) = {
                let bin = db.group(bin_id).ok_or_else(|| "回收站不存在".to_owned())?;
                (
                    bin.entries().map(|e| e.id()).collect::<Vec<EntryId>>(),
                    bin.groups().map(|g| g.id()).collect::<Vec<GroupId>>(),
                )
            };
            for entry_id in entries {
                if let Some(entry) = db.entry_mut(entry_id) {
                    entry.remove();
                }
            }
            for child_id in children {
                if let Some(group) = db.group_mut(child_id) {
                    group.remove();
                }
            }
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }
}

impl VaultSession {
    pub fn add_group(&mut self, input: &GroupInput) -> Result<VaultState, String> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err("分组名称不能为空".to_owned());
        }
        {
            let db = self.require_db_mut()?;
            let mut parent = match input.parent_uuid.as_deref() {
                None | Some(ROOT_GROUP_UUID) => db.root_mut(),
                Some(parent) => {
                    let parent_id = parse_group_id(parent)?;
                    db.group_mut(parent_id)
                        .ok_or_else(|| "父分组不存在".to_owned())?
                }
            };
            let mut group = parent.add_group();
            group.name = name.to_owned();
            match input.icon {
                Some(icon_id) => group.set_icon_builtin(icon_id as usize),
                None => group.set_icon_none(),
            }
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    pub fn rename_group(&mut self, uuid: &str, name: &str) -> Result<VaultState, String> {
        if uuid == ROOT_GROUP_UUID {
            return Err("不能重命名根分组".to_owned());
        }
        let name = name.trim();
        if name.is_empty() {
            return Err("分组名称不能为空".to_owned());
        }
        {
            let db = self.require_db_mut()?;
            let id = parse_group_id(uuid)?;
            let mut group = db.group_mut(id).ok_or_else(|| "分组不存在".to_owned())?;
            group.name = name.to_owned();
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Set a group's built-in KeePass icon index (`Some(i)`) or reset it to
    /// the default icon (`None`). The icon does not touch the group name.
    pub fn set_group_icon(&mut self, uuid: &str, icon: Option<u32>) -> Result<VaultState, String> {
        let id = parse_group_id(uuid)?;
        {
            let db = self.require_db_mut()?;
            let mut group = db.group_mut(id).ok_or_else(|| "分组不存在".to_owned())?;
            match icon {
                Some(index) => group.set_icon_builtin(index as usize),
                None => group.set_icon_none(),
            }
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Update group metadata (notes, tags, search participation) without
    /// touching the name or icon. `notes`: a present value sets it (empty
    /// string clears), absent keeps. `tags`: a comma-separated string sets
    /// them (empty clears), absent keeps. `enable_searching`: a present bool
    /// sets the KDBX group flag, absent keeps.
    pub fn update_group_meta(
        &mut self,
        uuid: &str,
        notes: Option<String>,
        tags: Option<String>,
        enable_searching: Option<bool>,
    ) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let mut group = if uuid == ROOT_GROUP_UUID {
                db.root_mut()
            } else {
                let id = parse_group_id(uuid)?;
                db.group_mut(id).ok_or_else(|| "分组不存在".to_owned())?
            };
            if let Some(notes) = notes {
                let notes = notes.trim();
                group.notes = if notes.is_empty() {
                    None
                } else {
                    Some(notes.to_owned())
                };
            }
            if let Some(tags) = tags {
                group.tags = super::serialize::parse_tags(&tags);
            }
            if let Some(flag) = enable_searching {
                group.enable_searching = Some(flag);
            }
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Replace an attachment's bytes with the content of a registered temp
    /// file (the external viewer's saved changes). Only files created by
    /// `open_attachment_temp` can be imported — never an arbitrary path.
    pub fn import_attachment_from_temp(
        &mut self,
        uuid: &str,
        name: &str,
        token: &str,
        store: &super::AttachmentTempStore,
    ) -> Result<VaultState, String> {
        const MAX_IMPORT_BYTES: u64 = 64 * 1024 * 1024;
        let path = store.path(token)?;
        let meta = std::fs::metadata(&path).map_err(|e| format!("读取临时附件失败: {e}"))?;
        if meta.len() > MAX_IMPORT_BYTES {
            return Err(format!("附件过大（{} 字节，上限 64 MiB）", meta.len()));
        }
        let data = std::fs::read(&path).map_err(|e| format!("读取临时附件失败: {e}"))?;
        {
            let db = self.require_db_mut()?;
            let id = parse_entry_id(uuid)?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            entry.remove_attachment_by_name(name);
            entry.add_attachment(name.to_owned(), Value::protected(data));
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Persist a group's expanded state to the KDBX `Group.is_expanded` flag so
    /// the tree reopens the same groups after a save + reopen.
    pub fn set_group_expanded(&mut self, uuid: &str, expanded: bool) -> Result<VaultState, String> {
        self.set_groups_expanded_delta(&[uuid.to_owned()], expanded)?;
        self.snapshot_without_icons()
    }

    /// Persist several group expansion flags in one transaction. All ids are
    /// validated before the first write so an unknown uuid cannot leave a
    /// partially expanded/collapsed tree; the snapshot is built only once.
    pub fn set_groups_expanded(
        &mut self,
        uuids: &[String],
        expanded: bool,
    ) -> Result<VaultState, String> {
        self.set_groups_expanded_delta(uuids, expanded)?;
        self.snapshot_without_icons()
    }

    /// Same batch mutation as `set_groups_expanded`, but returns only the
    /// delta (new revision + uuid→expanded map) instead of rebuilding and
    /// serializing the whole tree; the renderer applies it locally.
    pub fn set_groups_expanded_delta(
        &mut self,
        uuids: &[String],
        expanded: bool,
    ) -> Result<MutationDelta, String> {
        if uuids.is_empty() {
            return Ok(MutationDelta::GroupsExpanded {
                revision: self.revision,
                groups: HashMap::new(),
            });
        }
        let ids: Vec<GroupId> = uuids
            .iter()
            .map(|uuid| parse_group_id(uuid))
            .collect::<Result<_, _>>()?;
        {
            let db = self.require_db_mut()?;
            for id in &ids {
                db.group(*id).ok_or_else(|| "分组不存在".to_owned())?;
            }
            for id in ids {
                let mut group = db.group_mut(id).expect("validated group must exist");
                group.is_expanded = expanded;
            }
        }
        self.mark_dirty();
        let groups = uuids.iter().cloned().map(|uuid| (uuid, expanded)).collect();
        Ok(MutationDelta::GroupsExpanded {
            revision: self.revision,
            groups,
        })
    }

    /// Update the database-level `Meta` display fields. `name`/`description`
    /// are tri-state strings: an empty string clears the field, an absent
    /// value leaves it untouched.
    pub fn update_db_meta(
        &mut self,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            if let Some(name) = &name {
                db.meta.database_name = (!name.is_empty()).then(|| name.clone());
                db.meta.database_name_changed = Some(chrono::Utc::now().naive_utc());
            }
            if let Some(description) = &description {
                db.meta.database_description =
                    (!description.is_empty()).then(|| description.clone());
                db.meta.database_description_changed = Some(chrono::Utc::now().naive_utc());
            }
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Replace an entry's Auto-Type configuration (enabled flag, default
    /// sequence, and window associations) in one write.
    pub fn update_entry_autotype(
        &mut self,
        uuid: &str,
        input: &EntryAutoTypeInput,
    ) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let id = parse_entry_id(uuid)?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            entry.autotype = Some(AutoType {
                enabled: input.enabled,
                default_sequence: normalize_optional_sequence(input.default_sequence.as_deref()),
                data_transfer_obfuscation: DataTransferObfuscation::None,
                associations: input
                    .associations
                    .iter()
                    .map(|a| AutoTypeAssociation {
                        window: a.window.trim().to_owned(),
                        sequence: a.sequence.clone(),
                    })
                    .collect(),
            });
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }

    /// Update a group's Auto-Type settings (enabled flag and default
    /// sequence). Absent fields keep the current value; an empty sequence
    /// clears the group default.
    pub fn update_group_autotype(
        &mut self,
        uuid: &str,
        input: &GroupAutoTypeInput,
    ) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            if uuid == ROOT_GROUP_UUID {
                let mut root = db.root_mut();
                root.enable_autotype = input.enabled;
                if let Some(sequence) = &input.default_sequence {
                    root.default_autotype_sequence = normalize_optional_sequence(Some(sequence));
                }
            } else {
                let id = parse_group_id(uuid)?;
                let mut group = db.group_mut(id).ok_or_else(|| "分组不存在".to_owned())?;
                group.enable_autotype = input.enabled;
                if let Some(sequence) = &input.default_sequence {
                    group.default_autotype_sequence = normalize_optional_sequence(Some(sequence));
                }
            }
        }
        self.mark_dirty();
        self.snapshot_without_icons()
    }
}
