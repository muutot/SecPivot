use super::*;
use std::fmt;
use std::sync::atomic::AtomicUsize;
use tempfile::TempDir;

#[test]
fn disabled_logging_creates_nothing_and_never_evaluates_payloads() {
    let dir = TempDir::new().unwrap();
    let log = DiagnosticLog::default();
    log.configure(dir.path(), false);
    let evaluated = AtomicUsize::new(0);
    diagnostic_event!(
        log,
        "synthetic {}",
        evaluated.fetch_add(1, Ordering::SeqCst)
    );
    assert_eq!(evaluated.load(Ordering::SeqCst), 0);
    assert!(!dir.path().join("Logs").exists());
    assert!(!log.status().enabled);
    assert!(!log.status().active);
}

#[test]
fn actual_event_macro_writes_static_templates_without_formatting_sensitive_payloads() {
    struct Payload;
    impl fmt::Display for Payload {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            panic!("sensitive payload must never be formatted");
        }
    }
    let dir = TempDir::new().unwrap();
    let log = DiagnosticLog::default();
    log.configure(dir.path(), true);
    let payload = Payload;
    let credential = "synthetic-secret-that-must-never-be-logged";
    diagnostic_event!(
        log,
        "synthetic operation {payload}, credential {credential}, {{static}} {}",
        credential
    );
    let status = log.status();
    assert!(status.enabled && status.active);
    let path = status.path.unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(
        text.contains("synthetic operation [redacted], credential [redacted], {static} [redacted]")
    );
    assert!(!text.contains(credential));
    log.configure(dir.path(), false);
    diagnostic_event!(log, "late synthetic event {credential}");
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    log.configure(dir.path(), true);
    assert_ne!(log.status().path.as_deref(), Some(path.as_str()));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn unavailable_destination_preserves_existing_data_and_reports_inactive_logging() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("Logs"), b"synthetic existing data").unwrap();
    let log = DiagnosticLog::default();
    log.configure(dir.path(), true);
    diagnostic_event!(log, "cannot write synthetic diagnostic");
    assert!(log.status().enabled);
    assert!(!log.status().active);
    assert_eq!(
        std::fs::read(dir.path().join("Logs")).unwrap(),
        b"synthetic existing data"
    );
}

#[test]
fn log_limit_closes_the_sink_without_overwriting_or_deleting_existing_logs() {
    let dir = TempDir::new().unwrap();
    let log = DiagnosticLog::default();
    log.configure(dir.path(), true);
    let path = log.status().path.unwrap();
    for _ in 0..20_000 {
        log.record_template("synthetic bounded logging event {untrusted_argument}");
        if log.status().limit_reached {
            break;
        }
    }
    assert!(log.status().limit_reached);
    assert!(!log.status().active);
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.len() <= MAX_LOG_BYTES);
    log.record_template("synthetic late event");
    log.configure(dir.path(), true);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(!log.status().active);
}
