//! Protocol-core tests extracted from `bridge::mod.rs`.
use super::*;
use crate::config::PasswordGeneratorSettings;
use crate::crypto::{hmac_sha256, random_bytes};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::collections::HashMap;

// ---------------------------------------------------------------------
// Crypto vectors
// ---------------------------------------------------------------------

/// NIST SP 800-38A CBC-AES-256 vector: encrypting the first plaintext
/// block must yield the reference ciphertext, plus a full PKCS7 padding
/// block (0x10 × 16) appended by the padded API.
#[test]
fn aes256_cbc_matches_nist_vector() {
    let key = hex_bytes("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
    let iv = hex_bytes("000102030405060708090a0b0c0d0e0f");
    let plaintext = hex_bytes("6bc1bee22e409f96e93d7e117393172a");
    let expected_ct = hex_bytes("f58c4c04d6e5f1ba779eabfb5f7bfbd6");

    let ciphertext = crate::crypto::aes_cbc_encrypt(&key, &iv, &plaintext);
    assert_eq!(&ciphertext[..16], expected_ct.as_slice());
    // PKCS7 appends one full padding block (0x10 × 16); the padded API
    // strips it again on decrypt, so the round trip is the plaintext.
    assert_eq!(ciphertext.len(), 32);
    let round_trip = crate::crypto::aes_cbc_decrypt(&key, &iv, &ciphertext).unwrap();
    assert_eq!(round_trip, plaintext);
}

#[test]
fn encrypt_decrypt_round_trips_utf8_and_empty() {
    let key = [7u8; 32];
    let iv = [9u8; 16];
    let encoded = encrypt_field(&key, &iv, "用户@示例.com/密码🔐");
    assert_eq!(
        decrypt_field(&key, &iv, &encoded).unwrap(),
        "用户@示例.com/密码🔐"
    );

    let empty = encrypt_field(&key, &iv, "");
    assert_eq!(decrypt_field(&key, &iv, &empty).unwrap(), "");
}

#[test]
fn decrypt_rejects_wrong_key_or_tampered_text() {
    let key = [1u8; 32];
    let iv = [2u8; 16];
    let encoded = encrypt_field(&key, &iv, "secret");
    assert!(decrypt_field(&[0u8; 32], &iv, &encoded).is_err());

    let mut tampered = STANDARD.decode(&encoded).unwrap();
    tampered[0] ^= 0xff;
    let tampered_b64 = STANDARD.encode(&tampered);
    assert!(decrypt_field(&key, &iv, &tampered_b64).is_err());
    assert!(decrypt_field(&key, &iv, "not-base64!!").is_err());
}

/// RFC 4231 HMAC-SHA256 test case 2.
#[test]
fn hmac_matches_rfc4231_vector() {
    let key = b"Jefe";
    let data = b"what do ya want for nothing?";
    let expected = hex_bytes("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
    assert_eq!(hmac_sha256(key, data), expected);
}

#[test]
fn verifier_round_trip_and_checks() {
    let key = [3u8; 32];
    let nonce = random_bytes(16);
    let verifier = make_verifier(&key, &nonce);
    let nonce_b64 = STANDARD.encode(&nonce);

    assert!(check_verifier(&key, &nonce_b64, &verifier));
    assert!(!check_verifier(&[4u8; 32], &nonce_b64, &verifier));
    assert!(!check_verifier(&key, &nonce_b64, "AAAA"));
    assert!(!check_verifier(&key, "AAAA", &verifier));
    assert!(!check_verifier(&key, &STANDARD.encode([0u8; 8]), &verifier));
}

#[test]
fn response_hmac_is_stable_and_changes_with_key() {
    let key = [5u8; 32];
    let other = [6u8; 32];
    let nonce_b64 = STANDARD.encode([7u8; 16]);
    let verifier_b64 = STANDARD.encode([8u8; 32]);
    let hmac = response_hmac(&key, &nonce_b64, &verifier_b64);
    assert_eq!(hmac, response_hmac(&key, &nonce_b64, &verifier_b64));
    assert_ne!(hmac, response_hmac(&other, &nonce_b64, &verifier_b64));
    // 32-byte HMAC-SHA256 digest
    assert_eq!(STANDARD.decode(&hmac).unwrap().len(), 32);
}

// ---------------------------------------------------------------------
// Dispatch with a mock host
// ---------------------------------------------------------------------

struct MockHost {
    open: bool,
    clients: HashMap<String, Vec<u8>>,
    logins: Vec<BridgeLogin>,
    created: Vec<(String, String, String)>,
    updated: Vec<(String, String, String, String)>,
}

impl MockHost {
    fn open() -> Self {
        let key = [0x11; 32];
        let mut clients = HashMap::new();
        clients.insert("client-1".to_owned(), key.to_vec());
        Self {
            open: true,
            clients,
            logins: vec![BridgeLogin {
                uuid: "uuid-1".to_owned(),
                name: "示例站点".to_owned(),
                login: "user1".to_owned(),
                password: "pw-1".to_owned(),
            }],
            created: Vec::new(),
            updated: Vec::new(),
        }
    }
}

impl BridgeHost for MockHost {
    fn is_open(&self) -> bool {
        self.open
    }
    fn client_key(&self, id: &str) -> Option<Vec<u8>> {
        self.clients.get(id).cloned()
    }
    fn register_client(&mut self, id: &str, key: Vec<u8>) {
        self.clients.insert(id.to_owned(), key);
    }
    fn list_clients(&self) -> Vec<String> {
        self.clients.keys().cloned().collect()
    }
    fn remove_client(&mut self, id: &str) -> bool {
        self.clients.remove(id).is_some()
    }
    fn logins_for(&self, url: &str, submit_url: Option<&str>) -> Vec<BridgeLogin> {
        if url.contains("example.com") || submit_url.is_some_and(|s| s.contains("example.com")) {
            self.logins.clone()
        } else {
            Vec::new()
        }
    }
    fn db_hash(&self) -> String {
        "abc123".to_owned()
    }
    fn set_login(
        &mut self,
        login: &str,
        password: &str,
        url: &str,
        uuid: Option<&str>,
    ) -> Result<(), String> {
        self.updated.push((
            uuid.unwrap_or_default().to_owned(),
            login.to_owned(),
            password.to_owned(),
            url.to_owned(),
        ));
        Ok(())
    }
    fn create_login(&mut self, login: &str, password: &str, url: &str) -> Result<(), String> {
        self.created
            .push((login.to_owned(), password.to_owned(), url.to_owned()));
        Ok(())
    }
}

fn field(value: &str, key: &[u8], iv: &[u8]) -> String {
    encrypt_field(key, iv, value)
}

fn authorized_request(request_type: &str) -> BridgeRequest {
    let key = [0x11; 32];
    let nonce = random_bytes(16);
    let nonce_b64 = STANDARD.encode(&nonce);
    let verifier = make_verifier(&key, &nonce);
    BridgeRequest {
        request_type: request_type.to_owned(),
        id: Some("client-1".to_owned()),
        nonce: nonce_b64,
        verifier: Some(verifier),
        ..Default::default()
    }
}

#[test]
fn locked_vault_answers_error_without_keys() {
    let mut host = MockHost::open();
    host.open = false;
    let response = handle_request(authorized_request("get-logins"), &mut host, |_| true);
    assert!(!response.success);
    assert_eq!(response.error.as_deref(), Some("数据库未打开或已锁定"));
    assert!(response.verifier.is_empty());
}

#[test]
fn unknown_client_is_rejected_before_dispatch() {
    let mut request = authorized_request("get-logins");
    request.id = Some("ghost".to_owned());
    let mut host = MockHost::open();
    let response = handle_request(request, &mut host, |_| true);
    assert!(!response.success);
    assert!(response.error.unwrap().contains("未授权"));
}

#[test]
fn tampered_verifier_is_rejected() {
    let mut request = authorized_request("get-logins");
    request.verifier = Some(STANDARD.encode([0u8; 32]));
    let mut host = MockHost::open();
    let response = handle_request(request, &mut host, |_| true);
    assert!(!response.success);
    assert!(response.error.unwrap().contains("校验失败"));
}

#[test]
fn unsupported_request_type_is_rejected() {
    let mut host = MockHost::open();
    let response = handle_request(authorized_request("delete-logins"), &mut host, |_| true);
    assert!(!response.success);
    assert!(response.error.unwrap().contains("不支持"));
}

#[test]
fn test_associate_round_trip() {
    let mut host = MockHost::open();
    let response = handle_request(authorized_request("test-associate"), &mut host, |_| true);
    assert!(response.success);
    assert_eq!(response.id.as_deref(), Some("client-1"));
    assert!(response.entries.is_empty());
    assert_eq!(response.hash, "abc123");
    // Response envelope decrypts with the client key.
    let key = [0x11; 32];
    let iv = STANDARD.decode(&response.nonce).unwrap();
    assert_eq!(
        decrypt_field(&key, &iv, &response.verifier).unwrap(),
        STANDARD.encode(&iv)
    );
}

#[test]
fn get_logins_returns_encrypted_entries_and_hmac() {
    let key = [0x11; 32];
    let mut request = authorized_request("get-logins");
    request.url = Some(field(
        "https://example.com/login",
        &key,
        &STANDARD.decode(&request.nonce).unwrap(),
    ));
    let mut host = MockHost::open();
    let response = handle_request(request, &mut host, |_| true);
    assert!(response.success);
    assert_eq!(response.entries.len(), 1);

    let iv = STANDARD.decode(&response.nonce).unwrap();
    let entry = &response.entries[0];
    assert_eq!(decrypt_field(&key, &iv, &entry.login).unwrap(), "user1");
    assert_eq!(decrypt_field(&key, &iv, &entry.password).unwrap(), "pw-1");
    assert_eq!(decrypt_field(&key, &iv, &entry.name).unwrap(), "示例站点");
    assert_eq!(decrypt_field(&key, &iv, &entry.uuid).unwrap(), "uuid-1");

    // Verifier and hmac verify against the fresh response nonce.
    let expected_verifier_plain = STANDARD.encode(&iv);
    assert_eq!(
        decrypt_field(&key, &iv, &response.verifier).unwrap(),
        expected_verifier_plain
    );
    assert_eq!(
        response.hmac,
        response_hmac(&key, &response.nonce, &response.verifier)
    );
}

#[test]
fn get_logins_count_reports_count() {
    let key = [0x11; 32];
    let mut request = authorized_request("get-logins-count");
    request.url = Some(field(
        "https://example.com/login",
        &key,
        &STANDARD.decode(&request.nonce).unwrap(),
    ));
    let mut host = MockHost::open();
    let response = handle_request(request, &mut host, |_| true);
    assert!(response.success);
    assert_eq!(response.count, Some(1));
    assert_eq!(response.entries.len(), 1);
}

#[test]
fn get_logins_with_no_match_returns_empty_entries() {
    let key = [0x11; 32];
    let mut request = authorized_request("get-logins");
    request.url = Some(field(
        "https://elsewhere.io/",
        &key,
        &STANDARD.decode(&request.nonce).unwrap(),
    ));
    let mut host = MockHost::open();
    let response = handle_request(request, &mut host, |_| true);
    assert!(response.success);
    assert!(response.entries.is_empty());
    assert_eq!(response.count, None);
}

#[test]
fn set_login_creates_when_no_uuid_and_updates_with_uuid() {
    let key = [0x11; 32];

    let mut request = authorized_request("set-login");
    let iv = STANDARD.decode(&request.nonce).unwrap();
    request.login = Some(field("new-user", &key, &iv));
    request.password = Some(field("new-pw", &key, &iv));
    request.url = Some(field("https://example.com", &key, &iv));
    let mut host = MockHost::open();
    let response = handle_request(request, &mut host, |_| true);
    assert!(response.success);
    assert_eq!(host.created.len(), 1);
    assert_eq!(
        host.created[0],
        (
            "new-user".to_owned(),
            "new-pw".to_owned(),
            "https://example.com".to_owned()
        )
    );

    let mut request = authorized_request("set-login");
    let iv = STANDARD.decode(&request.nonce).unwrap();
    request.uuid = Some(field("uuid-1", &key, &iv));
    request.login = Some(field("old-user", &key, &iv));
    request.password = Some(field("old-pw", &key, &iv));
    request.url = Some(field("https://example.com", &key, &iv));
    let response = handle_request(request, &mut host, |_| true);
    assert!(response.success);
    assert_eq!(host.updated.len(), 1);
    assert_eq!(
        host.updated[0],
        (
            "uuid-1".to_owned(),
            "old-user".to_owned(),
            "old-pw".to_owned(),
            "https://example.com".to_owned()
        )
    );
}

#[test]
fn associate_approves_and_registers_new_client() {
    let key = [0x22; 32];
    let nonce = random_bytes(16);
    let nonce_b64 = STANDARD.encode(&nonce);
    let request = BridgeRequest {
        request_type: "associate".to_owned(),
        id: Some("browser-chrome".to_owned()),
        nonce: nonce_b64.clone(),
        verifier: Some(make_verifier(&key, &nonce)),
        key: Some(STANDARD.encode(key)),
        ..Default::default()
    };
    let mut host = MockHost::open();
    let approved = std::cell::Cell::new(false);
    let response = handle_request(request, &mut host, |id| {
        assert_eq!(id, "browser-chrome");
        approved.set(true);
        true
    });
    assert!(approved.get());
    assert!(response.success);
    assert_eq!(response.id.as_deref(), Some("browser-chrome"));
    assert!(host.clients.contains_key("browser-chrome"));

    // The new client is immediately usable.
    let nonce = random_bytes(16);
    let nonce_b64 = STANDARD.encode(&nonce);
    let request = BridgeRequest {
        request_type: "test-associate".to_owned(),
        id: Some("browser-chrome".to_owned()),
        nonce: nonce_b64.clone(),
        verifier: Some(make_verifier(&key, &nonce)),
        ..Default::default()
    };
    let response = handle_request(request, &mut host, |_| true);
    assert!(response.success);
    assert_eq!(response.id.as_deref(), Some("browser-chrome"));
}

#[test]
fn associate_rejects_bad_key_or_rejection() {
    let mut host = MockHost::open();

    // Key of the wrong length is refused before the approval prompt.
    let nonce = random_bytes(16);
    let nonce_b64 = STANDARD.encode(&nonce);
    let bad_key_request = BridgeRequest {
        request_type: "associate".to_owned(),
        nonce: nonce_b64,
        verifier: None,
        key: Some(STANDARD.encode([1u8; 16])),
        ..Default::default()
    };
    let response = handle_request(bad_key_request, &mut host, |_| true);
    assert!(!response.success);
    assert!(response.error.unwrap().contains("256"));

    // User rejection cancels the association.
    let key = [0x33; 32];
    let nonce = random_bytes(16);
    let nonce_b64 = STANDARD.encode(&nonce);
    let request = BridgeRequest {
        request_type: "associate".to_owned(),
        id: Some("denied-client".to_owned()),
        nonce: nonce_b64,
        verifier: Some(make_verifier(&key, &nonce)),
        key: Some(STANDARD.encode(key)),
        ..Default::default()
    };
    let response = handle_request(request, &mut host, |_| false);
    assert!(!response.success);
    assert!(!host.clients.contains_key("denied-client"));
    assert!(response.error.unwrap().contains("拒绝"));
}

#[test]
fn associate_fails_when_locked() {
    let mut host = MockHost::open();
    host.open = false;
    let nonce = random_bytes(16);
    let nonce_b64 = STANDARD.encode(&nonce);
    let request = BridgeRequest {
        request_type: "associate".to_owned(),
        nonce: nonce_b64,
        verifier: None,
        key: Some(STANDARD.encode([1u8; 32])),
        ..Default::default()
    };
    let response = handle_request(request, &mut host, |_| true);
    assert!(!response.success);
    assert_eq!(response.error.as_deref(), Some("数据库未打开或已锁定"));
}

#[test]
fn generate_password_meets_default_policy_and_is_fresh() {
    let mut host = MockHost::open();
    let response = handle_request(authorized_request("generate-password"), &mut host, |_| true);
    assert!(response.success);
    let password = response
        .password
        .expect("generate-password returns a password");
    assert_eq!(password.len(), 20);
    assert!(password.chars().any(|c| c.is_ascii_uppercase()));
    assert!(password.chars().any(|c| c.is_ascii_lowercase()));
    assert!(password.chars().any(|c| c.is_ascii_digit()));
    assert!(password.chars().any(|c| !c.is_ascii_alphanumeric()));

    let again = handle_request(authorized_request("generate-password"), &mut host, |_| true);
    assert_ne!(password, again.password.expect("fresh password"));
}

#[test]
fn generate_password_honors_configured_generator() {
    let settings = PasswordGeneratorSettings {
        length: 10,
        include_upper: true,
        include_lower: true,
        include_digits: true,
        include_symbols: true,
        exclude_similar: false,
        exclude_ambiguous: false,
        custom_charset: Some("ABC123".into()),
        exclude_chars: None,
        required_chars: Some("A3".into()),
        pattern: None,
        ..Default::default()
    };
    let mut host = MockHost::open();
    let response = handle_request_with_generator(
        authorized_request("generate-password"),
        &mut host,
        |_| true,
        &settings,
    );
    let password = response.password.expect("password returned");
    assert_eq!(password.chars().count(), 10);
    assert!(password.chars().all(|c| "ABC123".contains(c)));
    assert!(password.contains('A') && password.contains('3'));
}

fn hex_bytes(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn generator_with_custom_charset_and_required() {
    let settings = PasswordGeneratorSettings {
        length: 8,
        include_upper: true,
        include_lower: true,
        include_digits: true,
        include_symbols: true,
        exclude_similar: false,
        exclude_ambiguous: false,
        custom_charset: Some("ABC123".into()),
        exclude_chars: None,
        required_chars: Some("A3".into()),
        pattern: None,
        ..Default::default()
    };
    for _ in 0..20 {
        let password = generate_password_with(&settings).unwrap();
        assert_eq!(password.chars().count(), 8);
        assert!(password.chars().all(|c| "ABC123".contains(c)));
        assert!(password.contains('A') && password.contains('3'));
    }
}

#[test]
fn generator_with_exclusions_and_pattern() {
    let settings = PasswordGeneratorSettings {
        length: 4,
        include_upper: true,
        include_lower: true,
        include_digits: true,
        include_symbols: true,
        exclude_similar: false,
        exclude_ambiguous: false,
        custom_charset: None,
        exclude_chars: Some("1".into()),
        required_chars: None,
        pattern: Some("udlL".into()),
        ..Default::default()
    };
    for _ in 0..20 {
        let password = generate_password_with(&settings).unwrap();
        let chars: Vec<char> = password.chars().collect();
        assert!("ABCDEFGHIJKLMNOPQRSTUVWXYZ".contains(chars[0]));
        assert!("0123456789".contains(chars[1]));
        assert!("abcdefghijklmnopqrstuvwxyz".contains(chars[2]));
        assert_eq!(chars[3], 'L');
        assert!(!password.contains('1'));
    }
}

#[test]
fn generator_rejects_required_outside_pool() {
    let settings = PasswordGeneratorSettings {
        length: 8,
        include_upper: true,
        include_lower: true,
        include_digits: true,
        include_symbols: true,
        exclude_similar: false,
        exclude_ambiguous: false,
        custom_charset: Some("ABC".into()),
        exclude_chars: None,
        required_chars: Some("X".into()),
        pattern: None,
        ..Default::default()
    };
    assert!(generate_password_with(&settings).is_err());
}
