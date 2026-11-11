use super::*;
use zeroize::Zeroizing;

#[test]
fn exchange_encode_rejects_oversized_documents_even_when_gzip_is_small() {
    let mut accepted = Vec::new();
    for limit in [0, 1, 32, 4096] {
        let document = Zeroizing::new(vec![b'x'; limit + 1]);
        match encode_payload_limited(&document, false, limit) {
            Err(error) => assert_eq!(error, "剪贴板数据过大"),
            Ok(payload) => accepted.push((limit, payload.len())),
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted oversized documents: {accepted:?}"
    );
}

#[test]
fn exchange_encode_rejects_frames_over_budget_including_flags_and_gzip_overhead() {
    let incompressible: Vec<u8> = (0..=255).collect();
    let mut accepted = Vec::new();
    for (document, limit) in [
        (&[][..], 0),
        (&[][..], 4),
        (&[][..], 23),
        (&incompressible[..], 256),
    ] {
        assert!(document.len() <= limit);
        let control = Zeroizing::new(encode_payload(document, false).unwrap());
        assert!(control.len() > limit);
        match encode_payload_limited(document, false, limit) {
            Err(error) => assert_eq!(error, "剪贴板数据过大"),
            Ok(payload) => accepted.push((limit, payload.len())),
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted oversized frames: {accepted:?}"
    );
}

#[test]
fn exchange_encode_valid_exact_frames_and_unlimited_controls_keep_bytes() {
    for length in [0, 1, 31, 256] {
        let document = Zeroizing::new((0..length).map(|i| i as u8).collect::<Vec<_>>());
        let reference = Zeroizing::new(encode_payload(&document, false).unwrap());
        let exact_limit = document.len().max(reference.len());
        for limit in [exact_limit, usize::MAX] {
            let payload = Zeroizing::new(encode_payload_limited(&document, false, limit).unwrap());
            assert_eq!(payload.len(), reference.len());
            assert_eq!(&payload[..4], &[0, 0, 0, 0]);
            assert_eq!(&payload[4..6], &[0x1f, 0x8b]);
            assert_eq!(decode_payload(&payload).unwrap(), *document);
        }
    }
}

#[cfg(target_os = "windows")]
#[test]
fn exchange_encode_counts_dpapi_overhead_and_preserves_encrypted_exact_control() {
    // Native DPAPI sees only an empty synthetic document, never a user vault.
    let document = b"";
    let control = Zeroizing::new(encode_payload(document, true).unwrap());
    let plain = Zeroizing::new(encode_payload(document, false).unwrap());
    assert!(control.len() > plain.len());
    let over_budget = encode_payload_limited(document, true, control.len() - 1);
    assert!(
        over_budget.is_err(),
        "encrypted frame exceeded budget; plain={} protected={} proposed={}",
        plain.len(),
        control.len(),
        control.len() - 1
    );
    assert_eq!(over_budget.err().unwrap(), "剪贴板数据过大");
    let exact = Zeroizing::new(encode_payload_limited(document, true, control.len()).unwrap());
    assert_eq!(exact.len(), control.len());
    assert_eq!(&exact[..4], &[1, 0, 0, 0]);
    assert_eq!(decode_payload(&exact).unwrap(), document);
}
