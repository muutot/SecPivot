use super::*;
use crate::remote::{MemoryStorage, RemoteStorage};
use crate::vault::*;
use keepass::{config::KdfConfig, Database, DatabaseKey};
use std::sync::Arc;
use tempfile::TempDir;

fn now() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 26)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
}
fn serialized(
    changed: Option<NaiveDateTime>,
    rec: Option<isize>,
    force: Option<isize>,
    once: Option<bool>,
) -> Vec<u8> {
    let mut db = Database::new();
    db.config.kdf_config = KdfConfig::Aes { rounds: 1 };
    db.meta.master_key_changed = changed;
    db.meta.master_key_change_rec = rec;
    db.meta.master_key_change_force = force;
    db.meta.master_key_change_force_once = once;
    let mut bytes = Vec::new();
    db.save(
        &mut bytes,
        DatabaseKey::new().with_password("synthetic-expiry-only"),
    )
    .unwrap();
    bytes
}

#[test]
fn master_key_expiry_database_days_boundaries_priority_and_missing_dates() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-expiry.kdbx");
    for (changed, rec, force, once, expected) in [
        (
            Some(now() - Duration::days(30) + Duration::seconds(1)),
            Some(30),
            None,
            None,
            (false, false),
        ),
        (
            Some(now() - Duration::days(30)),
            Some(30),
            None,
            None,
            (true, false),
        ),
        (
            Some(now() - Duration::days(60)),
            Some(30),
            Some(60),
            None,
            (false, true),
        ),
        (
            Some(now() - Duration::days(60)),
            Some(-1),
            Some(-1),
            Some(false),
            (false, false),
        ),
        (
            Some(now() + Duration::days(2)),
            Some(0),
            Some(0),
            None,
            (false, false),
        ),
        (
            Some(now() + Duration::seconds(1)),
            Some(0),
            None,
            None,
            (true, false),
        ),
        (None, Some(0), Some(0), None, (false, false)),
        (None, None, None, Some(true), (false, true)),
    ] {
        let original = serialized(changed, rec, force, once);
        std::fs::write(&path, &original).unwrap();
        let mut session = VaultSession::default();
        session.open(&path, "synthetic-expiry-only", None).unwrap();
        let revision = session.revision;
        let offer = session
            .master_key_expiry_offer_at("", "", now(), now().date())
            .unwrap();
        assert_eq!((offer.recommended, offer.required), expected);
        assert_eq!(offer.force_once, once == Some(true));
        assert!(offer.invalid_rules.is_empty());
        assert_eq!(session.revision, revision);
        assert!(!session.dirty);
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let old_token = offer.token;
        session.close();
        session.open(&path, "synthetic-expiry-only", None).unwrap();
        assert_ne!(
            session
                .master_key_expiry_offer_at("", "", now(), now().date())
                .unwrap()
                .token,
            old_token
        );
        session.close();
    }
}

#[test]
fn master_key_expiry_duration_cutoff_failures_and_offline_read_only_assessment() {
    let dir = TempDir::new().unwrap();
    let storage = Arc::new(MemoryStorage::default());
    let changed = Some(now() - Duration::days(30));
    let original = serialized(changed, None, None, Some(false));
    storage.put("synthetic-expiry.kdbx", &original).unwrap();
    let mut session = VaultSession::default();
    session
        .open_remote(
            storage.clone(),
            "synthetic-expiry.kdbx",
            "synthetic-expiry-only",
            None,
            RemoteMode::InMemory,
            dir.path(),
            0,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    for (rec, force, expected, invalid) in [
        (" P30D ", "", (false, false), vec![]),
        ("P29D", "", (true, false), vec![]),
        ("P29D", "PT1S", (false, true), vec![]),
        ("2026-10-27T12:00:00Z", "", (false, false), vec![]),
        ("2026-10-27T12:00:00.0000001Z", "", (true, false), vec![]),
        ("2026-10-27T13:00:00+01:00", "", (false, false), vec![]),
        (
            "invalid",
            "P29D",
            (false, true),
            vec![InvalidExpiryRule::Recommended],
        ),
        (
            "P29D",
            "P",
            (true, false),
            vec![InvalidExpiryRule::Required],
        ),
        (
            "PT9223372036854775807S",
            "9999-99-99",
            (false, false),
            vec![InvalidExpiryRule::Recommended, InvalidExpiryRule::Required],
        ),
    ] {
        let offer = session
            .master_key_expiry_offer_at(rec, force, now(), now().date())
            .unwrap();
        assert_eq!(
            (offer.recommended, offer.required),
            expected,
            "{rec}/{force}"
        );
        assert_eq!(offer.invalid_rules, invalid);
    }
    for _ in 0..3 {
        session.note_save_failure();
    }
    let offer = session
        .master_key_expiry_offer_at("P1D", "", now(), now().date())
        .unwrap();
    assert!(offer.read_only && offer.recommended);
    assert_eq!(storage.get("synthetic-expiry.kdbx").unwrap(), original);
    session.close();
    assert!(session.master_key_expiry_offer("", "").is_err());
}

use chrono::Duration;

#[test]
fn master_key_expiry_xml_parser_matches_independent_dotnet_vectors() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/master-key-expiry-xml.json"
    ))
    .unwrap();
    let local_date =
        NaiveDate::parse_from_str(fixture["localDate"].as_str().unwrap(), "%Y-%m-%d").unwrap();
    for vector in fixture["durations"].as_array().unwrap() {
        let expected = vector["ticks"]
            .as_str()
            .map(|ticks| ticks.parse::<i64>().unwrap());
        assert_eq!(
            rules::duration_ticks(vector["rule"].as_str().unwrap().trim()).ok(),
            expected,
            "duration {}",
            vector["rule"]
        );
    }
    for vector in fixture["dates"].as_array().unwrap() {
        let expected = vector["utc"].as_str().map(|date| {
            chrono::DateTime::parse_from_rfc3339(date)
                .unwrap()
                .naive_utc()
        });
        assert_eq!(
            rules::cutoff_with_offset(vector["rule"].as_str().unwrap(), local_date, |_| Ok(
                fixture["boundaryOffsetSeconds"].as_i64().unwrap()
            ))
            .ok(),
            expected,
            "cutoff {}",
            vector["rule"]
        );
    }
}
