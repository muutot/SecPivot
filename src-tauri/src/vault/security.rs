//! OTP, favorites, favicons, auto-type context, password access, security
//! report and CSV export helpers on the open `VaultSession` (extracted from
//! mod.rs).

use super::entries::history_diff;
use super::helpers::{
    otp_kind_name, parse_entry_id, parse_entry_otp_spec, recycle_bin_id, walk_match,
    walk_match_candidates, walk_ref_match,
};
use super::serialize::{
    collect_favicon_hosts, escape_csv, estimate_entropy, extract_host, format_iso,
};
use super::*;
use crate::crypto::otp;
use crate::platform::autotype::{self, AutotypeContext};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use keepass::db::{EntryId, GroupId, Icon, Value};
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

    /// In-memory attachment preview: text stays in memory as utf8, raster
    /// images become `data:` URLs, anything else reports as binary. Preview
    /// payloads are capped at [`PREVIEW_MAX_BYTES`]; the full byte size is
    /// still reported so the UI can show the truncation.
    pub fn attachment_preview(&self, uuid: &str, name: &str) -> Result<AttachmentPreview, String> {
        const PREVIEW_MAX_BYTES: usize = 2 * 1024 * 1024;
        let data = self.attachment_data(uuid, name)?;
        let truncated = data.len() > PREVIEW_MAX_BYTES;
        let slice = &data[..data.len().min(PREVIEW_MAX_BYTES)];
        let size = data.len();
        if let Some(mime) = preview_image_mime(name) {
            Ok(AttachmentPreview {
                kind: "image".into(),
                data: format!("data:{mime};base64,{}", BASE64.encode(slice)),
                size,
                truncated,
            })
        } else if is_preview_text_name(name) {
            Ok(AttachmentPreview {
                kind: "text".into(),
                data: String::from_utf8_lossy(slice).into_owned(),
                size,
                truncated,
            })
        } else {
            Ok(AttachmentPreview {
                kind: "binary".into(),
                data: String::new(),
                size,
                truncated,
            })
        }
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
    /// new custom icon is created). Marks the session dirty when any icon
    /// bytes were actually written, so a manual save (or the favicon
    /// auto-save path) persists the change; persisting is the caller's job.
    pub fn apply_favicons(
        &mut self,
        jobs: &[FaviconJob],
        fetched: Vec<FaviconFetch>,
    ) -> Result<(), String> {
        let db = self.require_db_mut()?;
        let jobs: HashMap<&str, &FaviconJob> = jobs.iter().map(|j| (j.host.as_str(), j)).collect();
        let mut changed = false;
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
                            changed = true;
                        }
                    }
                    id
                }
                _ => {
                    let Some(mut first_entry) = db.entry_mut(first_id) else {
                        continue;
                    };
                    let id = first_entry.set_icon_custom_new(item.bytes.clone()).id();
                    changed = true;
                    id
                }
            };
            for uuid in job.entry_uuids.iter().skip(1) {
                let Some(mut entry) = db.entry_mut(parse_entry_id(uuid)?) else {
                    continue;
                };
                let _ = entry.set_icon_custom(icon_id);
            }
        }
        if changed {
            self.mark_dirty();
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
            walk_ref_match(db.root(), bin_id, spec, &mut found, true);
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
        walk_match(db.root(), bin_id, &lower, &mut best, true);
        best.map(|(_, uuid)| uuid)
            .ok_or_else(|| "没有找到匹配的条目".to_owned())
    }

    /// Collect every entry that matches the focused window title (same
    /// scoring as `autotype_match`), best first, capped for the picker UI.
    pub fn autotype_match_candidates(
        &self,
        window_title: &str,
    ) -> Result<Vec<AutotypeCandidate>, String> {
        let db = self.require_db()?;
        let lower = window_title.to_lowercase();
        if lower.trim().is_empty() {
            return Err("目标窗口标题为空".to_owned());
        }
        let bin_id = recycle_bin_id(db);
        let mut scored: Vec<(i32, String)> = Vec::new();
        walk_match_candidates(db.root(), bin_id, &lower, &mut scored, true);
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

        let mut candidates = Vec::new();
        for (_, uuid) in scored {
            let id = parse_entry_id(&uuid)?;
            let Some(entry) = db.entry(id) else {
                continue;
            };
            candidates.push(AutotypeCandidate {
                session_id: String::new(),
                title: entry.get_title().unwrap_or_default().to_owned(),
                username: entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
                uuid,
            });
            if candidates.len() >= 8 {
                break;
            }
        }
        if candidates.is_empty() {
            return Err("没有找到匹配的条目".to_owned());
        }
        Ok(candidates)
    }

    pub fn set_pending_autotype_window(&mut self, window_title: Option<String>) {
        self.pending_autotype_window = window_title;
    }

    pub fn take_pending_autotype_window(&mut self) -> Option<String> {
        self.pending_autotype_window.take()
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
        let bin_id = recycle_bin_id(db);
        let mut total = 0usize;
        let mut empty: Vec<String> = Vec::new();
        let mut weak: Vec<WeakEntry> = Vec::new();
        let mut by_password: HashMap<String, Vec<String>> = HashMap::new();

        fn scan(
            group: &keepass::db::GroupRef<'_>,
            bin_id: Option<GroupId>,
            total: &mut usize,
            empty: &mut Vec<String>,
            weak: &mut Vec<WeakEntry>,
            by_password: &mut HashMap<String, Vec<String>>,
        ) {
            // Recycled entries are excluded like every other maintenance
            // view (expired/similar/timeline): deleted junk must not inflate
            // the weak-password alarm list.
            if Some(group.id()) == bin_id {
                return;
            }
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
                scan(&child, bin_id, total, empty, weak, by_password);
            }
        }
        scan(
            &db.root(),
            bin_id,
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

    /// Group entries whose passwords are similar (at most two edits apart).
    /// Passwords never leave the session — groups carry only uuid/title/
    /// username. Exact duplicates are excluded (see `security_report`), the
    /// recycle bin is skipped, and the analysis is capped so huge databases
    /// stay responsive.
    pub fn similar_passwords(&self) -> Result<Vec<SimilarPasswordGroup>, String> {
        const MAX_ANALYZED: usize = 2000;
        const MAX_EDITS: usize = 2;
        let db = self.require_db()?;
        let bin_id = recycle_bin_id(db);

        let mut entries: Vec<(String, String, String, String)> = Vec::new();
        fn collect(
            group: &keepass::db::GroupRef<'_>,
            bin_id: Option<GroupId>,
            entries: &mut Vec<(String, String, String, String)>,
        ) {
            if Some(group.id()) == bin_id {
                return;
            }
            for entry in group.entries() {
                let password = entry.get(FIELD_PASSWORD).unwrap_or_default();
                if password.is_empty() {
                    continue;
                }
                entries.push((
                    entry.id().uuid().to_string(),
                    entry.get_title().unwrap_or_default().to_owned(),
                    entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
                    password.to_owned(),
                ));
            }
            for child in group.groups() {
                collect(&child, bin_id, entries);
            }
        }
        collect(&db.root(), bin_id, &mut entries);
        entries.truncate(MAX_ANALYZED);

        let mut parent: Vec<usize> = (0..entries.len()).collect();
        fn find(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        fn union(parent: &mut [usize], a: usize, b: usize) {
            let ra = find(parent, a);
            let rb = find(parent, b);
            if ra != rb {
                parent[rb] = ra;
            }
        }

        for (i, (_, _, _, a)) in entries.iter().enumerate() {
            for (j, (_, _, _, b)) in entries.iter().enumerate().skip(i + 1) {
                let len_diff = a.len().abs_diff(b.len());
                if len_diff > MAX_EDITS {
                    continue;
                }
                if a == b {
                    continue;
                }
                if levenshtein_at_most(a, b, MAX_EDITS) {
                    union(&mut parent, i, j);
                }
            }
        }

        let mut clusters: HashMap<usize, Vec<usize>> = HashMap::new();
        for (index, _) in entries.iter().enumerate() {
            let root = find(&mut parent, index);
            clusters.entry(root).or_default().push(index);
        }
        let mut groups: Vec<SimilarPasswordGroup> = clusters
            .into_values()
            .filter(|indices| indices.len() > 1)
            .map(|indices| {
                let mut members: Vec<SimilarEntry> = indices
                    .iter()
                    .map(|&index| {
                        let (uuid, title, username, _) = &entries[index];
                        SimilarEntry {
                            uuid: uuid.clone(),
                            title: title.clone(),
                            username: username.clone(),
                        }
                    })
                    .collect();
                members.sort_by(|a, b| a.title.cmp(&b.title));
                SimilarPasswordGroup { entries: members }
            })
            .collect();
        groups.sort_by_key(|g| std::cmp::Reverse(g.entries.len()));
        Ok(groups)
    }

    /// List entries whose expiry is in the past (recycle bin excluded), for
    /// the maintenance view. No secrets are included.
    pub fn expired_entries(&self) -> Result<Vec<ExpiredEntry>, String> {
        let db = self.require_db()?;
        let bin_id = recycle_bin_id(db);
        let now = chrono::Utc::now().naive_utc();
        let mut out = Vec::new();

        fn walk(
            group: &keepass::db::GroupRef<'_>,
            bin_id: Option<GroupId>,
            now: chrono::NaiveDateTime,
            out: &mut Vec<ExpiredEntry>,
        ) {
            if Some(group.id()) == bin_id {
                return;
            }
            for entry in group.entries() {
                let expired = entry.times.expires == Some(true)
                    && entry.times.expiry.is_some_and(|expiry| expiry < now);
                if expired {
                    out.push(ExpiredEntry {
                        uuid: entry.id().uuid().to_string(),
                        title: entry.get_title().unwrap_or_default().to_owned(),
                        username: entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
                        url: entry.get(FIELD_URL).unwrap_or_default().to_owned(),
                        expires: entry.times.expiry.map(format_iso).unwrap_or_default(),
                    });
                }
            }
            for child in group.groups() {
                walk(&child, bin_id, now, out);
            }
        }
        walk(&db.root(), bin_id, now, &mut out);
        out.sort_by(|a, b| a.expires.cmp(&b.expires));
        Ok(out)
    }

    /// Vault-wide change timeline: every transition between consecutive
    /// snapshots of an entry (and each newest snapshot → current state) as one
    /// event, newest first. Recycle-bin entries are excluded and the result is
    /// capped so huge databases stay responsive; no secrets cross the wire.
    pub fn change_timeline(&self) -> Result<Vec<ChangeTimelineEvent>, String> {
        const MAX_EVENTS: usize = 500;
        let db = self.require_db()?;
        let bin_id = recycle_bin_id(db);
        let mut events: Vec<ChangeTimelineEvent> = Vec::new();

        fn walk(
            group: &keepass::db::GroupRef<'_>,
            bin_id: Option<GroupId>,
            events: &mut Vec<ChangeTimelineEvent>,
        ) {
            if Some(group.id()) == bin_id {
                return;
            }
            for entry in group.entries() {
                let count = entry
                    .history
                    .as_ref()
                    .map(|history| history.get_entries().len())
                    .unwrap_or(0);
                // History snapshots are stored newest first: transition t
                // pairs snapshot t (older) with the current entry for t == 0,
                // or with snapshot t-1 (newer) otherwise. The event time is
                // the newer side's last modification — when either side lacks
                // a timestamp the event cannot be placed on a timeline and is
                // skipped.
                for t in 0..count {
                    let Some(older) = entry.historical(t) else {
                        continue;
                    };
                    let (diff, title, username, time) = if t == 0 {
                        let Some(time) = entry.times.last_modification else {
                            continue;
                        };
                        (
                            history_diff(&older, &entry),
                            entry.get_title().unwrap_or_default().to_owned(),
                            entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
                            format_iso(time),
                        )
                    } else {
                        let Some(newer) = entry.historical(t - 1) else {
                            continue;
                        };
                        let Some(time) = newer.times.last_modification else {
                            continue;
                        };
                        (
                            history_diff(&older, &newer),
                            newer.get_title().unwrap_or_default().to_owned(),
                            newer.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
                            format_iso(time),
                        )
                    };
                    events.push(ChangeTimelineEvent {
                        uuid: entry.id().uuid().to_string(),
                        title,
                        username,
                        time,
                        diff,
                    });
                }
            }
            for child in group.groups() {
                walk(&child, bin_id, events);
            }
        }
        walk(&db.root(), bin_id, &mut events);
        events.sort_by(|a, b| b.time.cmp(&a.time));
        events.truncate(MAX_EVENTS);
        Ok(events)
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

    /// Export a self-contained, print-friendly HTML emergency sheet. When
    /// `include_passwords` is set, entry passwords are embedded and the sheet
    /// carries a visible plaintext warning.
    pub fn export_emergency_sheet(
        &self,
        path: &str,
        include_passwords: bool,
    ) -> Result<(), String> {
        let content = self.emergency_sheet_content(include_passwords)?;
        write_csv_file(path, &content)
    }

    /// Build the HTML payload under the lock; the caller writes it outside
    /// the lock (same pattern as `export_csv_content`).
    pub(crate) fn emergency_sheet_content(
        &self,
        include_passwords: bool,
    ) -> Result<String, String> {
        let db = self.require_db()?;
        type SheetRow = (String, String, String, String, String);
        type SheetSection = (String, Vec<SheetRow>);
        let mut sections: Vec<SheetSection> = Vec::new();

        fn walk(
            group: &keepass::db::GroupRef<'_>,
            group_path: &str,
            sections: &mut Vec<SheetSection>,
        ) {
            let rows = group
                .entries()
                .map(|entry| {
                    (
                        entry.get_title().unwrap_or_default().to_owned(),
                        entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
                        entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned(),
                        entry.get(FIELD_URL).unwrap_or_default().to_owned(),
                        entry.get(FIELD_NOTES).unwrap_or_default().to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            if !rows.is_empty() {
                sections.push((group_path.to_owned(), rows));
            }
            for child in group.groups() {
                let child_path = if group_path.is_empty() {
                    child.name.clone()
                } else {
                    format!("{group_path} / {}", child.name)
                };
                walk(&child, &child_path, sections);
            }
        }
        walk(&db.root(), "", &mut sections);

        let mut html = String::from(
            "<!doctype html>\n<html lang=\"zh-CN\"><head><meta charset=\"utf-8\">\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
             <title>SecPivot 应急表</title><style>\
             body{font-family:system-ui,-apple-system,'Segoe UI',sans-serif;margin:24px;color:#1a1a1a;}\
             h1{font-size:20px;margin:0 0 4px;} .meta{color:#666;font-size:12px;margin:0 0 16px;}\
             .warning{color:#b00020;border:1px solid #b00020;padding:8px 10px;border-radius:6px;margin:0 0 16px;font-size:13px;}\
             h2{font-size:15px;border-bottom:1px solid #ddd;padding-bottom:4px;margin:20px 0 8px;}\
             table{border-collapse:collapse;width:100%;margin-bottom:8px;}\
             th,td{border:1px solid #ddd;padding:5px 8px;font-size:12px;text-align:left;vertical-align:top;}\
             th{background:#f5f5f5;} .mono{font-family:Consolas,monospace;word-break:break-all;}\
             @media print{body{margin:10mm;} .warning{border-color:#b00020;color:#b00020;} a{color:inherit;}}\
             </style></head><body>",
        );
        html.push_str("<h1>SecPivot 应急表</h1>");
        html.push_str("<p class=\"meta\">导出时间：");
        html.push_str(&chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        html.push_str(" · 共 ");
        let total: usize = sections.iter().map(|(_, rows)| rows.len()).sum();
        html.push_str(&total.to_string());
        html.push_str(" 个条目</p>");
        if include_passwords {
            html.push_str(
                "<p class=\"warning\">本文件包含明文密码！请妥善保管，使用后立即删除。</p>",
            );
        }
        for (group_path, rows) in &sections {
            let heading = if group_path.is_empty() {
                "根分组"
            } else {
                group_path
            };
            html.push_str("<h2>");
            html.push_str(&escape_html(heading));
            html.push_str("</h2><table><thead><tr><th>标题</th><th>用户名</th>");
            if include_passwords {
                html.push_str("<th>密码</th>");
            }
            html.push_str("<th>网址</th><th>备注</th></tr></thead><tbody>");
            for (title, username, password, url, notes) in rows {
                html.push_str("<tr><td>");
                html.push_str(&escape_html(title));
                html.push_str("</td><td>");
                html.push_str(&escape_html(username));
                html.push_str("</td>");
                if include_passwords {
                    html.push_str("<td class=\"mono\">");
                    html.push_str(&escape_html(password));
                    html.push_str("</td>");
                }
                html.push_str("<td>");
                if url.is_empty() {
                    html.push_str("<span></span>");
                } else {
                    html.push_str("<a href=\"");
                    html.push_str(&escape_html(url));
                    html.push_str("\">");
                    html.push_str(&escape_html(url));
                    html.push_str("</a>");
                }
                html.push_str("</td><td>");
                html.push_str(&escape_html(notes));
                html.push_str("</td></tr>");
            }
            html.push_str("</tbody></table>");
        }
        html.push_str("</body></html>\n");
        Ok(html)
    }

    /// Export all entries as a KeePass 2.x XML file (passwords included).
    pub fn export_xml(&self, path: &str) -> Result<(), String> {
        let content = self.export_xml_content()?;
        write_csv_file(path, &content)
    }

    /// Build the KeePass 2.x XML payload under the lock; the caller writes it
    /// outside the lock (same pattern as `export_csv_content`). The layout
    /// mirrors the official `File ▸ Export ▸ KeePass XML` output closely
    /// enough for re-import by SecPivot and KeePass: nested `<Group>`s carry
    /// UUID + name, entries carry their standard string fields, the TOTP seed
    /// (`otp`) and custom fields. Passwords and protected custom fields use
    /// the KeePass convention `Protected="True"` + Base64 value.
    pub(crate) fn export_xml_content(&self) -> Result<String, String> {
        let db = self.require_db()?;
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"yes\"?>\n\
             <KeePassFile><Meta><Generator>SecPivot</Generator></Meta><Root>",
        );
        fn write_string_field(xml: &mut String, key: &str, value: &str, protected: bool) {
            xml.push_str("<String><Key>");
            xml.push_str(&escape_html(key));
            xml.push_str("</Key><Value");
            if protected {
                xml.push_str(" Protected=\"True\">");
                xml.push_str(&BASE64.encode(value.as_bytes()));
            } else {
                xml.push('>');
                xml.push_str(&escape_html(value));
            }
            xml.push_str("</Value></String>");
        }
        fn write_entry(xml: &mut String, entry: &keepass::db::EntryRef<'_>) {
            xml.push_str("<Entry><UUID>");
            xml.push_str(&BASE64.encode(entry.id().uuid().as_bytes()));
            xml.push_str("</UUID>");
            write_string_field(xml, "Title", entry.get_title().unwrap_or_default(), false);
            write_string_field(
                xml,
                "UserName",
                entry.get(FIELD_USERNAME).unwrap_or_default(),
                false,
            );
            write_string_field(
                xml,
                "Password",
                entry.get(FIELD_PASSWORD).unwrap_or_default(),
                true,
            );
            write_string_field(xml, "URL", entry.get(FIELD_URL).unwrap_or_default(), false);
            write_string_field(
                xml,
                "Notes",
                entry.get(FIELD_NOTES).unwrap_or_default(),
                false,
            );
            if let Some(seed) = entry.get_raw_otp_value() {
                if !seed.is_empty() {
                    write_string_field(xml, "otp", seed, false);
                }
            }
            let mut names: Vec<&String> = entry
                .fields
                .keys()
                .filter(|name| !name.is_empty() && !RESERVED_FIELDS.contains(&name.as_str()))
                .collect();
            names.sort();
            for name in names {
                let value = &entry.fields[name];
                write_string_field(xml, name, value.get(), value.is_protected());
            }
            xml.push_str("</Entry>");
        }
        fn walk(xml: &mut String, group: &keepass::db::GroupRef<'_>) {
            xml.push_str("<Group><UUID>");
            xml.push_str(&BASE64.encode(group.id().uuid().as_bytes()));
            xml.push_str("</UUID><Name>");
            xml.push_str(&escape_html(&group.name));
            xml.push_str("</Name>");
            for entry in group.entries() {
                write_entry(xml, &entry);
            }
            for child in group.groups() {
                walk(xml, &child);
            }
            xml.push_str("</Group>");
        }
        walk(&mut xml, &db.root());
        xml.push_str("</Root></KeePassFile>\n");
        Ok(xml)
    }
}

/// HTML-escape text for the emergency sheet (never inject markup).
fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Whether `a` and `b` are at most `k` edits apart (Levenshtein with early
/// exit; `None` when the DP exceeds the bound).
fn levenshtein_at_most(a: &str, b: &str, k: usize) -> bool {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > k {
        return false;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        let mut row_min = usize::MAX;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let value = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
            curr[j] = value;
            row_min = row_min.min(value);
        }
        if row_min > k {
            return false;
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()] <= k
}

/// MIME type for raster image attachments that can preview in memory.
fn preview_image_mime(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    match lower.rsplit('.').next() {
        Some("png") => Some("image/png"),
        Some("jpg") | Some("jpeg") => Some("image/jpeg"),
        Some("gif") => Some("image/gif"),
        Some("webp") => Some("image/webp"),
        Some("bmp") => Some("image/bmp"),
        _ => None,
    }
}

/// Extensions whose content is previewed as text (never executed).
fn is_preview_text_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    matches!(
        ext,
        "txt"
            | "md"
            | "log"
            | "json"
            | "xml"
            | "csv"
            | "yml"
            | "yaml"
            | "toml"
            | "ini"
            | "cfg"
            | "conf"
            | "html"
            | "htm"
            | "css"
            | "js"
            | "ts"
            | "rs"
            | "py"
            | "sh"
            | "bat"
            | "ps1"
    )
}
