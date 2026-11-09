//! Small shared helpers that don't belong to a domain module: URL host
//! extraction and atomic file writes.

use std::path::{Path, PathBuf};

/// Canonical destination host, excluding userinfo, scheme, port and path.
/// IDNA domains and IP literals use the URL parser's normalized identity;
/// IPv6 is returned without brackets. Bare hosts/ports and protocol-relative
/// URLs remain supported. Invalid authorities and internal whitespace fail
/// closed rather than guessing a credential destination.
pub fn url_host(url: &str) -> Option<String> {
    url_authority(url).map(|authority| authority.host)
}

/// Canonical host and the explicitly written numeric port. An omitted port
/// remains None even for HTTP(S): strict KPRPC rules must not invent a port
/// from a scheme, or erase an explicitly specified default port.
#[derive(PartialEq, Eq)]
pub(crate) struct UrlAuthority {
    pub host: String,
    pub port: Option<u16>,
}

pub(crate) fn url_authority(url: &str) -> Option<UrlAuthority> {
    let input = url.trim();
    if input.is_empty() || input.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let scheme_end = input.find("://").filter(|&end| {
        let scheme = &input[..end];
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    });
    let (parsed, rest, bare) = if let Some(end) = scheme_end {
        (url::Url::parse(input).ok()?, &input[end + 3..], false)
    } else if let Some(relative) = input.strip_prefix("//") {
        (
            url::Url::parse(&format!("https:{input}")).ok()?,
            relative,
            false,
        )
    } else {
        (
            url::Url::parse(&format!("http://{input}")).ok()?,
            input,
            true,
        )
    };
    let host = match parsed.host()? {
        url::Host::Domain(domain) => domain.to_ascii_lowercase(),
        url::Host::Ipv4(address) => address.to_string(),
        url::Host::Ipv6(address) => address.to_string(),
    };
    // URL parsing validates the authority and canonicalizes the host, but
    // removes :443/:80/etc. Read only the original explicit port afterwards.
    let special = matches!(
        parsed.scheme(),
        "http" | "https" | "ftp" | "ws" | "wss" | "file"
    );
    let rest = if special {
        rest.trim_start_matches(['/', '\\'])
    } else {
        rest
    };
    let authority = rest
        .split(|c| matches!(c, '/' | '?' | '#') || (special && c == '\\'))
        .next()?;
    if bare && authority.contains('@') {
        return None;
    }
    let authority = authority.rsplit('@').next()?;
    let port_text = if authority.starts_with('[') {
        authority[authority.find(']')? + 1..].strip_prefix(':')
    } else {
        authority.rsplit_once(':').map(|(_, port)| port)
    };
    if bare && port_text == Some("") {
        return None;
    }
    let port = match port_text.filter(|port| !port.is_empty()) {
        Some(port) => Some(port.parse().ok()?),
        None => None,
    };
    Some(UrlAuthority { host, port })
}

/// Write `bytes` to `path` atomically: write to a unique sibling `.tmp` file, flush
/// it, then rename it over the target so readers never observe a half-written
/// file. On any failure the temp file is removed. `what` names the file type
/// in the user-facing error messages (e.g. `"配置"`, `"数据库"`).
pub fn atomic_write(path: &Path, bytes: &[u8], what: &str) -> Result<(), String> {
    let tmp = temp_sibling(path).map_err(|e| format!("写入{what}失败: {e}"))?;
    // The temp file holds the new content in clear (it is an encrypted vault,
    // but the same helper writes other files too), so it is created with
    // owner-only permissions instead of the process umask. `create_new` also
    // refuses to follow a pre-planted symlink. Each write owns a fresh random
    // sibling, so concurrent writes cannot remove or truncate one another's temp.
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // Only a successfully created file belongs to this operation. A collision
    // fails without deleting an existing file, including legacy fixed siblings.
    let mut file = options
        .open(&tmp)
        .map_err(|e| format!("写入{what}失败: {e}"))?;
    let write = (|| -> std::io::Result<()> {
        use std::io::Write;
        file.write_all(bytes)?;
        // The mode is only applied at creation; set it explicitly as well so
        // a recreated temp file can never keep a wider mode.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        // Flush the exact handle that received the bytes; reopening a path can
        // accidentally sync another writer's file. Windows requires write access.
        file.sync_all()
    })();
    drop(file);
    if let Err(e) = write {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("写入{what}失败: {e}"));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("保存{what}失败: {e}"));
    }
    // Best-effort: persist the rename itself (directory metadata).
    if let Some(parent) = path.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

/// A random sibling per write; `create_new` remains the final ownership check.
fn temp_sibling(path: &Path) -> Result<PathBuf, String> {
    let mut nonce = [0u8; 16];
    getrandom::getrandom(&mut nonce).map_err(|e| format!("无法生成临时文件名称: {e}"))?;
    let nonce: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{nonce}.tmp"));
    Ok(path.with_file_name(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_authority_matches_shared_rule_corpus() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/url-authorities.json"))
                .unwrap();
        for case in cases.as_array().unwrap() {
            let input = case["input"].as_str().unwrap();
            let actual = url_authority(input);
            if let Some(host) = case["host"].as_str() {
                let actual = actual.expect("supported authority must parse");
                assert_eq!(actual.host, host);
                assert_eq!(actual.port.map(u64::from), case["port"].as_u64());
            } else {
                assert!(
                    actual.is_none(),
                    "invalid authority must not produce a rule"
                );
            }
        }
    }

    #[test]
    fn url_host_uses_the_destination_authority() {
        for (input, expected) in [
            (
                "https://bank.example:443@unrelated.test/login",
                "unrelated.test",
            ),
            ("https://name:password@bank.example/login", "bank.example"),
            ("https://bücher.example/login", "xn--bcher-kva.example"),
            ("https://[2001:0db8::1]:443/login", "2001:db8::1"),
            ("//bank.example/login", "bank.example"),
        ] {
            assert_eq!(url_host(input).as_deref(), Some(expected));
        }
        for input in [
            "https://[::1",
            "https://bank.example:invalid/login",
            "https://bank.example\n.unrelated.test/login",
            "mailto:user@bank.example",
            "https://",
        ] {
            assert!(url_host(input).is_none(), "invalid authority was accepted");
        }
    }

    #[test]
    fn url_host_strips_scheme_port_and_path() {
        assert_eq!(
            url_host("https://github.com/login"),
            Some("github.com".into())
        );
        assert_eq!(url_host("http://a.b.c:8080/x?y=1"), Some("a.b.c".into()));
        assert_eq!(url_host("ftp://EXAMPLE.com:21"), Some("example.com".into()));
        assert_eq!(url_host("plain-host"), Some("plain-host".into()));
        assert_eq!(
            url_host("http://[2001:db8::1]:8080/x"),
            Some("2001:db8::1".into())
        );
        assert_eq!(url_host("https://[::1]/"), Some("::1".into()));
        assert_eq!(url_host("https://"), None);
        assert_eq!(url_host(""), None);
        assert_eq!(url_host("   "), None);
    }

    #[test]
    fn atomic_write_replaces_target_without_leaving_temp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.kdbx");
        atomic_write(&path, b"first", "数据库").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);

        atomic_write(&path, b"second", "数据库").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn parallel_atomic_writes_use_independent_temporary_files() {
        use std::sync::Barrier;
        let dir = tempfile::tempdir().unwrap();
        for attempt in 0..20 {
            let path = dir.path().join(format!("fixture-{attempt}.csv"));
            std::fs::write(&path, b"original").unwrap();
            let barrier = Barrier::new(3);
            let results = std::thread::scope(|scope| {
                let tasks: Vec<_> = (b'A'..=b'C')
                    .map(|tag| {
                        let path = &path;
                        let barrier = &barrier;
                        scope.spawn(move || {
                            let payload = vec![tag; 128 * 1024];
                            barrier.wait();
                            atomic_write(path, &payload, "fixture")
                        })
                    })
                    .collect();
                tasks
                    .into_iter()
                    .map(|task| task.join().unwrap())
                    .collect::<Vec<_>>()
            });
            assert!(results.iter().all(Result::is_ok), "{results:?}");
            let stored = std::fs::read(&path).unwrap();
            assert_eq!(stored.len(), 128 * 1024);
            assert!((b'A'..=b'C').any(|tag| stored.iter().all(|&byte| byte == tag)));
        }
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 20);
    }

    #[test]
    fn atomic_write_never_removes_a_legacy_sibling() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.csv");
        let legacy = dir.path().join("fixture.csv.tmp");
        std::fs::write(&legacy, b"another writer's file").unwrap();
        atomic_write(&path, b"new", "fixture").unwrap();
        assert_eq!(std::fs::read(&legacy).unwrap(), b"another writer's file");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn atomic_write_reports_save_failure() {
        let err = atomic_write(Path::new("Z:/not/a/real/dir/x"), b"data", "配置").unwrap_err();
        assert!(err.contains("写入配置失败"), "unexpected message: {err}");
    }
}
