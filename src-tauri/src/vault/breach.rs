//! Have I Been Pwned (HIBP) k-anonymity range check.
//!
//! Only the first 5 hex chars of each password's SHA-1 are sent to
//! `api.pwnedpasswords.com`; the full hash and the password itself never
//! leave the session. The check is strictly opt-in (an explicit menu action)
//! and never runs automatically.

use super::*;
use crate::crypto::{hex, sha1_bytes};
use keepass::db::GroupId;
use std::collections::HashMap;

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

/// First 5 hex chars of the password's SHA-1 (the k-anonymity prefix).
pub(crate) fn prefix_of(password: &str) -> String {
    hex(&sha1_bytes(password.as_bytes()))[..5].to_uppercase()
}

/// Parse an HIBP range response body: `SUFFIX:COUNT` per line (suffixes are
/// uppercase; keys are normalized).
fn parse_range(body: &str) -> HashMap<String, usize> {
    body.lines()
        .filter_map(|line| {
            let (suffix, count) = line.trim().split_once(':')?;
            Some((
                suffix.trim().to_uppercase(),
                count.trim().parse::<usize>().ok()?,
            ))
        })
        .collect()
}

/// Run the k-anonymity check for the given `(uuid, title, username, password)`
/// rows. Passwords are compared only by full SHA-1 locally; the network only
/// ever sees the 5-char prefix.
pub(crate) fn check_hibp(
    entries: &[(String, String, String, String)],
    client: &reqwest::blocking::Client,
    endpoint: &str,
) -> Result<Vec<BreachFinding>, String> {
    let mut by_prefix: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, (_, _, _, password)) in entries.iter().enumerate() {
        by_prefix
            .entry(prefix_of(password))
            .or_default()
            .push(index);
    }

    let mut findings = Vec::new();
    for (prefix, indices) in by_prefix {
        let url = format!("{endpoint}{prefix}");
        let body = client
            .get(&url)
            .send()
            .map_err(|e| format!("HIBP 查询失败: {e}"))?
            .error_for_status()
            .map_err(|e| format!("HIBP 返回错误: {e}"))?
            .text()
            .map_err(|e| format!("读取 HIBP 响应失败: {e}"))?;
        let suffixes = parse_range(&body);
        for index in indices {
            let (uuid, title, username, password) = &entries[index];
            let digest = hex(&sha1_bytes(password.as_bytes())).to_uppercase();
            let suffix = &digest[5..];
            if let Some(&count) = suffixes.get(suffix) {
                findings.push(BreachFinding {
                    uuid: uuid.clone(),
                    title: title.clone(),
                    username: username.clone(),
                    count,
                });
            }
        }
    }
    findings.sort_by_key(|finding| std::cmp::Reverse(finding.count));
    Ok(findings)
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    /// Read the HTTP request head (up to the blank line) so a keep-alive
    /// client request does not deadlock the mock on EOF.
    fn read_request_head(stream: &mut std::net::TcpStream) -> String {
        let mut head = String::new();
        let mut buf = [0u8; 1];
        while !head.ends_with("\r\n\r\n") {
            if stream.read(&mut buf).unwrap_or(0) == 0 {
                break;
            }
            head.push(buf[0] as char);
        }
        head
    }

    #[test]
    fn prefix_is_first_five_uppercase_sha1_hex() {
        // SHA-1("password") = 5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8
        assert_eq!(prefix_of("password"), "5BAA6");
    }

    #[test]
    fn range_parsing_normalizes_case_and_counts() {
        let map = parse_range("1E4C9B93F3F0682250B6CF8331B7EE68FD8:42\n00000:1\n");
        assert_eq!(map.get("1E4C9B93F3F0682250B6CF8331B7EE68FD8"), Some(&42));
        assert_eq!(map.get("00000"), Some(&1));
    }

    #[test]
    fn k_anonymity_check_sends_only_prefix_and_matches_locally() {
        // Mock server: assert the request path carries only the 5-char prefix,
        // then answer with the full suffix for "password".
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request_head(&mut stream);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("");
            assert_eq!(
                path, "/range/5BAA6",
                "full hash must never leave the client"
            );
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 39\r\n\r\n1E4C9B93F3F0682250B6CF8331B7EE68FD8:42\n"
            )
            .unwrap();
        });

        let client = reqwest::blocking::Client::new();
        let entries = vec![(
            "uuid-1".to_owned(),
            "GitHub".to_owned(),
            "octocat".to_owned(),
            "password".to_owned(),
        )];
        let findings = check_hibp(&entries, &client, &format!("http://{addr}/range/")).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].uuid, "uuid-1");
        assert_eq!(findings[0].title, "GitHub");
        assert_eq!(findings[0].count, 42);
        handle.join().unwrap();
    }

    #[test]
    fn unmatched_prefix_reports_no_findings() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request_head(&mut stream);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("");
            // Only the 5-char prefix may appear on the wire.
            assert!(path.starts_with("/range/") && path.len() == 12);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 0\r\n\r\n"
            )
            .unwrap();
        });

        let client = reqwest::blocking::Client::new();
        let entries = vec![(
            "uuid-2".to_owned(),
            "Safe".to_owned(),
            "u".to_owned(),
            "totally-unique-9x!".to_owned(),
        )];
        let findings = check_hibp(&entries, &client, &format!("http://{addr}/range/")).unwrap();
        assert!(findings.is_empty());
        handle.join().unwrap();
    }

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
