//! Entry-level merge of the local vault with its remote counterpart
//! (官方同步·条目级合并).
//!
//! The heavy lifting — UUID matching, last-modification comparison, ordinary
//! history merge, deleted-object tracking, group/icon merge — comes from the keepass
//! crate's `Database::merge`. On top of it this module enforces SecPivot's
//! recycle-bin rule (回收站排除): bin contents never take part in the merge.
//! They are detached from both clones first, and the local bin is reattached
//! afterwards. A detached bin object that collides with an object the merge
//! kept active is resolved by timestamp: the newer change wins (deletion vs.
//! edit), and a winning deletion is recorded in `deleted_objects` so it
//! propagates on later merges.

use super::helpers::recycle_bin_id;
use chrono::NaiveDateTime;
use keepass::db::merge::{MergeError, MergeEventTarget, MergeEventType};
use keepass::db::{Entry, EntryId, Group, GroupId, History, Icon, Times};
use keepass::Database;
use std::collections::{HashMap, HashSet};

/// A bin entry detached for the merge, remembered with the group it hung
/// under inside the bin. All current/historical binaries are restored from
/// the immutable original local database after metadata is reattached.
struct DetachedEntry {
    id: EntryId,
    parent: GroupId,
    entry: Entry,
}

/// A bin subtree group detached for the merge. Only content fields are
/// reused — child id lists are rebuilt by re-adding groups/entries.
struct DetachedGroup {
    id: GroupId,
    parent: GroupId,
    group: Group,
}

#[derive(Default)]
struct DetachedBin {
    /// Parent groups come before their children, so reattach works top-down.
    groups: Vec<DetachedGroup>,
    entries: Vec<DetachedEntry>,
}

/// The merge-relevant timestamp of a detached bin object: when it was moved
/// into the bin (the deletion time), falling back to the last content edit
/// for legacy bins without a move stamp.
fn bin_change(times: &Times) -> Option<NaiveDateTime> {
    times.location_changed.or(times.last_modification)
}

/// The merge-relevant timestamp of an object that stayed active: the latest
/// of its last content edit and last move.
fn active_change(times: &Times) -> Option<NaiveDateTime> {
    times.last_modification.max(times.location_changed)
}

/// Content comparison ignores bookkeeping timestamps and history, but expiry
/// date/enabled are entry content and must remain part of version identity.
fn content_diverged(a: &Entry, b: &Entry) -> bool {
    if expiration_diverged(a, b) {
        return true;
    }
    !a.content_eq(b)
}

fn expiration_diverged(a: &Entry, b: &Entry) -> bool {
    a.times.expiry != b.times.expiry || a.times.expires != b.times.expires
}

/// The crate ignores expiration in its equal-current-time comparison. Capture
/// candidates before it changes winner timestamps, then reject only those still
/// divergent and present after group/entry deletion decisions have completed.
fn collect_equal_time_expiration_conflicts(dest: &Database, source: &Database) -> Vec<EntryId> {
    source
        .iter_all_entries()
        .filter_map(|entry| {
            let local = dest.entry(entry.id())?;
            let time = entry.times.last_modification?;
            (local.times.last_modification == Some(time) && expiration_diverged(&local, &entry))
                .then_some(entry.id())
        })
        .collect()
}

/// Capture the older current version in either direction. The crate does not
/// consider expiration when deciding whether to back up a destination version.
/// Exact timestamp/content deduplication after merge avoids duplicate backups.
fn collect_losing_versions(dest: &Database, source: &Database) -> Vec<(EntryId, Entry)> {
    fn walk(
        group: keepass::db::GroupRef<'_>,
        source: &Database,
        losers: &mut Vec<(EntryId, Entry)>,
    ) {
        for entry in group.entries() {
            if let Some(remote) = source.entry(entry.id()) {
                if let (Some(local_time), Some(remote_time)) = (
                    entry.times.last_modification,
                    remote.times.last_modification,
                ) {
                    if local_time != remote_time && content_diverged(&entry, &remote) {
                        let mut loser = if local_time > remote_time {
                            (*remote).clone()
                        } else {
                            (*entry).clone()
                        };
                        loser.history = None;
                        losers.push((entry.id(), loser));
                    }
                }
            }
        }
        for child in group.groups() {
            walk(child, source, losers);
        }
    }
    let mut losers = Vec::new();
    walk(dest.root(), source, &mut losers);
    losers
}

/// The crate skips history union when current modification times match.
/// Capture only the source histories that differ; a real current-content
/// conflict still rejects the whole merge before these snapshots are applied.
fn collect_equal_current_histories(dest: &Database, source: &Database) -> Vec<(EntryId, History)> {
    source
        .iter_all_entries()
        .filter_map(|entry| {
            let local = dest.entry(entry.id())?;
            let time = entry.times.last_modification?;
            if local.times.last_modification != Some(time) || local.history == entry.history {
                return None;
            }
            entry
                .history
                .as_ref()
                .map(|history| (entry.id(), history.clone()))
        })
        .collect()
}

/// Union versions by timestamp AND complete content, including expiration and
/// canonical attachment IDs. A shared timestamp does not identify a version.
/// Timestamp buckets avoid comparing every unrelated pair in large histories.
fn union_history(entry: &mut Entry, extra: impl Iterator<Item = Entry>) {
    let mut versions: Vec<Entry> = Vec::new();
    let mut buckets: HashMap<Option<NaiveDateTime>, Vec<usize>> = HashMap::new();
    let original = entry.history.take().unwrap_or_default();
    for mut snapshot in original.get_entries().iter().cloned().chain(extra) {
        snapshot.history = None;
        let bucket = buckets.entry(snapshot.times.last_modification).or_default();
        let duplicate = bucket.iter().any(|&index| {
            let existing = &versions[index];
            !content_diverged(existing, &snapshot)
        });
        if !duplicate {
            bucket.push(versions.len());
            versions.push(snapshot);
        }
    }
    versions.sort_by_key(|snapshot| std::cmp::Reverse(snapshot.times.last_modification));
    entry.history = Some(History::from_entries(versions));
}

fn push_losers_into_history(dest: &mut Database, losers: Vec<(EntryId, Entry)>) {
    for (id, loser) in losers {
        if let Some(mut entry) = dest.entry_mut(id) {
            union_history(&mut entry, std::iter::once(loser));
        }
    }
}

/// keepass 0.13.22 omits tags when it creates a source-only group. Its
/// subsequent equality check then rejects the group it just created. Clear
/// only those tags in the temporary source clone; the merge log determines
/// which created/updated groups receive the original tags after success.
/// Existing groups keep their tags here, including equal-time divergence.
fn prepare_new_group_tags(dest: &Database, source: &mut Database) {
    let mut pending = vec![source.root().id()];
    while let Some(id) = pending.pop() {
        let Some(group) = source.group(id) else {
            continue;
        };
        pending.extend(group.groups().map(|child| child.id()));
        if dest.group(id).is_none() {
            if let Some(mut group) = source.group_mut(id) {
                group.tags.clear();
            }
        }
    }
}

/// Snapshot a bin entry's complete content/version tree before detaching it.
fn snapshot_entry(db: &Database, id: EntryId, parent: GroupId) -> Option<DetachedEntry> {
    let entry = db.entry(id)?;
    Some(DetachedEntry {
        id,
        parent,
        entry: (*entry).clone(),
    })
}

/// Recursively snapshot a bin subtree group (parents before children).
fn snapshot_group(db: &Database, id: GroupId, parent: GroupId, bin: &mut DetachedBin) {
    let Some(group) = db.group(id) else { return };
    let group_clone = (*group).clone();
    let child_groups: Vec<GroupId> = group.groups().map(|g| g.id()).collect();
    let child_entries: Vec<EntryId> = group.entries().map(|e| e.id()).collect();
    bin.groups.push(DetachedGroup {
        id,
        parent,
        group: group_clone,
    });
    for entry_id in child_entries {
        if let Some(entry) = snapshot_entry(db, entry_id, id) {
            bin.entries.push(entry);
        }
    }
    for child_id in child_groups {
        snapshot_group(db, child_id, id, bin);
    }
}

/// Detach the recycle-bin contents (direct entries + whole subgroup trees)
/// from a database clone, returning everything needed to reattach them.
/// The bin group shell itself stays in place. Detachment uses the
/// non-tracking `remove`, which does not record `deleted_objects`.
fn detach_bin(db: &mut Database) -> DetachedBin {
    let mut bin = DetachedBin::default();
    let Some(bin_id) = recycle_bin_id(db) else {
        return bin;
    };
    let (entry_ids, group_ids) = match db.group(bin_id) {
        Some(group) => (
            group.entries().map(|e| e.id()).collect::<Vec<_>>(),
            group.groups().map(|g| g.id()).collect::<Vec<_>>(),
        ),
        None => return bin,
    };
    for entry_id in &entry_ids {
        if let Some(entry) = snapshot_entry(db, *entry_id, bin_id) {
            bin.entries.push(entry);
        }
    }
    for group_id in &group_ids {
        snapshot_group(db, *group_id, bin_id, &mut bin);
    }
    for entry_id in entry_ids {
        if let Some(entry) = db.entry_mut(entry_id) {
            entry.remove();
        }
    }
    // `GroupMut::remove` cascades: nested entries/groups go with the top one.
    for group_id in group_ids {
        if let Some(group) = db.group_mut(group_id) {
            group.remove();
        }
    }
    bin
}

/// Whether `parent` sits inside a dropped snapshot subtree.
fn snapshot_ancestor_dropped(
    bin: &DetachedBin,
    dropped: &HashSet<GroupId>,
    mut parent: GroupId,
) -> bool {
    loop {
        if dropped.contains(&parent) {
            return true;
        }
        match bin.groups.iter().find(|g| g.id == parent) {
            Some(group) => parent = group.parent,
            None => return false,
        }
    }
}

/// Resolve collisions between detached bin objects and objects the merge
/// kept active, then reattach the surviving bin contents. For a collision
/// the strictly newer timestamp wins: a newer local deletion removes the
/// active object from the merged tree (recorded in `deleted_objects` so it
/// propagates); a newer remote edit drops the bin copy (and, for groups, its
/// whole snapshot subtree). Exact ties keep the active edit: resurrecting an
/// entry the user can see and re-delete beats silently dropping an edit.
fn restore_bin(dest: &mut Database, bin: DetachedBin, original: &Database) -> Result<(), String> {
    let mut dropped_groups: HashSet<GroupId> = HashSet::new();
    for snap in &bin.groups {
        let Some(active_time) = dest.group(snap.id).map(|g| active_change(&g.times)) else {
            continue; // not active in the merged tree — nothing to resolve
        };
        let bin_time = bin_change(&snap.group.times);
        if bin_time > active_time {
            if let Some(mut group) = dest.group_mut(snap.id) {
                // GroupTrack::remove records deleted_objects (deletion wins).
                let _ = group.track_changes().remove();
            }
        } else {
            dropped_groups.insert(snap.id);
        }
    }

    let mut dropped_entries: HashSet<EntryId> = HashSet::new();
    let mut restored_entries = Vec::new();
    for snap in &bin.entries {
        let Some(active) = dest.entry(snap.id) else {
            continue;
        };
        let bin_time = bin_change(&snap.entry.times);
        let active_time = active_change(&active.times);
        if bin_time > active_time {
            if let Some(mut entry) = dest.entry_mut(snap.id) {
                // EntryTrack::remove records deleted_objects (deletion wins).
                entry.track_changes().remove();
            }
        } else {
            dropped_entries.insert(snap.id);
        }
    }

    // Reattach survivors top-down; objects inside a dropped group go with it.
    for snap in &bin.groups {
        if dropped_groups.contains(&snap.id)
            || snapshot_ancestor_dropped(&bin, &dropped_groups, snap.parent)
        {
            continue;
        }
        let Some(mut parent) = dest.group_mut(snap.parent) else {
            continue;
        };
        let Ok(mut group) = parent.add_group_with_id(snap.id) else {
            continue;
        };
        group.name = snap.group.name.clone();
        group.notes = snap.group.notes.clone();
        group.tags = snap.group.tags.clone();
        group.times = snap.group.times.clone();
        group.custom_data = snap.group.custom_data.clone();
        group.is_expanded = snap.group.is_expanded;
        group.default_autotype_sequence = snap.group.default_autotype_sequence.clone();
        group.enable_autotype = snap.group.enable_autotype;
        group.enable_searching = snap.group.enable_searching;
        match snap.group.icon() {
            Some(Icon::BuiltIn(index)) => group.set_icon_builtin(*index),
            Some(Icon::Custom(id)) => {
                let _ = group.set_icon_custom(*id);
            }
            None => {}
        }
    }
    for snap in &bin.entries {
        if dropped_entries.contains(&snap.id)
            || snapshot_ancestor_dropped(&bin, &dropped_groups, snap.parent)
        {
            continue;
        }
        let Some(mut parent) = dest.group_mut(snap.parent) else {
            continue;
        };
        let Ok(mut entry) = parent.add_entry_with_id(snap.id) else {
            continue;
        };
        entry.fields = snap.entry.fields.clone();
        entry.autotype = snap.entry.autotype.clone();
        entry.tags = snap.entry.tags.clone();
        entry.times = snap.entry.times.clone();
        entry.custom_data = snap.entry.custom_data.clone();
        entry.foreground_color = snap.entry.foreground_color.clone();
        entry.background_color = snap.entry.background_color.clone();
        entry.override_url = snap.entry.override_url.clone();
        entry.quality_check = snap.entry.quality_check;
        entry.history = snap.entry.history.clone();
        match snap.entry.icon() {
            Some(Icon::BuiltIn(index)) => entry.set_icon_builtin(*index),
            Some(Icon::Custom(id)) => {
                let _ = entry.set_icon_custom(*id);
            }
            None => {}
        }
        restored_entries.push(snap.id);
    }
    dest.copy_entry_attachment_versions_from(original, &restored_entries)
        .map_err(|error| format!("无法恢复合并后的回收站附件：{error}"))
}

/// User-facing description of a merge failure (same-timestamp divergence is
/// a genuine conflict the user resolves with 覆盖远程 / 下载远程).
fn describe_merge_error(error: MergeError) -> String {
    match error {
        MergeError::EntryModificationTimeNotUpdated(id) => format!(
            "无法自动合并：条目 {id} 两侧在同一时间戳下内容不同，请改用「覆盖远程」或「下载远程」"
        ),
        MergeError::GroupModificationTimeNotUpdated(id) => format!(
            "无法自动合并：分组 {id} 两侧在同一时间戳下内容不同，请改用「覆盖远程」或「下载远程」"
        ),
        MergeError::DuplicateHistoryEntries(time, id) => {
            format!("无法自动合并：条目 {id} 的历史在 {time} 存在重复时间戳")
        }
        other => format!("无法自动合并远程数据库: {other}"),
    }
}

/// Merge `remote` into a clone of `local` by entry/group UUID +
/// last-modified timestamp and return the merged database. Histories are
/// combined even when current modification times match; distinct historical
/// content sharing a timestamp is preserved. The recycle bin is excluded — the merged
/// database keeps the local bin as-is and never pulls remote bin contents in.
pub(crate) fn merge_databases(local: &Database, remote: &Database) -> Result<Database, String> {
    let mut dest = local.clone();
    let mut source = remote.clone();

    // 回收站排除: bin contents are detached from both clones so they never
    // take part in the merge. The local bin is restored afterwards; the
    // remote bin snapshot is discarded entirely.
    let bin = detach_bin(&mut dest);
    let _remote_bin = detach_bin(&mut source);
    // The remote bin group shell must not leak into the active tree (it is
    // only a shell once its contents are detached, and the local side may
    // have no bin or a different one).
    if let Some(remote_bin_id) = recycle_bin_id(&source) {
        if let Some(group) = source.group_mut(remote_bin_id) {
            group.remove();
        }
    }

    // Canonicalize binary references before comparing and capturing versions.
    dest.prepare_attachment_merge(&mut source)
        .map_err(|error| format!("无法合并附件：{error}"))?;
    let losers = collect_losing_versions(&dest, &source);
    let equal_histories = collect_equal_current_histories(&dest, &source);
    let expiration_conflicts = collect_equal_time_expiration_conflicts(&dest, &source);
    prepare_new_group_tags(&dest, &mut source);
    let log = dest.merge(&source).map_err(describe_merge_error)?;
    for id in expiration_conflicts {
        if let (Some(entry), Some(remote)) = (dest.entry(id), source.entry(id)) {
            if expiration_diverged(&entry, &remote) {
                return Err(describe_merge_error(
                    MergeError::EntryModificationTimeNotUpdated(id),
                ));
            }
        }
    }
    dest.complete_attachment_merge(&source, &log);
    // The source-winner branches omit group tags and current entry expiration.
    // Use actual outcomes; objects deleted by later events stay removed.
    for event in log.events {
        if matches!(
            event.event_type,
            MergeEventType::Created | MergeEventType::Updated
        ) {
            match event.target {
                MergeEventTarget::Group(id) => {
                    if let (Some(mut group), Some(original)) =
                        (dest.group_mut(id), remote.group(id))
                    {
                        group.tags.clone_from(&original.tags);
                    }
                }
                MergeEventTarget::Entry(id) => {
                    if let (Some(mut entry), Some(original)) =
                        (dest.entry_mut(id), source.entry(id))
                    {
                        entry.times.expiry = original.times.expiry;
                        entry.times.expires = original.times.expires;
                    }
                }
                _ => {}
            }
        }
    }

    for (id, history) in equal_histories {
        if let Some(mut entry) = dest.entry_mut(id) {
            union_history(&mut entry, history.get_entries().iter().cloned());
        }
    }
    push_losers_into_history(&mut dest, losers);
    dest.rebuild_attachment_references();
    restore_bin(&mut dest, bin, local)?;
    Ok(dest)
}
