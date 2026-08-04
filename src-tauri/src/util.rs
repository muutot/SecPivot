//! Small shared helpers that don't belong to a domain module: URL host
//! extraction and atomic file writes.

use std::path::{Path, PathBuf};

/// Lower-cased host of a URL, with scheme/port/path stripped.
///
/// Any `://` scheme is dropped, then the leading host portion up to the first
/// of `/`, `:`, `?`, `#` is taken, trimmed, and lower-cased. Plain hostnames
/// and `host:port` inputs pass through unchanged (everything after the host is
/// a delimiter). Returns `None` when nothing remains, so callers can distinguish
/// "no host" from a literal `""`.
pub fn url_host(url: &str) -> Option<String> {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest
        .split(['/', ':', '?', '#'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// Write `bytes` to `path` atomically: write to a sibling `.tmp` file, then
/// rename it over the target so readers never observe a half-written file. On
/// rename failure the temp file is removed. `what` names the file type in the
/// user-facing error messages (e.g. `"配置"`, `"数据库"`).
pub fn atomic_write(path: &Path, bytes: &[u8], what: &str) -> Result<(), String> {
    let tmp = temp_sibling(path);
    std::fs::write(&tmp, bytes).map_err(|e| format!("写入{what}失败: {e}"))?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("保存{what}失败: {e}"));
    }
    Ok(())
}

/// `<name>.<ext>.tmp` sibling for an atomic write (`config.json` →
/// `config.json.tmp`, `vault.kdbx` → `vault.kdbx.tmp`).
fn temp_sibling(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_host_strips_scheme_port_and_path() {
        assert_eq!(
            url_host("https://github.com/login"),
            Some("github.com".into())
        );
        assert_eq!(url_host("http://a.b.c:8080/x?y=1"), Some("a.b.c".into()));
        assert_eq!(url_host("ftp://EXAMPLE.com:21"), Some("example.com".into()));
        assert_eq!(url_host("plain-host"), Some("plain-host".into()));
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
        assert!(!dir.path().join("test.kdbx.tmp").exists());

        atomic_write(&path, b"second", "数据库").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        assert!(!dir.path().join("test.kdbx.tmp").exists());
    }

    #[test]
    fn atomic_write_reports_save_failure() {
        let err = atomic_write(Path::new("Z:/not/a/real/dir/x"), b"data", "配置").unwrap_err();
        assert!(err.contains("写入配置失败"), "unexpected message: {err}");
    }
}
