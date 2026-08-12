//! OTP, favorites, favicons, auto-type context, password access, security
//! report and CSV export helpers on the open `VaultSession` (extracted from
//! mod.rs).

use super::helpers::{
    otp_kind_name, parse_entry_id, parse_entry_otp_spec, recycle_bin_id, walk_match, walk_ref_match,
};
use super::serialize::{collect_favicon_hosts, escape_csv, estimate_entropy, extract_host};
use super::*;
use crate::crypto::otp;
use crate::platform::autotype::{self, AutotypeContext};
use keepass::db::{EntryId, Icon, Value};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

impl VaultSession {
    pub(crate) fn attachment_data(&self, uuid: &str, name: &str) -> Result<Vec<u8>, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        let attachment = entry
            .attachment_by_name(name)
            .ok_or_else(|| "附件不存在".to_owned())?;
        Ok(attachment.data.get().to_vec())
    }

    /// Convenience used by tests and callers that may hold the lock anyway.
    pub fn save_attachment(&self, uuid: &str, name: &str, dest: &str) -> Result<(), String> {
        let data = self.attachment_data(uuid, name)?;
        write_attachment_file(&data, dest)
    }

    /// Toggle the favorite/pin marker on an entry (persisted as a custom field).
    pub fn toggle_favorite(&mut self, uuid: &str) -> Result<VaultState, String> {
        self.toggle_favorite_delta(uuid)?;
        self.snapshot_without_icons()
    }

    /// Same mutation as `toggle_favorite`, but returns only the delta (new
    /// revision + affected entry) instead of rebuilding/serializing the whole
    /// tree — the renderer applies it to its local state.
    pub fn toggle_favorite_delta(&mut self, uuid: &str) -> Result<MutationDelta, String> {
        let favorite = {
            let db = self.require_db_mut()?;
            let id = parse_entry_id(uuid)?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            let favorite = entry.get(FIELD_FAVORITE) != Some(FIELD_FAVORITE_TRUE);
            if favorite {
                entry.set(
                    FIELD_FAVORITE,
                    Value::unprotected(FIELD_FAVORITE_TRUE.to_owned()),
                );
            } else {
                entry.fields.remove(FIELD_FAVORITE);
            }
            favorite
        };
        self.mark_dirty();
        Ok(MutationDelta::Favorite {
            revision: self.revision,
            uuid: uuid.to_owned(),
            favorite,
        })
    }

    /// Compute the one-time password for an entry that carries an OTP seed
    /// field. Detects the kind from the field name: `otp`/`TimeOtp` = TOTP,
    /// `HmacOtp` = HOTP, `SteamOtp`/`steam` = Steam Guard. HOTP advances its
    /// counter on every request and rewrites the seed field server-side (no
    /// history snapshot), so the next code uses the new counter.
    pub fn totp_code(&mut self, uuid: &str) -> Result<TotpCode, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("读取系统时间失败: {e}"))?
            .as_secs();
        let id = parse_entry_id(uuid)?;
        let (is_hotp, spec) = {
            let db = self.require_db()?;
            let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
            let spec = parse_entry_otp_spec(&entry)?;
            (spec.kind == otp::OtpKind::Hotp, spec)
        };
        let code = otp::compute(&spec, now)?;
        if is_hotp {
            self.advance_hotp_counter(id, &spec)?;
        }
        Ok(TotpCode {
            code: code.code,
            kind: otp_kind_name(spec.kind).to_owned(),
            valid_for: code.valid_for,
            period: code.period,
            counter: code.counter,
        })
    }

    /// Advance an `HmacOtp` entry's counter by rewriting the seed field with
    /// `counter+1`. Mutates without `track_changes` so showing a code does not
    /// pollute the entry's history; the vault is left dirty so the next save
    /// persists the new counter.
    fn advance_hotp_counter(&mut self, id: EntryId, spec: &otp::OtpSpec) -> Result<(), String> {
        let next = {
            let mut next = spec.clone();
            next.counter = spec.counter + 1;
            otp::render_hotp_seed(&next)
        };
        {
            let db = self.require_db_mut()?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            entry.set(FIELD_HMAC_OTP, Value::unprotected(next));
        }
        self.mark_dirty();
        Ok(())
    }

    /// Distinct URL hosts referenced by entry URLs, with the entries per host
    /// (KeePass "Download Favicons" job list). Non-http(s) URLs are skipped.
    pub fn favicon_jobs(&self) -> Result<Vec<FaviconJob>, String> {
        let db = self.require_db()?;
        let mut map: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        collect_favicon_hosts(&db.root(), &mut map);
        Ok(map
            .into_iter()
            .map(|(host, entry_uuids)| FaviconJob { host, entry_uuids })
            .collect())
    }

    /// Favicon jobs restricted to the given entry UUIDs (multi-select download);
    /// only those entries get icons, never their same-host siblings. Entries
    /// without a parseable http(s) URL are skipped; unknown uuids are ignored.
    pub fn favicon_jobs_selected(&self, uuids: &[String]) -> Result<Vec<FaviconJob>, String> {
        let db = self.require_db()?;
        let mut map: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for uuid in uuids {
            let id = parse_entry_id(uuid)?;
            let Some(entry) = db.entry(id) else {
                continue;
            };
            if let Some(host) = extract_host(entry.get(FIELD_URL).unwrap_or_default()) {
                map.entry(host)
                    .or_default()
                    .push(entry.id().uuid().to_string());
            }
        }
        Ok(map
            .into_iter()
            .map(|(host, entry_uuids)| FaviconJob { host, entry_uuids })
            .collect())
    }

    /// Store fetched favicon bytes as database custom icons and point every
    /// entry of the same host at that icon. An entry that already references
    /// an identical icon keeps it; otherwise the icon data is replaced (or a
    /// new custom icon is created). Persisting is the caller's job.
    pub fn apply_favicons(
        &mut self,
        jobs: &[FaviconJob],
        fetched: Vec<FaviconFetch>,
    ) -> Result<(), String> {
        let db = self.require_db_mut()?;
        let jobs: HashMap<&str, &FaviconJob> = jobs.iter().map(|j| (j.host.as_str(), j)).collect();
        for item in fetched {
            let Some(job) = jobs.get(item.host.as_str()) else {
                continue;
            };
            let Some(first) = job.entry_uuids.first() else {
                continue;
            };
            let first_id = parse_entry_id(first)?;
            let existing = {
                let Some(first_entry) = db.entry_mut(first_id) else {
                    continue;
                };
                first_entry.icon().cloned()
            };
            let icon_id = match existing {
                Some(Icon::Custom(id)) => {
                    let identical = db
                        .custom_icon(id)
                        .is_some_and(|icon| icon.data == item.bytes);
                    if !identical {
                        if let Some(mut icon) = db.custom_icon_mut(id) {
                            icon.data = item.bytes.clone();
                        }
                    }
                    id
                }
                _ => {
                    let Some(mut first_entry) = db.entry_mut(first_id) else {
                        continue;
                    };
                    first_entry.set_icon_custom_new(item.bytes.clone()).id()
                }
            };
            for uuid in job.entry_uuids.iter().skip(1) {
                let Some(mut entry) = db.entry_mut(parse_entry_id(uuid)?) else {
                    continue;
                };
                let _ = entry.set_icon_custom(icon_id);
            }
        }
        Ok(())
    }

    /// Collect the fields an auto-type sequence can substitute, for the given entry.
    pub fn autotype_context(&self, uuid: &str) -> Result<AutotypeContext, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        Ok(AutotypeContext {
            username: entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
            password: entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned(),
            title: entry.get_title().unwrap_or_default().to_owned(),
            url: entry.get(FIELD_URL).unwrap_or_default().to_owned(),
            notes: entry.get(FIELD_NOTES).unwrap_or_default().to_owned(),
        })
    }

    /// Expand `{REF:...}` field references in an auto-type sequence against
    /// the database. Entries inside the recycle bin are not referenceable.
    pub fn expand_autotype_sequence(&self, sequence: &str) -> Result<String, String> {
        let db = self.require_db()?;
        let bin_id = recycle_bin_id(db);
        autotype::expand_refs(sequence, |spec| {
            let mut found: Option<String> = None;
            walk_ref_match(db.root(), bin_id, spec, &mut found);
            found
        })
        .map_err(|e| e.to_string())
    }

    /// Resolve the effective auto-type sequence for an entry, following the
    /// KeePass lookup order: the entry's own `AutoType.default_sequence`,
    /// then the nearest ancestor group's `default_autotype_sequence` (walking
    /// up the tree), then the global default. `AutoType.enabled=false` on the
    /// entry, or `enable_autotype=false` on an ancestor group, yields `None`
    /// (auto-type is disabled for this entry entirely).
    pub fn resolve_autotype_sequence(&self, uuid: &str) -> Result<Option<String>, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        // Entry-level AutoType: explicitly disabled entries never auto-type.
        if let Some(autotype) = &entry.autotype {
            if !autotype.enabled {
                return Ok(None);
            }
            if let Some(seq) = autotype.default_sequence.as_deref() {
                if !seq.trim().is_empty() {
                    return Ok(Some(seq.to_owned()));
                }
            }
        }
        // Walk ancestor groups; the nearest group with a sequence wins.
        let mut gid = entry.parent().id();
        loop {
            let group = db.group(gid).ok_or_else(|| "分组不存在".to_owned())?;
            if group.enable_autotype == Some(false) {
                return Ok(None);
            }
            if let Some(seq) = group.default_autotype_sequence.as_deref() {
                if !seq.trim().is_empty() {
                    return Ok(Some(seq.to_owned()));
                }
            }
            match group.parent() {
                Some(parent) => gid = parent.id(),
                None => break,
            }
        }
        Ok(None)
    }

    /// Resolve the sequence for a global-hotkey auto-type run given the
    /// focused window title. KeePass order: the first matching window
    /// association wins, otherwise the entry/group default resolution applies.
    pub fn resolve_autotype_sequence_for_window(
        &self,
        uuid: &str,
        window_title: &str,
    ) -> Result<Option<String>, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        if let Some(autotype) = &entry.autotype {
            if !autotype.enabled {
                return Ok(None);
            }
            for association in &autotype.associations {
                if Self::window_title_matches(&association.window, window_title) {
                    if !association.sequence.trim().is_empty() {
                        return Ok(Some(association.sequence.clone()));
                    }
                    break;
                }
            }
            if let Some(sequence) = autotype.default_sequence.as_deref() {
                if !sequence.trim().is_empty() {
                    return Ok(Some(sequence.to_owned()));
                }
            }
        }
        let mut group_id = entry.parent().id();
        loop {
            let group = db.group(group_id).ok_or_else(|| "分组不存在".to_owned())?;
            if group.enable_autotype == Some(false) {
                return Ok(None);
            }
            if let Some(sequence) = group.default_autotype_sequence.as_deref() {
                if !sequence.trim().is_empty() {
                    return Ok(Some(sequence.to_owned()));
                }
            }
            match group.parent() {
                Some(parent) => group_id = parent.id(),
                None => break,
            }
        }
        Ok(None)
    }

    /// KeePass-style window matching for Auto-Type associations: case
    /// insensitive; a bare pattern matches any title containing it, and `*`
    /// acts as a glob wildcard (anchored prefix/suffix around the literal
    /// parts).
    pub fn window_title_matches(pattern: &str, title: &str) -> bool {
        let pattern = pattern.to_lowercase();
        let title = title.to_lowercase();
        if !pattern.contains('*') {
            return title.contains(&pattern);
        }
        let parts: Vec<&str> = pattern.split('*').filter(|part| !part.is_empty()).collect();
        let mut rest = title.as_str();
        for (index, part) in parts.iter().enumerate() {
            let Some(position) = rest.find(part) else {
                return false;
            };
            if index == 0 && !pattern.starts_with('*') && position != 0 {
                return false;
            }
            rest = &rest[position + part.len()..];
        }
        pattern.ends_with('*') || rest.is_empty()
    }

    /// Best-matching entry for global auto-type given the title of the window
    /// in focus. Matches the URL host or the entry title against the window
    /// title (case-insensitive); entries inside the recycle bin are skipped.
    /// Returns the entry UUID.
    pub fn autotype_match(&self, window_title: &str) -> Result<String, String> {
        let db = self.require_db()?;
        let lower = window_title.to_lowercase();
        if lower.trim().is_empty() {
            return Err("目标窗口标题为空".to_owned());
        }
        let bin_id = recycle_bin_id(db);
        let mut best: Option<(i32, String)> = None;
        walk_match(db.root(), bin_id, &lower, &mut best);
        best.map(|(_, uuid)| uuid)
            .ok_or_else(|| "没有找到匹配的条目".to_owned())
    }
}

impl VaultSession {
    pub fn get_entry_password(&self, uuid: &str) -> Result<String, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        Ok(entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned())
    }

    /// Fetch a single entry's TOTP seed on demand (never part of `VaultState`).
    /// `None` means the entry has no seed configured.
    pub fn get_entry_totp(&self, uuid: &str) -> Result<Option<String>, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        Ok(entry.get_raw_otp_value().map(str::to_owned))
    }

    /// Fetch one custom field's value on demand. Protected custom fields are
    /// never part of `VaultState`/`VaultEntry` — the value crosses the IPC only
    /// on an explicit reveal/copy/edit action. `None` when the field does not
    /// exist or is a reserved standard column.
    pub fn get_custom_field_value(&self, uuid: &str, name: &str) -> Result<Option<String>, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let trimmed = name.trim();
        if trimmed.is_empty() || RESERVED_FIELDS.contains(&trimmed) {
            return Ok(None);
        }
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        Ok(entry.get(trimmed).map(str::to_owned))
    }

    /// Analyze all entries server-side; no passwords leave the session.
    pub fn security_report(&self) -> Result<SecurityReport, String> {
        let db = self.require_db()?;
        let mut total = 0usize;
        let mut empty: Vec<String> = Vec::new();
        let mut weak: Vec<WeakEntry> = Vec::new();
        let mut by_password: HashMap<String, Vec<String>> = HashMap::new();

        fn scan(
            group: &keepass::db::GroupRef<'_>,
            total: &mut usize,
            empty: &mut Vec<String>,
            weak: &mut Vec<WeakEntry>,
            by_password: &mut HashMap<String, Vec<String>>,
        ) {
            for entry in group.entries() {
                *total += 1;
                let password = entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned();
                by_password
                    .entry(password.clone())
                    .or_default()
                    .push(entry.id().uuid().to_string());
                // `QualityCheck=false` opts the entry out of password-quality
                // evaluation entirely (KeePass semantics): neither empty nor
                // weak passwords are flagged. Duplicate detection is a separate
                // reuse concern and still applies.
                if !entry.quality_check {
                    continue;
                }
                if password.is_empty() {
                    empty.push(entry.id().uuid().to_string());
                    continue;
                }
                let bits = estimate_entropy(&password);
                if bits < 72 {
                    weak.push(WeakEntry {
                        uuid: entry.id().uuid().to_string(),
                        bits,
                    });
                }
            }
            for child in group.groups() {
                scan(&child, total, empty, weak, by_password);
            }
        }
        scan(
            &db.root(),
            &mut total,
            &mut empty,
            &mut weak,
            &mut by_password,
        );

        weak.sort_by_key(|w| w.bits);
        let mut duplicates: Vec<DuplicatePasswords> = by_password
            .into_iter()
            .filter(|(_, uuids)| uuids.len() > 1)
            .map(|(_, uuids)| {
                let count = uuids.len();
                DuplicatePasswords { count, uuids }
            })
            .collect();
        duplicates.sort_by_key(|d| std::cmp::Reverse(d.count));

        Ok(SecurityReport {
            total,
            empty,
            weak,
            duplicates,
        })
    }

    /// Export all entries as CSV (passwords included) straight to a file.
    pub fn export_csv(&self, path: &str) -> Result<(), String> {
        let content = self.export_csv_content()?;
        write_csv_file(path, &content)
    }

    /// Build the CSV payload under the lock; the caller writes it outside
    /// the lock (see `write_csv_file`).
    pub(crate) fn export_csv_content(&self) -> Result<String, String> {
        let db = self.require_db()?;
        let mut lines = vec!["Group,Title,Username,Password,URL,Notes,TOTP,Favorite".to_owned()];

        fn walk(group: &keepass::db::GroupRef<'_>, group_path: &str, lines: &mut Vec<String>) {
            for entry in group.entries() {
                let favorite = if entry.get(FIELD_FAVORITE) == Some(FIELD_FAVORITE_TRUE) {
                    "true"
                } else {
                    "false"
                };
                let row = [
                    escape_csv(group_path),
                    escape_csv(entry.get_title().unwrap_or_default()),
                    escape_csv(entry.get(FIELD_USERNAME).unwrap_or_default()),
                    escape_csv(entry.get(FIELD_PASSWORD).unwrap_or_default()),
                    escape_csv(entry.get(FIELD_URL).unwrap_or_default()),
                    escape_csv(entry.get(FIELD_NOTES).unwrap_or_default()),
                    escape_csv(entry.get_raw_otp_value().unwrap_or_default()),
                    escape_csv(favorite),
                ];
                lines.push(row.join(","));
            }
            for child in group.groups() {
                let child_path = if group_path.is_empty() {
                    child.name.clone()
                } else {
                    format!("{group_path} / {}", child.name)
                };
                walk(&child, &child_path, lines);
            }
        }
        walk(&db.root(), "", &mut lines);

        Ok(format!("\u{FEFF}{}\r\n", lines.join("\r\n")))
    }
}
