//! KeePass-style drive-root suggestions. Never read key contents or recurse.

use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::Semaphore;

const MAX_ENTRIES_PER_ROOT: usize = 4096;
const MAX_SUGGESTIONS: usize = 128;
const SEARCH_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KeyfileSuggestions {
    pub paths: Vec<String>,
    pub incomplete: bool,
    pub supported: bool,
}

#[cfg(any(windows, test))]
fn eligible_drive(kind: u32, removable: bool) -> bool {
    matches!(kind, 0 | 3 | 4 | 6) || (kind == 2 && removable)
}

#[cfg(windows)]
fn drive_roots(removable: bool) -> Option<Vec<PathBuf>> {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    // Both calls use only valid NUL-terminated drive-root strings. They do not
    // open/read keyfiles. Readiness/access errors are handled by read_dir.
    let drives = unsafe { GetLogicalDrives() };
    if drives == 0 {
        return None;
    }
    Some(
        (0..26)
            .filter(|index| drives & (1 << index) != 0)
            .filter_map(|index| {
                let root = format!("{}:\\", char::from(b'A' + index));
                let wide: Vec<u16> = root.encode_utf16().chain(Some(0)).collect();
                eligible_drive(unsafe { GetDriveTypeW(wide.as_ptr()) }, removable)
                    .then(|| PathBuf::from(root))
            })
            .collect(),
    )
}

#[cfg(not(windows))]
fn drive_roots(_: bool) -> Option<Vec<PathBuf>> {
    Some(Vec::new())
}

fn collect(roots: Vec<PathBuf>) -> KeyfileSuggestions {
    let mut result = KeyfileSuggestions {
        supported: true,
        ..Default::default()
    };
    let mut seen = HashSet::new();
    for root in roots {
        let entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(_) => {
                result.incomplete = true;
                continue;
            }
        };
        for (index, entry) in entries.enumerate() {
            if index >= MAX_ENTRIES_PER_ROOT || result.paths.len() >= MAX_SUGGESTIONS {
                result.incomplete = true;
                break;
            }
            let Ok(entry) = entry else {
                result.incomplete = true;
                continue;
            };
            let Ok(kind) = entry.file_type() else {
                result.incomplete = true;
                continue;
            };
            // file_type does not follow symlinks, directories or junctions.
            if !kind.is_file() {
                continue;
            }
            let path = entry.path();
            let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
                continue;
            };
            if !extension.eq_ignore_ascii_case("key") && !extension.eq_ignore_ascii_case("keyx") {
                continue;
            }
            let Some(path) = path.to_str() else {
                result.incomplete = true;
                continue;
            };
            if seen.insert(path.to_lowercase()) {
                result.paths.push(path.to_owned());
            }
        }
    }
    result.paths.sort();
    result
}

async fn run_bounded(
    gate: Arc<Semaphore>,
    timeout: Duration,
    job: impl FnOnce() -> KeyfileSuggestions + Send + 'static,
) -> KeyfileSuggestions {
    let incomplete = || KeyfileSuggestions {
        supported: true,
        incomplete: true,
        ..Default::default()
    };
    let Ok(permit) = gate.try_acquire_owned() else {
        return incomplete();
    };
    let work = tokio::task::spawn_blocking(move || {
        // A timed-out filesystem read may keep running. Its permit stays in the
        // worker, preventing repeated prompts from spawning unlimited reads.
        let _permit = permit;
        job()
    });
    match tokio::time::timeout(timeout, work).await {
        Ok(Ok(result)) => result,
        _ => incomplete(),
    }
}

pub(crate) async fn search(enabled: bool, removable: bool) -> KeyfileSuggestions {
    if !enabled || !cfg!(windows) {
        return KeyfileSuggestions {
            supported: cfg!(windows),
            ..Default::default()
        };
    }
    static GATE: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let gate = GATE.get_or_init(|| Arc::new(Semaphore::new(1))).clone();
    run_bounded(gate, SEARCH_TIMEOUT, move || match drive_roots(removable) {
        Some(roots) => collect(roots),
        None => KeyfileSuggestions {
            supported: true,
            incomplete: true,
            ..Default::default()
        },
    })
    .await
}

#[cfg(test)]
mod tests;
