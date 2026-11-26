//! Host fixture: actual Android builders, public DER and disposable localhost TLS only.
#[path = "net.rs"]
mod net;

#[cfg(test)]
mod tests {
    use crate::net;
    use rustls_pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, UnixTime};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::Arc,
        time::{Duration, Instant},
    };

    const REMOVED: &[u8] = include_bytes!("tls-removed-emsign-g3.der");
    const DISTRUSTED: &[u8] = include_bytes!("tls-distrusted-spkis.der");
    const LOCALHOST: &[u8] = include_bytes!("tls-localhost-cert.der");

    #[test]
    fn every_bundled_root_converts_and_removed_emsign_g3_is_not_trusted() {
        let expected = webpki_root_certs::TLS_SERVER_ROOT_CERTS;
        assert!(expected.len() > 100);
        assert!(expected.iter().all(|cert| cert.as_ref() != REMOVED));
        for cert in expected {
            webpki::anchor_from_trusted_cert(cert).unwrap();
        }
        // Exercise the production conversion through both builders. The fixture
        // below exercises actual handshakes, not only successful construction.
        net::client_builder(reqwest::Client::builder())
            .build()
            .unwrap();
        net::blocking_client_builder(reqwest::blocking::Client::builder())
            .build()
            .unwrap();
    }

    // Tiny DER walker only locates an existing public certificate's SPKI. It
    // does not synthesize parser success or certificate-validation outcomes.
    fn tlv(bytes: &[u8], start: usize) -> (usize, usize) {
        let first = bytes[start + 1];
        let (length, header) = if first < 128 {
            (usize::from(first), 2)
        } else {
            let count = usize::from(first & 127);
            assert!(count > 0 && count <= 4);
            let length = bytes[start + 2..start + 2 + count]
                .iter()
                .fold(0usize, |n, b| n * 256 + usize::from(*b));
            (length, count + 2)
        };
        assert!(start + header + length <= bytes.len());
        (start + header, start + header + length)
    }

    #[test]
    fn public_verifier_rejects_all_eight_distrusted_spkis_before_issuer_search() {
        let (outer, _) = tlv(REMOVED, 0);
        let (mut field, _) = tlv(REMOVED, outer);
        // v3 version, serial, signature algorithm, issuer, validity, subject.
        for _ in 0..6 {
            field = tlv(REMOVED, field).1;
        }
        let (spki, end) = tlv(REMOVED, field);
        assert_eq!(end - spki, 118);
        assert_eq!(DISTRUSTED.len(), 8 * 118);
        for replacement in DISTRUSTED.chunks_exact(118) {
            let mut bytes = REMOVED.to_vec();
            bytes[spki..end].copy_from_slice(replacement);
            let cert = CertificateDer::from(bytes);
            let ee = webpki::EndEntityCert::try_from(&cert).unwrap();
            let result = ee.verify_for_usage(
                &[],
                &[],
                &[],
                UnixTime::since_unix_epoch(Duration::from_secs(1_800_000_000)),
                webpki::KeyUsage::server_auth(),
                None,
                None,
            );
            assert_eq!(format!("{:?}", result.err().unwrap()), "CertDistrusted");
        }
        let control = CertificateDer::from(REMOVED);
        let ee = webpki::EndEntityCert::try_from(&control).unwrap();
        let result = ee.verify_for_usage(
            &[],
            &[],
            &[],
            UnixTime::since_unix_epoch(Duration::from_secs(1_800_000_000)),
            webpki::KeyUsage::server_auth(),
            None,
            None,
        );
        assert_ne!(format!("{:?}", result.err().unwrap()), "CertDistrusted");
    }

    fn server() -> (String, std::thread::JoinHandle<std::io::Result<()>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("https://{}/", listener.local_addr().unwrap());
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(LOCALHOST.to_vec())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
                include_bytes!("tls-localhost-key.der").to_vec(),
            )),
        )
        .unwrap();
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(8);
            let socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => return Err(e),
                }
            };
            socket.set_nonblocking(false)?;
            socket.set_read_timeout(Some(Duration::from_secs(5)))?;
            socket.set_write_timeout(Some(Duration::from_secs(5)))?;
            let mut stream = rustls::StreamOwned::new(
                rustls::ServerConnection::new(Arc::new(config)).unwrap(),
                socket,
            );
            let mut request = [0u8; 2048];
            assert!(stream.read(&mut request)? > 0);
            stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
            )?;
            stream.flush()
        });
        (url, worker)
    }

    #[test]
    fn blocking_android_client_rejects_unknown_ca_and_explicit_fixture_trust_succeeds() {
        let (url, peer) = server();
        let client = net::blocking_client_builder(reqwest::blocking::Client::builder())
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let error = client.get(url).send().err().unwrap();
        assert!(format!("{error:?}").contains("UnknownIssuer"));
        assert!(peer.join().unwrap().is_err());
        let (url, peer) = server();
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .tls_certs_only([reqwest::Certificate::from_der(LOCALHOST).unwrap()])
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        assert_eq!(client.get(url).send().unwrap().text().unwrap(), "ok");
        peer.join().unwrap().unwrap();
    }

    #[tokio::test]
    async fn async_android_client_rejects_unknown_ca_and_explicit_fixture_trust_succeeds() {
        let (url, peer) = server();
        let client = net::client_builder(reqwest::Client::builder())
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let error = client.get(url).send().await.err().unwrap();
        assert!(format!("{error:?}").contains("UnknownIssuer"));
        assert!(peer.join().unwrap().is_err());
        let (url, peer) = server();
        let client = reqwest::Client::builder()
            .no_proxy()
            .tls_certs_only([reqwest::Certificate::from_der(LOCALHOST).unwrap()])
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        assert_eq!(
            client.get(url).send().await.unwrap().text().await.unwrap(),
            "ok"
        );
        peer.join().unwrap().unwrap();
    }
}
