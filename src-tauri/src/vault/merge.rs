//! Entry-level merge of the local vault with its remote counterpart
//! (官方同步·条目级合并).
//!
//! The heavy lifting — UUID matching, last-modification comparison, history
//! union, deleted-object tracking, group/icon merge — comes from the keepass
//! crate's `Database::merge`. On top of it this module enforces SecPivot's
//! recycle-bin rule (回收站排除): bin contents never take part in the merge.
//! They are detached from both clones first, and the local bin is reattached
//! afterwards. A detached bin object that collides with an object the merge
//! kept active is resolved by timestamp: the newer change wins (deletion vs.
//! edit), and a winning deletion is recorded in `deleted_objects` so it
//! propagates on later merges.

use super::helpers::recycle_bin_id;
use chrono::NaiveDateTime;
use keepass::db::merge::MergeError;
use keepass::db::{Entry, EntryId, Group, GroupId, Icon, Times, Value};
use keepass::Database;
use std::collections::HashSet;

/// A bin entry detached for the merge, remembered with the group it hung
/// under inside the bin and its attachment payloads (`EntryMut::remove`
/// drops last-referenced attachments from the database).
struct DetachedEntry {
    id: EntryId,
    parent: GroupId,
    entry: Entry,
    attachments: Vec<(String, Value<Vec<u8>>)>,
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

/// Content comparison ignoring timestamps and history (mirrors the keepass
/// merge's divergence check).
fn content_diverged(a: &Entry, b: &Entry) -> bool {
    let mut a = a.clone();
    a.times = Times::default();
    a.history = None;
    let mut b = b.clone();
    b.times = Times::default();
    b.history = None;
    a != b
}

/// Capture the remote side's current state for entries where the LOCAL side
/// is newer: the keepass merge only pushes the losing side into the history
/// when the remote wins, so local winners need their remote loser preserved
/// here (历史保留).
fn collect_remote_losers(dest: &Database, source: &Database) -> Vec<(EntryId, Entry)> {
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
                    if local_time > remote_time && content_diverged(&entry, &remote) {
                        let mut loser = (*remote).clone();
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

/// Push captured losers into the merged entries' histories, skipping ones
/// already identical to the newest history entry.
fn push_losers_into_history(dest: &mut Database, losers: Vec<(EntryId, Entry)>) {
    for (id, loser) in losers {
        let dominated = dest
            .entry(id)
            .map(|entry| {
                entry
                    .historical(0)
                    .map(|newest| !content_diverged(&newest, &loser))
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        if dominated {
            continue;
        }
        if let Some(mut entry) = dest.entry_mut(id) {
            let mut history = entry.history.take().unwrap_or_default();
            history.add_entry(loser);
            entry.history = Some(history);
        }
    }
}

/// Snapshot a bin entry (content + attachments) before detaching it.
fn snapshot_entry(db: &Database, id: EntryId, parent: GroupId) -> Option<DetachedEntry> {
    let entry = db.entry(id)?;
    let attachments = entry
        .attachments_named()
        .map(|(name, attachment)| (name.to_owned(), attachment.data.clone()))
        .collect();
    Some(DetachedEntry {
        id,
        parent,
        entry: (*entry).clone(),
        attachments,
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
/// the newer timestamp wins: a newer local deletion removes the active
/// object from the merged tree (recorded in `deleted_objects` so it
/// propagates); a newer remote edit drops the bin copy (and, for groups, its
/// whole snapshot subtree).
fn restore_bin(dest: &mut Database, bin: DetachedBin) {
    let mut dropped_groups: HashSet<GroupId> = HashSet::new();
    for snap in &bin.groups {
        let Some(active_time) = dest.group(snap.id).map(|g| active_change(&g.times)) else {
            continue; // not active in the merged tree — nothing to resolve
        };
        let bin_time = bin_change(&snap.group.times);
        if bin_time >= active_time {
            if let Some(mut group) = dest.group_mut(snap.id) {
                // GroupTrack::remove records deleted_objects (deletion wins).
                let _ = group.track_changes().remove();
            }
        } else {
            dropped_groups.insert(snap.id);
        }
    }

    let mut dropped_entries: HashSet<EntryId> = HashSet::new();
    for snap in &bin.entries {
        let Some(active) = dest.entry(snap.id) else {
            continue;
        };
        let bin_time = bin_change(&snap.entry.times);
        let active_time = active_change(&active.times);
        if bin_time >= active_time {
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
        for (name, data) in &snap.attachments {
            entry.add_attachment(name.clone(), data.clone());
        }
    }
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
/// preserved (the keepass merge unions both sides' histories and pushes the
/// losing side's current state); the recycle bin is excluded — the merged
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

    // The keepass merge only preserves the losing side's current state when
    // the remote wins; capture local winners' remote losers up front so they
    // can be pushed into the merged history afterwards.
    let losers = collect_remote_losers(&dest, &source);
    dest.merge(&source).map_err(describe_merge_error)?;

    push_losers_into_history(&mut dest, losers);
    restore_bin(&mut dest, bin);
    Ok(dest)
}
