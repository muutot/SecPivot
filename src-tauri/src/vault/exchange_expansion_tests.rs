use super::*;
use crate::vault::VaultSession;
use tempfile::TempDir;
use zeroize::Zeroizing;

#[derive(Deserialize)]
struct Fixture {
    over_budget: Vec<BudgetCase>,
    amplification: String,
    exact: String,
    empty: String,
}

#[derive(Deserialize)]
struct BudgetCase {
    name: String,
    limit: usize,
    xml: String,
    expanded_bytes: usize,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/exchange-expanded-binaries.json"
    ))
    .unwrap()
}

fn decoded(xml: &str) -> Zeroizing<Vec<u8>> {
    let framed = Zeroizing::new(encode_payload(xml.as_bytes(), false).unwrap());
    let document = Zeroizing::new(decode_payload(&framed).unwrap());
    assert_eq!(&*document, xml.as_bytes());
    document
}

fn expanded_size(entries: &[PastedEntry]) -> usize {
    entries
        .iter()
        .map(|entry| {
            entry
                .attachments
                .iter()
                .map(|(_, b, _)| b.len())
                .sum::<usize>()
                + entry.custom_icon.as_ref().map_or(0, Vec::len)
        })
        .sum()
}

#[test]
fn exchange_expansion_small_shared_references_have_distinct_owned_bytes() {
    let input = fixture();
    assert_eq!(input.amplification.len(), 3665);
    let parsed = parse_entries_document(&decoded(&input.amplification)).unwrap();
    assert_eq!(parsed.len(), 16);
    assert_eq!(expanded_size(&parsed), 16384);
    for entry in &parsed {
        assert_eq!(entry.attachments[0].1, vec![b'a'; 1024]);
        assert!(entry.attachments[0].2);
    }
    assert_ne!(
        parsed[0].attachments[0].1.as_ptr(),
        parsed[1].attachments[0].1.as_ptr()
    );
}

#[test]
fn exchange_expansion_budget_rejects_attachment_icon_and_combined_overflow() {
    let input = fixture();
    assert_eq!(input.over_budget.len(), 4);
    let mut accepted = Vec::new();
    for case in input.over_budget {
        let document = decoded(&case.xml);
        // All controls are small and valid through the production wrapper.
        assert_eq!(
            expanded_size(&parse_entries_document(&document).unwrap()),
            case.expanded_bytes
        );
        match parse_entries_document_limited(&document, case.limit) {
            Err(error) => assert_eq!(error, "剪贴板数据过大"),
            Ok(entries) => accepted.push((case.name, expanded_size(&entries))),
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted over-budget cases: {accepted:?}"
    );
}

#[test]
fn exchange_expansion_exact_and_unlimited_control_preserve_shared_bytes() {
    let document = decoded(&fixture().exact);
    for limit in [16, usize::MAX] {
        let parsed = parse_entries_document_limited(&document, limit).unwrap();
        assert_eq!(expanded_size(&parsed), 16);
        assert_eq!(parsed.len(), 2);
        for (index, entry) in parsed.iter().enumerate() {
            assert_eq!(entry.attachments[0].1, vec![0, 1, 2, 255]);
            assert_eq!(entry.attachments[0].2, index == 0);
            assert_eq!(entry.custom_icon, Some(vec![5, 6, 7, 8]));
        }
    }
}

#[test]
fn exchange_expansion_empty_and_unreferenced_binaries_use_no_clone_budget() {
    let document = decoded(&fixture().empty);
    let parsed = parse_entries_document_limited(&document, 0).unwrap();
    assert_eq!(parsed.len(), 3);
    assert_eq!(expanded_size(&parsed), 0);
    for entry in parsed {
        assert_eq!(entry.attachments.len(), 1);
        assert!(entry.attachments[0].1.is_empty());
        assert!(entry.attachments[0].2);
    }
}

#[test]
fn exchange_expansion_valid_shared_bytes_survive_native_save_and_reopen() {
    let document = decoded(&fixture().exact);
    let parsed = parse_entries_document_limited(&document, 16).unwrap();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-expanded-exchange.kdbx");
    let mut target = VaultSession::default();
    target
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let mut ids = Vec::new();
    for entry in &parsed {
        ids.push(insert_entry(&mut target.require_db_mut().unwrap().root_mut(), entry).unwrap());
    }
    target.mark_dirty();
    target.save().unwrap();
    target.close();
    target.open(&path, "synthetic-master", None).unwrap();
    for (index, id) in ids.into_iter().enumerate() {
        let entry = target
            .require_db()
            .unwrap()
            .entry(EntryId::from_uuid(id))
            .unwrap();
        let (name, attachment) = entry.attachments_named().next().unwrap();
        assert_eq!(entry.attachments_named().count(), 1);
        assert_eq!(name, "synthetic-0.bin");
        assert_eq!(attachment.data.get(), &[0, 1, 2, 255]);
        assert_eq!(attachment.data.is_protected(), index == 0);
        assert_eq!(entry.custom_icon().unwrap().data, vec![5, 6, 7, 8]);
    }
}
