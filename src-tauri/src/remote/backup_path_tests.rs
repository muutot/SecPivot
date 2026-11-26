use super::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn backup_templates_cannot_escape_or_replace_the_current_mirror() {
    let root = tempdir().unwrap();
    let mirror = root.path().join("mirror");
    fs::create_dir(&mirror).unwrap();
    fs::write(mirror.join("seed.kdbx"), b"old encrypted mirror").unwrap();
    let outside = root.path().join("outside.kdbx.bak");
    fs::write(&outside, b"unrelated outside file").unwrap();
    for template in [
        "../outside.kdbx.bak",
        "..\\outside.kdbx.bak",
        "nested/{name}.{timestamp}.bak",
        "{name}.{ext}",
        "SEED.KDBX",
        "{name}.{timestamp}.bak.",
        "{name}.{timestamp}.bak ",
        "NUL",
    ] {
        assert!(
            write_local_copy(&mirror, "seed.kdbx", b"new encrypted mirror", 2, template).is_err(),
            "unsafe template must be rejected: {template}"
        );
        assert_eq!(
            fs::read(mirror.join("seed.kdbx")).unwrap(),
            b"old encrypted mirror"
        );
        assert_eq!(fs::read(&outside).unwrap(), b"unrelated outside file");
    }
    assert_eq!(fs::read_dir(&mirror).unwrap().count(), 1);
}

#[test]
fn absolute_drive_and_device_names_fail_without_filesystem_access() {
    for name in [
        "/seed.bak",
        "C:seed.bak",
        "C:\\seed.bak",
        "\\\\host\\seed.bak",
        "NUL",
        "CON.txt",
        "LPT1.bak",
        "COM9",
    ] {
        assert!(
            !valid_file_name(name),
            "invalid cross-platform filename: {name}"
        );
    }
    for name in [
        "seed.20261127120000.000.kdbx.bak",
        "数据库.bak",
        "{unknown}.bak",
    ] {
        assert!(valid_file_name(name));
    }
}

#[test]
fn invalid_backup_templates_are_rejected_before_the_first_directory_is_created() {
    let root = tempdir().unwrap();
    let mirror = root.path().join("new-mirror");
    assert!(write_local_copy(&mirror, "seed.kdbx", b"encrypted", 3, "../{name}.bak").is_err());
    assert!(!mirror.exists());
    assert!(write_local_copy(&mirror, "..", b"encrypted", 0, "unused").is_err());
    assert!(!mirror.exists());
    write_local_copy(&mirror, "seed.kdbx", b"encrypted", 0, "unused/disabled").unwrap();
    assert_eq!(fs::read(mirror.join("seed.kdbx")).unwrap(), b"encrypted");
}

#[test]
fn retention_preserves_unrelated_files_and_directories_and_orders_numeric_collisions() {
    let root = tempdir().unwrap();
    let stamp = "20261127120000000";
    for suffix in ["", "_1", "_2", "_10"] {
        fs::write(
            root.path().join(format!("seed.{stamp}{suffix}.kdbx.bak")),
            suffix,
        )
        .unwrap();
    }
    for name in [
        "seed.notes.kdbx.bak",
        "seed.9.kdbx.bak",
        "seed.kdbx",
        "other.20261127120000000.kdbx.bak",
    ] {
        fs::write(root.path().join(name), b"unrelated").unwrap();
    }
    let directory = root.path().join("seed.20261126120000000.kdbx.bak");
    fs::create_dir(&directory).unwrap();
    prune_local_backups(
        root.path(),
        "seed",
        "kdbx",
        2,
        "{name}.{timestamp}.{ext}.bak",
    )
    .unwrap();
    assert!(!root.path().join(format!("seed.{stamp}.kdbx.bak")).exists());
    assert!(!root
        .path()
        .join(format!("seed.{stamp}_1.kdbx.bak"))
        .exists());
    assert!(root
        .path()
        .join(format!("seed.{stamp}_2.kdbx.bak"))
        .exists());
    assert!(root
        .path()
        .join(format!("seed.{stamp}_10.kdbx.bak"))
        .exists());
    assert!(directory.is_dir());
    for name in [
        "seed.notes.kdbx.bak",
        "seed.9.kdbx.bak",
        "seed.kdbx",
        "other.20261127120000000.kdbx.bak",
    ] {
        assert_eq!(fs::read(root.path().join(name)).unwrap(), b"unrelated");
    }
}

#[test]
fn nonrotating_templates_never_authorize_retention_deletion() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("seed.kdbx"), b"current mirror").unwrap();
    fs::write(root.path().join("seed.kdbx.bak"), b"legacy backup").unwrap();
    for template in ["{name}.{ext}", "{name}.{ext}.bak"] {
        prune_local_backups(root.path(), "seed", "kdbx", 0, template).unwrap();
    }
    assert_eq!(
        fs::read(root.path().join("seed.kdbx")).unwrap(),
        b"current mirror"
    );
    assert_eq!(
        fs::read(root.path().join("seed.kdbx.bak")).unwrap(),
        b"legacy backup"
    );
}

#[test]
fn retention_excludes_a_current_mirror_that_looks_like_a_timestamp() {
    let root = tempdir().unwrap();
    let name = "20260101120000000";
    fs::write(root.path().join(name), b"extensionless current mirror").unwrap();
    fs::write(root.path().join("20261127120000000"), b"rotated backup").unwrap();
    prune_local_backups(root.path(), name, "kdbx", 0, "{timestamp}").unwrap();
    assert_eq!(
        fs::read(root.path().join(name)).unwrap(),
        b"extensionless current mirror"
    );
    assert!(!root.path().join("20261127120000000").exists());
}
