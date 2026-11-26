//! Optional local diagnostics: UTC timestamps and redacted static templates only.

use chrono::Utc;
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

const MAX_LOG_BYTES: usize = 1024 * 1024;
const MAX_TEMPLATE_CHARS: usize = 2048;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticStatus {
    pub enabled: bool,
    pub active: bool,
    pub path: Option<String>,
    pub limit_reached: bool,
}

#[derive(Default)]
struct State {
    root: Option<PathBuf>,
    file: Option<File>,
    path: Option<PathBuf>,
    bytes: usize,
    limit_reached: bool,
}

#[derive(Default)]
pub(crate) struct DiagnosticLog {
    enabled: AtomicBool,
    state: Mutex<State>,
}

pub(crate) fn global() -> &'static DiagnosticLog {
    static LOG: OnceLock<DiagnosticLog> = OnceLock::new();
    LOG.get_or_init(DiagnosticLog::default)
}

impl DiagnosticLog {
    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    pub(crate) fn configure(&self, data_dir: &Path, enabled: bool) {
        let Ok(mut state) = self.state.lock() else {
            self.enabled.store(false, Ordering::Release);
            return;
        };
        if !enabled {
            self.enabled.store(false, Ordering::Release);
            state.file = None;
            return;
        }
        let root = data_dir.join("Logs");
        if state.root.as_ref() != Some(&root) || !self.is_enabled() {
            *state = State {
                root: Some(root.clone()),
                ..Default::default()
            };
        }
        self.enabled.store(true, Ordering::Release);
        if state.file.is_some() || state.limit_reached {
            return;
        }
        // Each enabled run gets a fresh create_new file. Existing log files or
        // final file links are never opened, overwritten, followed or deleted.
        static NEXT_LOG: AtomicU64 = AtomicU64::new(0);
        let path = root.join(format!(
            "secpivot-{}-{}-{}.log",
            Utc::now().format("%Y%m%dT%H%M%S%.9f"),
            std::process::id(),
            NEXT_LOG.fetch_add(1, Ordering::Relaxed)
        ));
        if std::fs::create_dir_all(&root).is_err() {
            return;
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let Ok(file) = options.open(&path) else {
            return;
        };
        state.file = Some(file);
        state.path = Some(path);
        state.bytes = 0;
        write_event(&mut state, "diagnostic logging started");
    }

    pub(crate) fn record_template(&self, template: &'static str) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        // Disable takes the same mutex. After it returns, no pending writer can
        // append a late event to the old file.
        if self.is_enabled() {
            write_event(&mut state, template);
        }
    }

    pub(crate) fn record_config_parse(
        &self,
        category: serde_json::error::Category,
        line: usize,
        column: usize,
        recovered: bool,
    ) {
        let category = match category {
            serde_json::error::Category::Io => "Io",
            serde_json::error::Category::Syntax => "Syntax",
            serde_json::error::Category::Data => "Data",
            serde_json::error::Category::Eof => "Eof",
        };
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if self.is_enabled() {
            write_message(&mut state, &format!("config.parse.failed category={category} line={line} column={column} recovered={recovered}"));
        }
    }

    pub(crate) fn status(&self) -> DiagnosticStatus {
        let Ok(state) = self.state.lock() else {
            return DiagnosticStatus::default();
        };
        DiagnosticStatus {
            enabled: self.is_enabled(),
            active: state.file.is_some(),
            path: state
                .path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            limit_reached: state.limit_reached,
        }
    }
}

fn write_event(state: &mut State, template: &'static str) {
    write_message(state, &redact_template(template));
}

fn write_message(state: &mut State, message: &str) {
    if state.file.is_none() {
        return;
    }
    let line = format!("{} {}\n", Utc::now().to_rfc3339(), message);
    if state.bytes.saturating_add(line.len()) > MAX_LOG_BYTES {
        state.limit_reached = true;
        state.file = None;
        return;
    }
    if state
        .file
        .as_mut()
        .unwrap()
        .write_all(line.as_bytes())
        .is_err()
    {
        state.file = None;
    } else {
        state.bytes += line.len();
    }
}

fn redact_template(template: &'static str) -> String {
    let mut chars = template.chars().take(MAX_TEMPLATE_CHARS).peekable();
    let mut output = String::new();
    while let Some(ch) = chars.next() {
        match ch {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                output.push('{');
            }
            '{' => {
                for inside in chars.by_ref() {
                    if inside == '}' {
                        break;
                    }
                }
                output.push_str("[redacted]");
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                output.push('}');
            }
            ch if ch.is_control() => output.push(' '),
            ch => output.push(ch),
        }
    }
    output
}

#[cfg(test)]
mod tests;
