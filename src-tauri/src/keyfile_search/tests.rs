use super::*;
use tempfile::TempDir;

#[test]
fn official_drive_classes_exclude_optical_and_gate_removable() {
    for kind in [0, 3, 4, 6] {
        assert!(eligible_drive(kind, false));
    }
    for kind in [1, 5, 7] {
        assert!(!eligible_drive(kind, true));
    }
    assert!(!eligible_drive(2, false));
    assert!(eligible_drive(2, true));
}

#[test]
fn suggestions_are_top_level_case_insensitive_paths_without_content_reads() {
    let dir = TempDir::new().unwrap();
    let key = dir.path().join("synthetic.KEYX");
    let bytes = b"synthetic file whose contents must stay untouched";
    std::fs::write(&key, bytes).unwrap();
    std::fs::write(dir.path().join("empty.key"), []).unwrap();
    std::fs::write(dir.path().join("other.kdbx"), bytes).unwrap();
    let nested = dir.path().join("directory.key");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("nested.key"), bytes).unwrap();
    let result = collect(vec![dir.path().to_owned(), dir.path().to_owned()]);
    assert!(result.supported);
    assert!(!result.incomplete);
    assert_eq!(result.paths.len(), 2);
    assert!(result.paths.contains(&key.to_str().unwrap().to_owned()));
    assert_eq!(std::fs::read(key).unwrap(), bytes);
    let wire = serde_json::to_value(&result).unwrap();
    assert_eq!(wire.as_object().unwrap().len(), 3);
    assert!(!wire.to_string().contains("contents must stay untouched"));
}

#[test]
fn inaccessible_roots_and_candidate_caps_are_explicitly_incomplete() {
    let dir = TempDir::new().unwrap();
    assert!(collect(vec![dir.path().join("missing")]).incomplete);
    for index in 0..=MAX_SUGGESTIONS {
        std::fs::write(dir.path().join(format!("synthetic-{index}.key")), []).unwrap();
    }
    let result = collect(vec![dir.path().to_owned()]);
    assert_eq!(result.paths.len(), MAX_SUGGESTIONS);
    assert!(result.incomplete);
}

#[tokio::test]
async fn disabled_search_does_not_discover_any_actual_drives() {
    let result = search(false, true).await;
    assert!(result.paths.is_empty());
    assert!(!result.incomplete);
}

#[tokio::test]
async fn timed_out_worker_keeps_its_permit_until_the_filesystem_job_returns() {
    let gate = Arc::new(Semaphore::new(1));
    let (release, wait) = std::sync::mpsc::channel();
    let result = run_bounded(gate.clone(), Duration::from_millis(1), move || {
        wait.recv().unwrap();
        KeyfileSuggestions::default()
    })
    .await;
    assert!(result.incomplete);
    assert_eq!(gate.available_permits(), 0);
    let second = run_bounded(gate.clone(), SEARCH_TIMEOUT, || {
        panic!("a second blocking read must not start")
    })
    .await;
    assert!(second.incomplete);
    release.send(()).unwrap();
    let permit = tokio::time::timeout(SEARCH_TIMEOUT, gate.acquire())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
    assert_eq!(gate.available_permits(), 1);
}
