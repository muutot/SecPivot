use super::*;
use zeroize::Zeroizing;

#[test]
fn exchange_inflate_over_limit_rejects_without_returning_a_prefix() {
    let mut accepted = Vec::new();
    for limit in [0, 1, 32, 4096] {
        let source = Zeroizing::new(vec![b'x'; limit + 1]);
        let framed = Zeroizing::new(encode_payload(&source, false).unwrap());
        match gunzip_limited(&framed[4..], limit) {
            Err(error) => assert_eq!(error, "剪贴板数据过大"),
            Ok(bytes) => accepted.push((limit, bytes.len())),
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted/truncated limit cases: {accepted:?}"
    );
}

#[test]
fn exchange_inflate_exact_limit_checks_checksum_size_and_truncated_footer() {
    let source = b"synthetic checksum control";
    let framed = encode_payload(source, false).unwrap();
    let gzip = &framed[4..];
    let mut bad_crc = gzip.to_vec();
    let mut bad_size = gzip.to_vec();
    let mut truncated = gzip.to_vec();
    let length = gzip.len();
    bad_crc[length - 8] ^= 1;
    bad_size[length - 4] ^= 1;
    truncated.pop();
    let mut accepted = Vec::new();
    for (index, body) in [bad_crc, bad_size, truncated].into_iter().enumerate() {
        // These small invalid streams already reject through the production
        // 64 MiB wrapper; the exact output boundary must enforce the same check.
        let mut payload = vec![0, 0, 0, 0];
        payload.extend_from_slice(&body);
        assert!(decode_payload(&payload).is_err());
        match gunzip_limited(&body, source.len()) {
            Err(error) => assert_eq!(error, "剪贴板数据已损坏"),
            Ok(_) => accepted.push(index),
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted corrupt footer cases: {accepted:?}"
    );
}

#[test]
fn exchange_inflate_valid_empty_short_and_exact_boundaries_keep_bytes() {
    for (length, limit) in [
        (0, 0),
        (1, 1),
        (31, 32),
        (32, 32),
        (4096, 4096),
        (0, usize::MAX),
    ] {
        let source = Zeroizing::new(vec![b'x'; length]);
        let framed = Zeroizing::new(encode_payload(&source, false).unwrap());
        assert_eq!(gunzip_limited(&framed[4..], limit).unwrap(), *source);
        assert_eq!(decode_payload(&framed).unwrap(), *source);
    }
}

#[test]
fn exchange_inflate_over_limit_xml_cannot_publish_a_parseable_prefix() {
    let document = "<KeePassFile><Root><Group><UUID>AAAAAAAAAAAAAAAAAAAAAA==</UUID><Entry><UUID>AAAAAAAAAAAAAAAAAAAAAA==</UUID><String><Key>Title</Key><Value>Synthetic</Value></String></Entry></Group></Root></KeePassFile>";
    let source = Zeroizing::new(format!("{document}!"));
    let framed = Zeroizing::new(encode_payload(source.as_bytes(), false).unwrap());
    match gunzip_limited(&framed[4..], document.len()) {
        Err(error) => assert_eq!(error, "剪贴板数据过大"),
        Ok(prefix) => {
            let parseable_entries = parse_entries_document(&prefix).map(|entries| entries.len());
            panic!("oversize stream returned prefix; parseable entries: {parseable_entries:?}");
        }
    }
}
