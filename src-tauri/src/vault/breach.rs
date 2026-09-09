//! Have I Been Pwned (HIBP) k-anonymity range check.
//!
//! Only the first 5 hex chars of each password's SHA-1 are sent to
//! `api.pwnedpasswords.com`; the full hash and the password itself never
//! leave the session. The check is strictly opt-in (an explicit menu action)
//! and never runs automatically.
//!
//! The live implementation is the async `commands::entries::check_hibp_entries`
//! (the `check_hibp` command is a thin wrapper around it); this module keeps
//! the shared types and the session row collector.

use super::*;
use keepass::db::GroupId;

/// Production endpoint; tests inject a local mock instead.
pub const HIBP_RANGE_URL: &str = "https://api.pwnedpasswords.com/range/";

/// One password that appears in known breach data, reported to the user.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BreachFinding {
    pub uuid: String,
    pub title: String,
    pub username: String,
    /// How many times the password appeared in breach data.
    pub count: usize,
}

impl VaultSession {
    /// Collect `(uuid, title, username, password)` rows for the HIBP check:
    /// the given uuids, or every entry except the recycle bin. Passwords are
    /// consumed inside the session and never returned by the command itself.
    pub(crate) fn hibp_entries(
        &self,
        uuids: Option<&[String]>,
    ) -> Result<Vec<(String, String, String, String)>, String> {
        let db = self.require_db()?;
        let bin_id = recycle_bin_id(db);
        let filter = uuids.map(|ids| {
            ids.iter()
                .cloned()
                .collect::<std::collections::HashSet<_>>()
        });
        let mut rows = Vec::new();

        fn walk(
            group: &keepass::db::GroupRef<'_>,
            bin_id: Option<GroupId>,
            filter: &Option<std::collections::HashSet<String>>,
            rows: &mut Vec<(String, String, String, String)>,
        ) {
            if Some(group.id()) == bin_id {
                return;
            }
            for entry in group.entries() {
                let uuid = entry.id().uuid().to_string();
                if let Some(filter) = filter {
                    if !filter.contains(&uuid) {
                        continue;
                    }
                }
                rows.push((
                    uuid,
                    entry.get_title().unwrap_or_default().to_owned(),
                    entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
                    entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned(),
                ));
            }
            for child in group.groups() {
                walk(&child, bin_id, filter, rows);
            }
        }
        walk(&db.root(), bin_id, &filter, &mut rows);
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hibp_entries_filter_uuids_and_skip_recycle_bin() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("v.kdbx");
        let mut session = VaultSession::default();
        session
            .create(&path, "master", "Aes", "Aes256", "None", None)
            .unwrap();
        let state = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "GitHub".into(),
                username: "octocat".into(),
                password: "password".into(),
                url: String::new(),
                notes: String::new(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let uuid = state.root.entries[0].uuid.clone();
        let rows = session.hibp_entries(None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, "GitHub");
        assert_eq!(session.hibp_entries(Some(&[uuid])).unwrap().len(), 1);
        assert!(session
            .hibp_entries(Some(&["nope".to_owned()]))
            .unwrap()
            .is_empty());
    }
}
