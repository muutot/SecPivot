//! Entry and group CRUD on the open `VaultSession`, including history and the
//! recycle bin (extracted from mod.rs).

use super::helpers::{
    ensure_recycle_bin, group_contains, parse_entry_id, parse_group_id, recycle_bin_id,
    resolve_group_id,
};
use super::serialize::{
    apply_patch_fields, attachment_size, decode_attachments, delete_history_entry, format_iso,
    sync_attachments, sync_custom_fields, trim_entry_history, write_fields,
};
use super::*;
use keepass::db::{EntryId, GroupId, Icon, Times, Value};

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
        self.snapshot()
    }

    pub fn update_entry(&mut self, uuid: &str, input: &EntryInput) -> Result<VaultState, String> {
        let id = parse_entry_id(uuid)?;
        let target_group = resolve_group_id(self.require_db()?, &input.group_uuid)?;
        // Decode attachment payloads up-front; a decode failure must not
        // leave a half-applied update (fields written, history snapshotted).
        let payloads = decode_attachments(&input.attachments)?;
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
            trim_entry_history(&mut entry);
        }
        self.mark_dirty();
        self.snapshot()
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
            return self.snapshot();
        }
        let ids: Vec<EntryId> = uuids
            .iter()
            .map(|uuid| parse_entry_id(uuid))
            .collect::<Result<_, _>>()?;
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
                trim_entry_history(&mut entry);
            }
        }
        self.mark_dirty();
        self.snapshot()
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
        self.snapshot()
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
        self.snapshot()
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
        self.snapshot()
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
            trim_entry_history(&mut entry);
        }
        self.mark_dirty();
        self.snapshot()
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
        self.snapshot()
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
        self.snapshot()
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
        self.snapshot()
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
        self.snapshot()
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
        self.snapshot()
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
        self.snapshot()
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
        self.snapshot()
    }
}
