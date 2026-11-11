//! One logical editor submission: validate, mutate a candidate, publish once.
use super::helpers::{parse_entry_id, resolve_group_id};
use super::serialize::{
    decode_attachments, parse_color, sync_attachments, sync_custom_fields, trim_entry_history,
    trim_entry_history_by_size, write_fields, AttachmentPayload,
};
use super::*;
use keepass::db::{AutoType, Color, EntryMut, Value};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub enum EntrySaveTarget {
    Create {},
    Edit { uuid: String },
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntryFlagsInput {
    pub override_url: Option<String>,
    pub quality_check: Option<bool>,
    pub foreground_color: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntrySaveRequest {
    pub target: EntrySaveTarget,
    pub input: EntryInput,
    pub auto_type: Option<EntryAutoTypeInput>,
    pub flags: Option<EntryFlagsInput>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntrySaveResult {
    pub uuid: String,
    pub state: VaultState,
}

fn write_editor_fields(
    entry: &mut EntryMut<'_>,
    request: &EntrySaveRequest,
    payloads: &[AttachmentPayload],
    autotype: &Option<AutoType>,
    foreground: Option<Option<Color>>,
) {
    write_fields(entry, &request.input);
    sync_custom_fields(entry, &request.input.custom_fields);
    sync_attachments(entry, &request.input.attachments, payloads);
    if let Some(autotype) = autotype {
        entry.autotype = Some(autotype.clone());
    }
    if let Some(flags) = &request.flags {
        if let Some(value) = &flags.override_url {
            entry.override_url = (!value.trim().is_empty()).then(|| value.trim().to_owned());
        }
        if let Some(check) = flags.quality_check {
            entry.quality_check = check;
        }
        if let Some(color) = foreground {
            entry.foreground_color = color;
        }
    }
}

impl VaultSession {
    pub fn save_entry(&mut self, request: &EntrySaveRequest) -> Result<EntrySaveResult, String> {
        // Like existing editor CRUD, edits remain available in memory after save
        // degradation. Persistence continues to enforce its own writable gate.
        let db = self.require_db()?;
        let group_id = resolve_group_id(db, &request.input.group_uuid)?;
        let (id, existing_rules) = match &request.target {
            EntrySaveTarget::Create {} => (None, false),
            EntrySaveTarget::Edit { uuid } => {
                let id = parse_entry_id(uuid)?;
                let entry = db.entry(id).ok_or("条目不存在")?;
                (Some(id), entry.fields.contains_key(FIELD_WEBSITE_RULES))
            }
        };
        let rules = if id.is_none() {
            super::website_rules::input_rules(&request.input.custom_fields)?
        } else {
            None // Generic content editing preserves the existing managed policy.
        };
        let has_rules = rules.is_some() || existing_rules;
        if has_rules
            && request
                .flags
                .as_ref()
                .and_then(|f| f.override_url.as_deref())
                .is_some_and(|url| !url.trim().is_empty())
        {
            return Err("逐网址规则与覆盖网址不能同时启用".into());
        }
        let foreground = match request
            .flags
            .as_ref()
            .and_then(|f| f.foreground_color.as_deref())
            .map(str::trim)
        {
            None => None,
            Some("") => Some(None),
            Some(color) => Some(Some(
                parse_color(Some(color)).ok_or("前景色无效，应为 #RRGGBB")?,
            )),
        };
        let autotype = request
            .auto_type
            .as_ref()
            .map(super::entries::build_entry_autotype);
        let payloads = decode_attachments(&request.input.attachments)?;
        let (cap, max_size) = super::entries::history_limits(&db.meta);
        let mut candidate = db.clone();
        let uuid = if let Some(id) = id {
            let mut entry = candidate.entry_mut(id).ok_or("条目不存在")?;
            if entry.parent_mut().id() != group_id {
                entry
                    .move_to(group_id)
                    .map_err(|e| format!("移动条目失败: {e}"))?;
                entry.times.location_changed = Some(keepass::db::Times::now());
            }
            {
                let mut tracked = entry.track_changes();
                write_editor_fields(
                    &mut tracked.as_mut(),
                    request,
                    &payloads,
                    &autotype,
                    foreground,
                );
                tracked.times.last_modification = Some(keepass::db::Times::now());
            }
            trim_entry_history(&mut entry, cap);
            let sizes = super::entries::snapshot_sizes(&entry.as_ref());
            trim_entry_history_by_size(&mut entry, max_size, &sizes);
            entry.id().to_string()
        } else {
            let mut group = candidate.group_mut(group_id).ok_or("目标分组不存在")?;
            let mut entry = group.add_entry();
            write_editor_fields(&mut entry, request, &payloads, &autotype, foreground);
            if let Some(rules) = rules {
                entry.set(
                    FIELD_WEBSITE_RULES,
                    Value::protected(rules.as_json().to_owned()),
                );
            }
            entry.id().to_string()
        };
        let original = self.db.replace(candidate);
        let dirty = self.dirty;
        let revision = self.revision;
        let modified = self.modified_at.clone();
        let cache = self.cached_snapshot.take();
        self.mark_dirty();
        match self.snapshot_without_icons() {
            Ok(state) => Ok(EntrySaveResult { uuid, state }),
            Err(error) => {
                self.db = original;
                self.dirty = dirty;
                self.revision = revision;
                self.modified_at = modified;
                self.cached_snapshot = cache;
                Err(error)
            }
        }
    }
}
