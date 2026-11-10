pub(crate) mod attachment;
pub(crate) mod autotype;
pub(crate) mod color;
pub(crate) mod custom_data;
pub(crate) mod entry;
pub(crate) mod group;
pub(crate) mod history;
pub(crate) mod icon;
pub(crate) mod meta;
pub(crate) mod times;
pub(crate) mod value;

use std::collections::HashMap;

pub use attachment::{Attachment, AttachmentId, AttachmentMut, AttachmentRef};
pub use autotype::{AutoType, AutoTypeAssociation, DataTransferObfuscation};
pub use color::{Color, ParseColorError};
pub use custom_data::{CustomDataItem, CustomDataValue};
pub use entry::{DestinationGroupNotFoundError, Entry, EntryId, EntryMut, EntryRef, EntryTrack};
pub use group::{
    DuplicateEntryIdError, DuplicateGroupIdError, Group, GroupId, GroupMut, GroupRef, GroupTrack,
    MoveGroupError,
};
pub use history::History;
pub use icon::{
    CustomIcon, CustomIconId, CustomIconMut, CustomIconNotFoundError, CustomIconRef, Icon,
};
pub use meta::{MemoryProtection, Meta};
pub use times::Times;
pub use value::Value;

use crate::config::DatabaseConfig;

use chrono::NaiveDateTime;
use uuid::Uuid;

/// A decrypted KeePass database
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct Database {
    /// Configuration settings of the database such as encryption and compression algorithms
    pub config: DatabaseConfig,

    /// Metadata of the KeePass database
    pub meta: Meta,

    /// Root node of the KeePass database
    pub(crate) root: GroupId,

    /// All attachments in the database, stored in a flat HashMap
    pub(crate) attachments: HashMap<AttachmentId, Attachment>,

    /// Attachment leases held by not-yet-appended EntryTrack snapshots.
    #[cfg_attr(feature = "serialization", serde(skip))]
    pub(crate) pending_history_attachments: HashMap<AttachmentId, usize>,

    /// All custom icons in the database, stored in a flat HashMap
    pub(crate) custom_icons: HashMap<CustomIconId, CustomIcon>,

    /// All entries in the database, stored in a flat HashMap
    pub(crate) entries: HashMap<EntryId, Entry>,

    /// All groups in the database, stored in a flat HashMap
    pub(crate) groups: HashMap<GroupId, Group>,

    /// References to previously-deleted objects and their deletion times.
    pub deleted_objects: HashMap<Uuid, Option<NaiveDateTime>>,
}

/// Fail closed when a merge cannot map every current/historical binary.
#[cfg(feature = "_merge")]
#[derive(Debug, thiserror::Error)]
pub enum AttachmentMergeError {
    /// At least one binary reference has no backing payload.
    #[error("A current or historical attachment payload is missing")]
    MissingPayload,
    /// A requested source or destination entry does not exist.
    #[error("The entry to receive attachment versions is missing")]
    MissingEntry,
    /// An existing history or merge operation still holds binary leases.
    #[error("Attachment history or merge leases are still active")]
    PendingHistory,
    /// A version tree changed before its attachment maps were restored.
    #[error("The cloned attachment version layout does not match its source")]
    VersionMismatch,
}

#[cfg(feature = "_merge")]
type AttachmentFingerprint = (bool, [u8; 32]);
#[cfg(feature = "_merge")]
type AttachmentIndex = HashMap<AttachmentFingerprint, Vec<AttachmentId>>;

#[cfg(feature = "_merge")]
fn attachment_fingerprint(data: &Value<Vec<u8>>) -> AttachmentFingerprint {
    use sha2::{Digest, Sha256};
    (data.is_protected(), Sha256::digest(data.get()).into())
}

#[cfg(feature = "_merge")]
fn collect_entry_attachment_ids(entry: &Entry, ids: &mut std::collections::HashSet<AttachmentId>) {
    ids.extend(entry.attachments.values().copied());
    if let Some(history) = &entry.history {
        for historical in &history.entries {
            collect_entry_attachment_ids(historical, ids);
        }
    }
}

#[cfg(feature = "_merge")]
fn remap_entry_attachment_ids(entry: &mut Entry, map: &HashMap<AttachmentId, AttachmentId>) {
    for id in entry.attachments.values_mut() {
        *id = map[id];
    }
    if let Some(history) = &mut entry.history {
        for historical in &mut history.entries {
            remap_entry_attachment_ids(historical, map);
        }
    }
}

#[cfg(feature = "_merge")]
fn same_attachment_version_layout(a: &Entry, b: &Entry) -> bool {
    match (&a.history, &b.history) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.entries.len() == b.entries.len()
                && a.entries
                    .iter()
                    .zip(&b.entries)
                    .all(|(a, b)| same_attachment_version_layout(a, b))
        }
        _ => false,
    }
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            meta: self.meta.clone(),
            root: self.root,
            attachments: self.attachments.clone(),
            // Live guards borrow their original database; clones own no leases.
            pending_history_attachments: HashMap::new(),
            custom_icons: self.custom_icons.clone(),
            entries: self.entries.clone(),
            groups: self.groups.clone(),
            deleted_objects: self.deleted_objects.clone(),
        }
    }
}

impl Database {
    /// Create a new database with a single root group and no entries, groups, or attachments.
    ///
    /// The root group will be assigned a new random UUID.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self::new_with_root_id(GroupId::new())
    }

    /// Create a new database with the given configuration and a single root group.
    ///
    /// The root group will be assigned a new random UUID.
    pub fn with_config(config: DatabaseConfig) -> Self {
        Self::with_data(config, GroupId::new())
    }

    /// Create a new database with the given group UUID
    pub fn new_with_root_id(root_id: GroupId) -> Self {
        let root = Group::with_id(root_id, None);

        let mut groups = HashMap::new();
        groups.insert(root_id, root);

        Database {
            config: DatabaseConfig::default(),
            meta: Meta::default(),
            root: root_id,
            attachments: HashMap::new(),
            pending_history_attachments: HashMap::new(),
            custom_icons: HashMap::new(),
            entries: HashMap::new(),
            groups,
            deleted_objects: HashMap::new(),
        }
    }

    pub(crate) fn with_data(config: DatabaseConfig, root_id: GroupId) -> Self {
        let root = Group::with_id(root_id, None);

        let mut groups = HashMap::new();
        groups.insert(root_id, root);

        Database {
            config,
            meta: Meta::default(),
            root: root_id,
            attachments: HashMap::new(),
            pending_history_attachments: HashMap::new(),
            custom_icons: HashMap::new(),
            entries: HashMap::new(),
            groups,
            deleted_objects: HashMap::new(),
        }
    }

    /// Get an immutable reference to the root group of the database.
    pub fn root(&self) -> GroupRef<'_> {
        GroupRef::new(self, self.root)
    }

    /// Get a mutable reference to the root group of the database.
    pub fn root_mut(&mut self) -> GroupMut<'_> {
        GroupMut::new(self, self.root)
    }

    /// Get an immutable reference to the recycle bin group, if it exists
    pub fn recycle_bin(&self) -> Option<GroupRef<'_>> {
        let recyclebin_id = self.meta.recyclebin_uuid.map(GroupId::from_uuid)?;
        self.group(recyclebin_id)
    }

    /// Get a mutable reference to the recycle bin group, if it exists
    pub fn recycle_bin_mut(&mut self) -> Option<GroupMut<'_>> {
        let recyclebin_id = self.meta.recyclebin_uuid.map(GroupId::from_uuid)?;
        self.group_mut(recyclebin_id)
    }

    /// Get the number of attachments in the database
    pub fn num_attachments(&self) -> usize {
        self.attachments.len()
    }

    /// Rebuild one entry's current/historical reverse references and collect
    /// only attachments with no entry or in-flight history owner.
    pub(crate) fn reindex_entry_attachments(&mut self, id: EntryId) {
        let mut desired: HashMap<AttachmentId, Vec<Option<usize>>> = HashMap::new();
        if let Some(entry) = self.entries.get(&id) {
            for attachment_id in entry.attachments.values() {
                desired.entry(*attachment_id).or_default().push(None);
            }
            if let Some(history) = &entry.history {
                for (index, historical) in history.entries.iter().enumerate() {
                    for attachment_id in historical.attachments.values() {
                        desired.entry(*attachment_id).or_default().push(Some(index));
                    }
                }
            }
        }
        let mut affected: std::collections::HashSet<_> = desired.keys().copied().collect();
        for (attachment_id, attachment) in &mut self.attachments {
            if attachment
                .entries
                .iter()
                .any(|&(entry_id, _)| entry_id == id)
            {
                affected.insert(*attachment_id);
            }
            attachment.entries.retain(|&(entry_id, _)| entry_id != id);
            if let Some(indices) = desired.get(attachment_id) {
                for index in indices {
                    attachment.entries.insert((id, *index));
                }
            }
        }
        let pending = &self.pending_history_attachments;
        self.attachments.retain(|attachment_id, attachment| {
            !affected.contains(attachment_id)
                || !attachment.entries.is_empty()
                || pending.contains_key(attachment_id)
        });
    }

    /// Rebuild all reverse references after loading or bulk history changes.
    pub fn reindex_attachment_history(&mut self) {
        self.rebuild_attachment_references();
        let pending = &self.pending_history_attachments;
        self.attachments.retain(|attachment_id, attachment| {
            !attachment.entries.is_empty() || pending.contains_key(attachment_id)
        });
    }

    /// Load reconstructs ownership without silently deleting orphan header data.
    pub fn rebuild_attachment_references(&mut self) {
        for attachment in self.attachments.values_mut() {
            attachment.entries.clear();
        }
        for (id, entry) in &self.entries {
            for attachment_id in entry.attachments.values() {
                if let Some(attachment) = self.attachments.get_mut(attachment_id) {
                    attachment.entries.insert((*id, None));
                }
            }
            if let Some(history) = &entry.history {
                for (index, historical) in history.entries.iter().enumerate() {
                    for attachment_id in historical.attachments.values() {
                        if let Some(attachment) = self.attachments.get_mut(attachment_id) {
                            attachment.entries.insert((*id, Some(index)));
                        }
                    }
                }
            }
        }
    }

    /// Prepare two temporary merge clones with one content-based binary namespace.
    /// Protection flags are part of identity; hashes index candidates but exact
    /// byte comparison decides equality. Existing orphan header bytes remain.
    #[cfg(feature = "_merge")]
    pub fn prepare_attachment_merge(
        &mut self,
        source: &mut Self,
    ) -> Result<(), AttachmentMergeError> {
        if !self.pending_history_attachments.is_empty()
            || !source.pending_history_attachments.is_empty()
        {
            return Err(AttachmentMergeError::PendingHistory);
        }
        let mut source_ids = std::collections::HashSet::new();
        for entry in source.entries.values() {
            collect_entry_attachment_ids(entry, &mut source_ids);
        }
        let mut dest_ids = std::collections::HashSet::new();
        for entry in self.entries.values() {
            collect_entry_attachment_ids(entry, &mut dest_ids);
        }
        if source_ids
            .iter()
            .any(|id| !source.attachments.contains_key(id))
            || dest_ids.iter().any(|id| !self.attachments.contains_key(id))
        {
            return Err(AttachmentMergeError::MissingPayload);
        }
        let (mut index, dest_map) = self.canonical_attachment_index();
        for entry in self.entries.values_mut() {
            remap_entry_attachment_ids(entry, &dest_map);
        }
        let source_map = self.import_attachment_ids(source, source_ids, &mut index);
        for entry in source.entries.values_mut() {
            remap_entry_attachment_ids(entry, &source_map);
        }
        // Move existing source values into their remapped slots rather than
        // cloning the entire destination binary pool into a third copy.
        let mut remapped = HashMap::new();
        for (id, mut attachment) in std::mem::take(&mut source.attachments) {
            if let Some(mapped) = source_map.get(&id) {
                attachment.id = *mapped;
                attachment.entries.clear();
                remapped.entry(*mapped).or_insert(attachment);
            }
        }
        source.attachments = remapped;
        source.rebuild_attachment_references();
        // Entry creation and deletion precede merge-log completion. Keep every
        // prepared binary alive while its new reverse owners are still pending.
        self.pending_history_attachments
            .extend(self.attachments.keys().copied().map(|id| (id, 1)));
        self.rebuild_attachment_references();
        Ok(())
    }

    /// Apply current binary references only for actual source content winners.
    /// Histories already carry the prepared namespace; no timestamp decisions
    /// are repeated here and later-deleted entries are not resurrected.
    #[cfg(feature = "_merge")]
    pub fn complete_attachment_merge(&mut self, source: &Self, log: &crate::db::merge::MergeLog) {
        use crate::db::merge::{MergeEventTarget, MergeEventType};
        for event in &log.events {
            if matches!(
                event.event_type,
                MergeEventType::Created | MergeEventType::Updated
            ) {
                if let MergeEventTarget::Entry(id) = event.target {
                    if let (Some(dest), Some(source)) =
                        (self.entries.get_mut(&id), source.entries.get(&id))
                    {
                        dest.attachments.clone_from(&source.attachments);
                    }
                }
            }
        }
        self.rebuild_attachment_references();
        self.pending_history_attachments.clear();
    }

    /// Restore binary references for complete entry/version trees cloned from
    /// source (e.g. a detached local recycle bin). The layout must still match.
    /// Import every referenced version before installing any of its maps.
    #[cfg(feature = "_merge")]
    pub fn copy_entry_attachment_versions_from(
        &mut self,
        source: &Self,
        ids: &[EntryId],
    ) -> Result<(), AttachmentMergeError> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut payload_ids = std::collections::HashSet::new();
        for id in ids {
            let dest = self
                .entries
                .get(id)
                .ok_or(AttachmentMergeError::MissingEntry)?;
            let original = source
                .entries
                .get(id)
                .ok_or(AttachmentMergeError::MissingEntry)?;
            if !same_attachment_version_layout(dest, original) {
                return Err(AttachmentMergeError::VersionMismatch);
            }
            collect_entry_attachment_ids(original, &mut payload_ids);
        }
        if payload_ids
            .iter()
            .any(|id| !source.attachments.contains_key(id))
        {
            return Err(AttachmentMergeError::MissingPayload);
        }
        let (mut index, _) = self.canonical_attachment_index();
        let map = self.import_attachment_ids(source, payload_ids, &mut index);
        for id in ids {
            // Validation above established both entry trees and matching layout.
            let dest = self
                .entries
                .get_mut(id)
                .ok_or(AttachmentMergeError::MissingEntry)?;
            let original = source
                .entries
                .get(id)
                .ok_or(AttachmentMergeError::MissingEntry)?;
            dest.attachments.clone_from(&original.attachments);
            if let Some(history) = &mut dest.history {
                fn copy_maps(dest: &mut Entry, source: &Entry) {
                    dest.attachments.clone_from(&source.attachments);
                    if let (Some(dest), Some(source)) = (&mut dest.history, &source.history) {
                        for (dest, source) in dest.entries.iter_mut().zip(&source.entries) {
                            copy_maps(dest, source);
                        }
                    }
                }
                if let Some(original) = &original.history {
                    for (dest, source) in history.entries.iter_mut().zip(&original.entries) {
                        copy_maps(dest, source);
                    }
                }
            }
            remap_entry_attachment_ids(dest, &map);
        }
        self.rebuild_attachment_references();
        Ok(())
    }

    #[cfg(feature = "_merge")]
    fn canonical_attachment_index(&self) -> (AttachmentIndex, HashMap<AttachmentId, AttachmentId>) {
        let mut ids: Vec<_> = self.attachments.keys().copied().collect();
        ids.sort_by_key(|id| id.id());
        let mut index: AttachmentIndex = HashMap::new();
        let mut map = HashMap::new();
        for id in ids {
            let data = &self.attachments[&id].data;
            let candidates = index.entry(attachment_fingerprint(data)).or_default();
            let canonical = candidates
                .iter()
                .find(|candidate| self.attachments[candidate].data == *data)
                .copied();
            let canonical = canonical.unwrap_or_else(|| {
                candidates.push(id);
                id
            });
            map.insert(id, canonical);
        }
        (index, map)
    }

    #[cfg(feature = "_merge")]
    fn import_attachment_ids(
        &mut self,
        source: &Self,
        ids: std::collections::HashSet<AttachmentId>,
        index: &mut AttachmentIndex,
    ) -> HashMap<AttachmentId, AttachmentId> {
        let mut ids: Vec<_> = ids.into_iter().collect();
        ids.sort_by_key(|id| id.id());
        let mut map = HashMap::new();
        let mut next_id = AttachmentId::next_free(self).id();
        for id in ids {
            let data = &source.attachments[&id].data;
            let candidates = index.entry(attachment_fingerprint(data)).or_default();
            let existing = candidates
                .iter()
                .find(|candidate| self.attachments[candidate].data == *data)
                .copied();
            let dest_id = existing.unwrap_or_else(|| {
                while self.attachments.contains_key(&AttachmentId::new(next_id)) {
                    next_id += 1;
                }
                let dest_id = AttachmentId::new(next_id);
                next_id += 1;
                self.attachments.insert(
                    dest_id,
                    Attachment {
                        id: dest_id,
                        entries: Default::default(),
                        data: data.clone(),
                    },
                );
                candidates.push(dest_id);
                dest_id
            });
            map.insert(id, dest_id);
        }
        map
    }

    /// Get the number of custom icons in the database
    pub fn num_custom_icons(&self) -> usize {
        self.custom_icons.len()
    }

    /// Get the number of entries in the database
    pub fn num_entries(&self) -> usize {
        self.entries.len()
    }

    /// Get the number of groups in the database, including the root group and the recycle bin (if it exists)
    pub fn num_groups(&self) -> usize {
        self.groups.len()
    }

    /// Iterate over all attachments with immutable access.
    pub fn iter_all_attachments(&self) -> impl Iterator<Item = AttachmentRef<'_>> + '_ {
        self.attachments
            .keys()
            .map(move |id| AttachmentRef::new(self, *id))
    }

    /// Iterate over all attachments with mutable access. The provided closure is
    /// called for each `AttachmentMut` and borrows are limited to the closure body.
    pub fn foreach_attachment_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(AttachmentMut<'_>),
    {
        let ids: Vec<AttachmentId> = self.attachments.keys().copied().collect();
        for id in ids {
            f(AttachmentMut::new(self, id));
        }
    }

    /// Iterate over all entries with immutable access.
    pub fn iter_all_entries(&self) -> impl Iterator<Item = EntryRef<'_>> + '_ {
        self.entries.keys().map(move |id| EntryRef::new(self, *id))
    }

    /// Iterate over all entries with mutable access. The provided closure is
    /// called for each `EntryMut` and borrows are limited to the closure body.
    pub fn foreach_entry_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(EntryMut<'_>),
    {
        let ids: Vec<EntryId> = self.entries.keys().copied().collect();
        for id in ids {
            f(EntryMut::new(self, id));
        }
    }

    /// Iterate over all custom icons with immutable access.
    pub fn iter_all_custom_icons(&self) -> impl Iterator<Item = CustomIconRef<'_>> + '_ {
        self.custom_icons
            .keys()
            .map(move |id| CustomIconRef::new(self, *id))
    }

    /// Iterate over all custom icons with mutable access. The provided closure is
    /// called for each `CustomIconMut` and borrows are limited to the closure body.
    pub fn foreach_custom_icon_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(CustomIconMut<'_>),
    {
        let ids: Vec<CustomIconId> = self.custom_icons.keys().copied().collect();
        for id in ids {
            f(CustomIconMut::new(self, id));
        }
    }

    /// Iterate over all groups with immutable access. This includes the root group and the recycle
    /// bin (if it exists).
    pub fn iter_all_groups(&self) -> impl Iterator<Item = GroupRef<'_>> + '_ {
        self.groups.keys().map(move |id| GroupRef::new(self, *id))
    }

    /// Iterate over all groups with mutable access. The provided closure is
    /// called for each `GroupMut` and borrows are limited to the closure body.
    pub fn foreach_group_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(GroupMut<'_>),
    {
        let ids: Vec<GroupId> = self.groups.keys().copied().collect();
        for id in ids {
            f(GroupMut::new(self, id));
        }
    }

    /// Get an immutable reference to the attachment with the given ID, if it exists
    pub fn attachment(&self, id: AttachmentId) -> Option<AttachmentRef<'_>> {
        self.attachments
            .contains_key(&id)
            .then(move || AttachmentRef::new(self, id))
    }

    /// Get a mutable reference to the attachment with the given ID, if it exists
    pub fn attachment_mut(&mut self, id: AttachmentId) -> Option<AttachmentMut<'_>> {
        self.attachments
            .contains_key(&id)
            .then(move || AttachmentMut::new(self, id))
    }

    /// Get an immutable reference to the custom icon with the given ID, if it exists
    pub fn custom_icon(&self, id: CustomIconId) -> Option<CustomIconRef<'_>> {
        self.custom_icons
            .contains_key(&id)
            .then(move || CustomIconRef::new(self, id))
    }

    /// Get a mutable reference to the custom icon with the given ID, if it exists
    pub fn custom_icon_mut(&mut self, id: CustomIconId) -> Option<CustomIconMut<'_>> {
        self.custom_icons
            .contains_key(&id)
            .then(move || CustomIconMut::new(self, id))
    }

    /// Get an immutable reference to the entry with the given ID, if it exists
    pub fn entry(&self, id: EntryId) -> Option<EntryRef<'_>> {
        self.entries
            .contains_key(&id)
            .then(move || EntryRef::new(self, id))
    }

    /// Get a mutable reference to the entry with the given ID, if it exists
    pub fn entry_mut(&mut self, id: EntryId) -> Option<EntryMut<'_>> {
        self.entries
            .contains_key(&id)
            .then(move || EntryMut::new(self, id))
    }

    /// Get an immutable reference to the group with the given ID, if it exists
    pub fn group(&self, id: GroupId) -> Option<GroupRef<'_>> {
        self.groups
            .contains_key(&id)
            .then(move || GroupRef::new(self, id))
    }

    /// Get a mutable reference to the group with the given ID, if it exists
    pub fn group_mut(&mut self, id: GroupId) -> Option<GroupMut<'_>> {
        self.groups
            .contains_key(&id)
            .then(move || GroupMut::new(self, id))
    }
}

#[cfg(all(test, feature = "_merge"))]
mod secpivot_merge_attachment_tests {
    use super::*;
    #[test]
    fn secpivot_merge_rejects_missing_payload_without_mutating_clones() {
        let mut dest = Database::new();
        let mut source = dest.clone();
        let id = source.root_mut().add_entry().id();
        source
            .entries
            .get_mut(&id)
            .unwrap()
            .attachments
            .insert("synthetic.txt".into(), AttachmentId::new(999));
        let old_dest = dest.clone();
        let old_source = source.clone();
        assert!(matches!(
            dest.prepare_attachment_merge(&mut source),
            Err(AttachmentMergeError::MissingPayload)
        ));
        assert_eq!(dest, old_dest);
        assert_eq!(source, old_source);
    }
    #[test]
    fn secpivot_merge_same_bytes_deduplicate_only_with_equal_protection() {
        let mut dest = Database::new();
        let a = dest.root_mut().add_entry().id();
        dest.entry_mut(a)
            .unwrap()
            .add_attachment("same.txt", Value::protected(b"synthetic".to_vec()));
        let mut source = Database::new_with_root_id(dest.root);
        let b = source.root_mut().add_entry().id();
        source
            .entry_mut(b)
            .unwrap()
            .add_attachment("same.txt", Value::unprotected(b"synthetic".to_vec()));
        let c = source.root_mut().add_entry().id();
        source
            .entry_mut(c)
            .unwrap()
            .add_attachment("same.txt", Value::protected(b"synthetic".to_vec()));
        dest.prepare_attachment_merge(&mut source).unwrap();
        assert_eq!(dest.num_attachments(), 2);
        let original = dest
            .entry(a)
            .unwrap()
            .attachment_by_name("same.txt")
            .unwrap()
            .id();
        assert_ne!(
            source
                .entry(b)
                .unwrap()
                .attachment_by_name("same.txt")
                .unwrap()
                .id(),
            original
        );
        assert_eq!(
            source
                .entry(c)
                .unwrap()
                .attachment_by_name("same.txt")
                .unwrap()
                .id(),
            original
        );
    }
}

#[cfg(all(test, feature = "_merge"))]
mod secpivot_merge_lifetime_tests {
    use super::*;
    #[test]
    fn secpivot_merge_shared_created_owner_survives_local_deletion() {
        let mut dest = Database::new();
        let old = {
            let mut root = dest.root_mut();
            let mut entry = root.add_entry();
            entry.add_attachment("same.txt", Value::protected(b"synthetic-shared".to_vec()));
            entry.times.last_modification = Some(Times::epoch());
            entry.id()
        };
        let mut source = dest.clone();
        source.entry_mut(old).unwrap().track_changes().remove();
        let new = source.root_mut().add_entry().id();
        source
            .entry_mut(new)
            .unwrap()
            .add_attachment("same.txt", Value::protected(b"synthetic-shared".to_vec()));
        dest.prepare_attachment_merge(&mut source).unwrap();
        let log = dest.merge(&source).unwrap();
        dest.complete_attachment_merge(&source, &log);
        assert!(dest.entry(old).is_none());
        assert!(
            dest.entry(new)
                .unwrap()
                .attachment_by_name("same.txt")
                .unwrap()
                .get()
                .as_slice()
                == b"synthetic-shared"
        );
        assert!(dest.pending_history_attachments.is_empty());
    }
    #[test]
    fn secpivot_remove_deduplicated_attachment_names_without_panic() {
        let mut dest = Database::new();
        let id = dest.root_mut().add_entry().id();
        for name in ["first.txt", "second.txt"] {
            dest.entry_mut(id)
                .unwrap()
                .add_attachment(name, Value::protected(b"synthetic-identical".to_vec()));
        }
        let mut source = dest.clone();
        dest.prepare_attachment_merge(&mut source).unwrap();
        let log = dest.merge(&source).unwrap();
        dest.complete_attachment_merge(&source, &log);
        let entry = dest.entry(id).unwrap();
        assert_eq!(
            entry.attachment_by_name("first.txt").unwrap().id(),
            entry.attachment_by_name("second.txt").unwrap().id()
        );
        dest.entry_mut(id).unwrap().remove();
        assert_eq!(dest.num_entries(), 0);
        dest.reindex_attachment_history();
        assert_eq!(dest.num_attachments(), 0);
    }
    #[test]
    fn secpivot_merge_refuses_existing_leases_without_changing_inputs() {
        let mut dest = Database::new();
        dest.pending_history_attachments
            .insert(AttachmentId::new(7), 1);
        let mut source = Database::new();
        let old_dest = dest.clone();
        let old_source = source.clone();
        assert!(matches!(
            dest.prepare_attachment_merge(&mut source),
            Err(AttachmentMergeError::PendingHistory)
        ));
        assert_eq!(dest.entries, old_dest.entries);
        assert_eq!(source, old_source);
        assert_eq!(
            dest.pending_history_attachments.get(&AttachmentId::new(7)),
            Some(&1)
        );
    }
}
