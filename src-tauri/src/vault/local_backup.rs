//! Encrypted pre-overwrite backups with a bounded, owned-name retention index.

use crate::config::LocalBackupSettings;
use crate::remote::backup::{expand_backup_template, valid_file_name, validate_backup_template};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const INDEX_LIMIT: u64 = 64 * 1024;
const INDEX_NAME: &str = "index.json";
pub(crate) const BUSY: &str = "另一项本地备份正在进行，请稍后重试";

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    version: u8,
    files: Vec<String>,
}

pub(crate) fn validate_template(template: &str) -> Result<String, String> {
    let template = template.trim();
    let literal = template
        .replace("{name}", "")
        .replace("{timestamp}", "")
        .replace("{ext}", "");
    if template.matches("{timestamp}").count() != 1 || literal.contains(['{', '}']) {
        return Err("本地备份模板需要一个 {timestamp}；支持 {name}、{timestamp}、{ext}".into());
    }
    validate_backup_template(template, "vault.kdbx", "vault", "kdbx")?;
    let candidate = expand_backup_template(template, "vault", "20000101000000.000", "kdbx");
    if [INDEX_NAME, "writer.lock"]
        .iter()
        .any(|name| candidate.eq_ignore_ascii_case(name))
    {
        return Err("备份模板不能使用内部清单或锁文件名".into());
    }
    Ok(template.to_owned())
}

/// A dedicated adjacent directory keeps each source file's retention independent.
pub(crate) fn directory(path: &Path) -> Result<PathBuf, String> {
    let name = path.file_name().ok_or("数据库文件名无效")?;
    let mut directory = std::ffi::OsString::from(".");
    directory.push(name);
    directory.push(".secpivot-backups");
    Ok(path.with_file_name(directory))
}

fn regular(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_file() => Ok(true),
        Ok(_) => Err("备份路径不是普通文件，已取消操作".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("无法检查备份文件: {error}")),
    }
}

fn lock_directory(dir: &Path) -> Result<File, String> {
    match fs::symlink_metadata(dir) {
        Ok(meta) if meta.file_type().is_dir() => {}
        Ok(_) => return Err("备份目录不是普通目录，已取消保存".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let builder = fs::DirBuilder::new();
            #[cfg(unix)]
            let mut builder = builder;
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder
                .create(dir)
                .map_err(|error| format!("无法创建备份目录: {error}"))?;
        }
        Err(error) => return Err(format!("无法检查备份目录: {error}")),
    }
    let path = dir.join("writer.lock");
    regular(&path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options
        .open(path)
        .map_err(|error| format!("无法打开备份锁: {error}"))?;
    lock.try_lock().map_err(|_| BUSY.to_owned())?;
    Ok(lock)
}

fn load_index(dir: &Path) -> Result<Index, String> {
    let path = dir.join(INDEX_NAME);
    if !regular(&path)? {
        return Ok(Index {
            version: 1,
            files: Vec::new(),
        });
    }
    let bytes = super::file_io::read_with_limit(&path, "备份清单", INDEX_LIMIT)?;
    let index: Index =
        serde_json::from_slice(&bytes).map_err(|_| "备份清单无效，已取消保存".to_owned())?;
    let mut seen = std::collections::HashSet::new();
    if index.version != 1
        || index.files.len() > 1024
        || index.files.iter().any(|name| {
            !valid_file_name(name)
                || [INDEX_NAME, "writer.lock"]
                    .iter()
                    .any(|reserved| name.eq_ignore_ascii_case(reserved))
                || !seen.insert(name.to_lowercase())
        })
    {
        return Err("备份清单无效，已取消保存".into());
    }
    Ok(index)
}

fn save_index(dir: &Path, index: &Index) -> Result<(), String> {
    let bytes = serde_json::to_vec(index).map_err(|_| "无法生成备份清单".to_owned())?;
    if bytes.len() as u64 > INDEX_LIMIT {
        return Err("备份清单已达到容量上限，请检查备份目录".into());
    }
    crate::util::atomic_write(&dir.join(INDEX_NAME), &bytes, "备份清单")
}

fn create_backup(
    dir: &Path,
    template: &str,
    stem: &str,
    ext: &str,
    bytes: &[u8],
    stamp: &str,
) -> Result<String, String> {
    for suffix in 0..128 {
        let stamp = if suffix == 0 {
            stamp.to_owned()
        } else {
            format!("{stamp}_{suffix}")
        };
        let name = expand_backup_template(template, stem, &stamp, ext);
        if !valid_file_name(&name)
            || [INDEX_NAME, "writer.lock"]
                .iter()
                .any(|reserved| name.eq_ignore_ascii_case(reserved))
        {
            return Err("备份模板生成了无效文件名".into());
        }
        let path = dir.join(&name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = match options.open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("无法创建本地备份: {error}")),
        };
        let written = file.write_all(bytes).and_then(|()| file.sync_all());
        drop(file);
        if let Err(error) = written {
            let _ = fs::remove_file(&path);
            return Err(format!("本地备份写入失败: {error}"));
        }
        return Ok(name);
    }
    Err("无法分配独立的备份文件名，请稍后重试".into())
}

/// Only names recorded by successful backup writes may be removed; untracked
/// same-shape files and crash leftovers remain untouched.
fn prune(dir: &Path, index: &mut Index, keep: usize) -> Result<(), String> {
    let excess = index.files.len().saturating_sub(keep);
    let old = std::mem::take(&mut index.files);
    let mut failed = false;
    for (position, name) in old.into_iter().enumerate() {
        if position < excess {
            let path = dir.join(&name);
            match regular(&path) {
                Ok(false) => continue,
                Ok(true) if fs::remove_file(&path).is_ok() => continue,
                _ => failed = true,
            }
        }
        index.files.push(name);
    }
    save_index(dir, index)?;
    if failed {
        return Err("部分旧备份无法清理，请检查备份目录".into());
    }
    Ok(())
}

pub(crate) fn write(
    path: &Path,
    bytes: &[u8],
    settings: &LocalBackupSettings,
) -> Result<Option<String>, String> {
    write_with(path, bytes, settings, |path, bytes| {
        super::helpers::write_database_bytes(path, bytes)
    })
}

fn write_with(
    path: &Path,
    bytes: &[u8],
    settings: &LocalBackupSettings,
    write: impl FnOnce(&Path, &[u8]) -> Result<(), String>,
) -> Result<Option<String>, String> {
    if !settings.enabled {
        return write(path, bytes).map(|()| None);
    }
    if super::file_io::is_document_uri(path) {
        return Err("文档 URI 不支持同目录本地备份".into());
    }
    if !(1..=50).contains(&settings.count) {
        return Err("本地备份保留数量必须为 1 至 50".into());
    }
    let template = validate_template(&settings.template)?;
    if !regular(path)? {
        return write(path, bytes).map(|()| None);
    }
    let dir = directory(path)?;
    let _lock = lock_directory(&dir)?;
    let mut index = load_index(&dir)?;
    let previous = super::file_io::read(path, "原数据库")?;
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or("数据库文件名无效")?;
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("kdbx");
    let stamp = chrono::Utc::now().format("%Y%m%d%H%M%S%.3f").to_string();
    let name = create_backup(&dir, &template, stem, ext, &previous, &stamp)?;
    index.files.push(name);
    // Publish ownership before overwrite. A failed index or primary write
    // retains the complete backup and never triggers retention deletion.
    save_index(&dir, &index)?;
    write(path, bytes)?;
    Ok(prune(&dir, &mut index, settings.count as usize)
        .err()
        .map(|error| format!("本地已保存，备份清理未完成: {error}")))
}

#[cfg(test)]
mod tests;
