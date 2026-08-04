//! Local mirror + backup rotation for remote vaults: write the downloaded
//! KDBX bytes under the local cache dir, rotating timestamped backups and
//! pruning to a retention count. Extracted from `vault.rs`; pure std + chrono.

use std::path::{Path, PathBuf};

/// Validate an S3 object key for a vault file. Keys need not end in `.kdbx`:
/// whether the object really is a database is decided by the KDBX parse.
pub(crate) fn validate_remote_key(key: &str) -> Result<String, String> {
    let key = key.trim().trim_start_matches('/').to_owned();
    if key.is_empty() {
        return Err("远程文件 Key 不能为空".to_owned());
    }
    Ok(key)
}

/// Basename of an S3 object key, used as the local mirror file name.
pub(crate) fn remote_key_basename(key: &str) -> String {
    key.rsplit('/').next().unwrap_or(key).to_owned()
}

/// Expand a backup name template. `{name}` → file stem, `{timestamp}` →
/// `YYYYMMDDHHmmssSSS`, `{ext}` → extension without the dot (falls back to
/// the caller's `default_ext` when the source file has none). Unknown tokens
/// are kept verbatim so a typo cannot silently drop the extension.
pub(crate) fn expand_backup_template(template: &str, name: &str, stamp: &str, ext: &str) -> String {
    template
        .replace("{name}", name)
        .replace("{timestamp}", stamp)
        .replace("{ext}", ext)
}

/// Whether `candidate` matches the shape a template produces: the template is
/// filled in for `{name}`/`{ext}` and `{timestamp}` acts as a wildcard run.
/// A template without `{timestamp}` matches only the exact filled name.
pub(crate) fn template_matches(candidate: &str, name: &str, ext: &str, template: &str) -> bool {
    let filled = template.replace("{name}", name).replace("{ext}", ext);
    match filled.split_once("{timestamp}") {
        Some((before, after)) => {
            candidate.len() >= before.len() + after.len()
                && candidate.starts_with(before)
                && candidate.ends_with(after)
        }
        None => candidate == filled,
    }
}

/// Write the local mirror of a remote vault under `dir`, rotating up to
/// `backup_count` timestamped backups (named via `backup_template`) of the
/// previous file first.
pub(crate) fn write_local_copy(
    dir: &Path,
    name: &str,
    bytes: &[u8],
    backup_count: usize,
    backup_template: &str,
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建本地目录失败: {e}"))?;
    let dest = dir.join(name);
    if backup_count > 0 && dest.exists() {
        let stem = dest
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(name)
            .to_owned();
        let ext = dest
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("kdbx")
            .to_owned();
        let stamp = chrono::Utc::now().format("%Y%m%d%H%M%S%.3f").to_string();
        let backup = dir.join(expand_backup_template(backup_template, &stem, &stamp, &ext));
        if backup != dest {
            std::fs::rename(&dest, &backup).map_err(|e| format!("创建本地备份失败: {e}"))?;
        }
        prune_local_backups(dir, &stem, &ext, backup_count, backup_template)?;
    }
    std::fs::write(&dest, bytes).map_err(|e| format!("写入本地副本失败: {e}"))
}

/// Keep only the newest `keep` backup files matching `backup_template`.
pub(crate) fn prune_local_backups(
    dir: &Path,
    stem: &str,
    ext: &str,
    keep: usize,
    backup_template: &str,
) -> Result<(), String> {
    let mut backups: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("读取本地备份目录失败: {e}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|name| template_matches(name, stem, ext, backup_template))
        })
        .collect();
    backups.sort();
    let total = backups.len();
    for path in backups.into_iter().take(total.saturating_sub(keep)) {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_expands_known_tokens_and_keeps_unknown() {
        assert_eq!(
            expand_backup_template(
                "{name}.{timestamp}.{ext}.bak",
                "seed",
                "20260101120000000",
                "kdbx"
            ),
            "seed.20260101120000000.kdbx.bak"
        );
        assert_eq!(
            expand_backup_template("{name}-{typo}", "seed", "20260101120000000", "kdbx"),
            "seed-{typo}"
        );
    }

    #[test]
    fn template_match_uses_timestamp_as_wildcard() {
        let t = "{name}.{timestamp}.{ext}.bak";
        assert!(template_matches(
            "seed.20260101120000000.kdbx.bak",
            "seed",
            "kdbx",
            t
        ));
        assert!(template_matches("seed.9.kdbx.bak", "seed", "kdbx", t));
        assert!(!template_matches(
            "other.20260101120000000.kdbx.bak",
            "seed",
            "kdbx",
            t
        ));
        assert!(!template_matches(
            "seed.20260101120000000.other.bak",
            "seed",
            "kdbx",
            t
        ));
        assert!(template_matches(
            "seed.kdbx.bak",
            "seed",
            "kdbx",
            "{name}.{ext}.bak"
        ));
        assert!(!template_matches(
            "seed.kdbx.bak.extra",
            "seed",
            "kdbx",
            "{name}.{ext}.bak"
        ));
    }

    #[test]
    fn key_helpers_trim_and_split() {
        assert_eq!(
            validate_remote_key("/vaults/a.kdbx").unwrap(),
            "vaults/a.kdbx"
        );
        assert_eq!(
            validate_remote_key("  vaults/a.kdbx  ").unwrap(),
            "vaults/a.kdbx"
        );
        assert!(validate_remote_key("").is_err());
        assert_eq!(remote_key_basename("vaults/a.kdbx"), "a.kdbx");
        assert_eq!(remote_key_basename("a.kdbx"), "a.kdbx");
    }

    #[test]
    fn write_local_copy_rotates_and_prunes() {
        let dir = tempfile::tempdir().unwrap();
        let name = "seed.kdbx";
        write_local_copy(
            dir.path(),
            name,
            b"first",
            2,
            "{name}.{timestamp}.{ext}.bak",
        )
        .unwrap();
        write_local_copy(
            dir.path(),
            name,
            b"second",
            2,
            "{name}.{timestamp}.{ext}.bak",
        )
        .unwrap();
        write_local_copy(
            dir.path(),
            name,
            b"third",
            2,
            "{name}.{timestamp}.{ext}.bak",
        )
        .unwrap();
        assert_eq!(std::fs::read(dir.path().join(name)).unwrap(), b"third");
        let backups: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("seed.") && n.ends_with(".bak"))
            .collect();
        assert_eq!(backups.len(), 2, "keeps only the newest two");
    }

    #[test]
    fn write_local_copy_without_backups_just_writes() {
        let dir = tempfile::tempdir().unwrap();
        write_local_copy(
            dir.path(),
            "plain.kdbx",
            b"data",
            0,
            "{name}.{timestamp}.{ext}.bak",
        )
        .unwrap();
        assert_eq!(
            std::fs::read(dir.path().join("plain.kdbx")).unwrap(),
            b"data"
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
