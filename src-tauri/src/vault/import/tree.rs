//! One rollback-safe publication for complete prepared import trees.
use crate::vault::{helpers::parse_group_id, VaultSession, VaultState, ROOT_GROUP_UUID};
use keepass::db::{Database, EntryId, GroupRef, Value};
use uuid::Uuid;

pub(super) struct SourceArchive {
    pub title: &'static str,
    pub key: &'static str,
    pub value: Value<String>,
    /// External JSON fields are literal data, never internal KeePass references.
    pub literal_fields: bool,
}

fn entry_ids(group: GroupRef<'_>, ids: &mut Vec<EntryId>) {
    ids.extend(group.entries().map(|entry| entry.id()));
    for child in group.groups() {
        entry_ids(child, ids);
    }
}

impl VaultSession {
    pub(crate) fn native_import_lifetime(&self, target: &str) -> Result<Uuid, String> {
        self.validate_1pux_target(target)?;
        if self.history_lifetime.is_nil() {
            return Err("数据库会话已结束；未导入任何内容".into());
        }
        Ok(self.history_lifetime)
    }

    pub(super) fn import_prepared_tree(
        &mut self,
        mut source: Database,
        archive: SourceArchive,
        target: &str,
        lifetime: Uuid,
    ) -> Result<VaultState, String> {
        if self.native_import_lifetime(target)? != lifetime {
            return Err("数据库会话已更换；未导入任何内容".into());
        }
        let db = self.require_db()?;
        let target_id = if target == ROOT_GROUP_UUID {
            db.root().id()
        } else {
            parse_group_id(target)?
        };
        let mut candidate = db.clone();
        let mut literal_fields = Vec::new();
        if archive.literal_fields {
            let mut ids = Vec::new();
            entry_ids(source.root(), &mut ids);
            for id in ids {
                literal_fields.push(std::mem::take(
                    &mut source.entry_mut(id).ok_or("导入条目无效")?.fields,
                ));
            }
        }
        let root = candidate
            .import_group_tree(source, target_id)
            .map_err(|_| "导入分组树无效；未导入任何内容")?;
        if archive.literal_fields {
            let mut ids = Vec::new();
            entry_ids(candidate.group(root).ok_or("导入分组树无效")?, &mut ids);
            if ids.len() != literal_fields.len() {
                return Err("导入条目无效；未导入任何内容".into());
            }
            for (id, fields) in ids.into_iter().zip(literal_fields) {
                candidate.entry_mut(id).ok_or("导入条目无效")?.fields = fields;
            }
        }
        {
            let mut group = candidate
                .group_mut(root)
                .ok_or("目标分组不存在；未导入任何内容")?;
            let mut entry = group.add_entry();
            entry
                .fields
                .insert("Title".into(), Value::unprotected(archive.title));
            entry.fields.insert(archive.key.into(), archive.value);
            entry.quality_check = false;
        }
        let previous = self.db.replace(candidate);
        let previous_meta = (
            self.dirty,
            self.revision,
            self.modified_at.clone(),
            self.cached_snapshot.take(),
            self.pending_file_reload.take(),
        );
        self.mark_dirty();
        match self.snapshot() {
            Ok(state) => Ok(state),
            Err(error) => {
                self.db = previous;
                (
                    self.dirty,
                    self.revision,
                    self.modified_at,
                    self.cached_snapshot,
                    self.pending_file_reload,
                ) = previous_meta;
                Err(error)
            }
        }
    }
}
