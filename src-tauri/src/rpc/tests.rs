//! Protocol-core tests extracted from `rpc::mod.rs`.
use super::*;
use crate::crypto::{random_bytes, random_hex, sha256_hex, KEY_LEN};
use crate::util::url_host;
use num_bigint::BigUint;
use serde_json::{json, Value};
use std::collections::HashMap;
fn big(n_hex: &str) -> BigUint {
    BigUint::parse_bytes(n_hex.as_bytes(), 16).unwrap()
}

/// Mirrors kprpcClient.js `SRPc.calculations` + `key()` exactly, so the
/// server-side math is cross-checked against the extension's algorithm.
/// `a` is the client's private ephemeral; `A = g^a mod N` is derived here,
/// exactly like `SRPc` does (`this.A = modPow(this.g, this.a, this.N)`).
fn js_client_handshake(
    salt_hex: &str,
    b_hex: &str,
    password: &str,
    a: &BigUint,
) -> (String, String, String) {
    let n = group();
    let g = BigUint::from(SRP_G);
    let k = big(SRP_K_HEX);
    let a_public = mod_pow(&g, a, &n);
    let a_hex = a_public.to_str_radix(16).to_uppercase();
    let b = big(b_hex);
    let u = big(&sha256_hex(&format!("{a_hex}{b_hex}")));
    let x = big(&sha256_hex(&format!("{salt_hex}{password}")));
    let kgx = (&k * mod_pow(&g, &x, &n)) % &n;
    let aux = a + &u * &x;
    let s = mod_pow(&((&b + &n - &kgx) % &n), &aux, &n);
    let s_upper = s.to_str_radix(16).to_uppercase();
    let m = sha256_hex(&format!("{a_hex}{b_hex}{s_upper}"));
    let m2 = sha256_hex(&format!("{a_hex}{m}{s_upper}"));
    let k_hex = sha256_hex(&s_upper);
    (m, m2, k_hex)
}

#[test]
fn srp_round_trip_with_js_style_client() {
    let password = "yk4q-9Kz2!";
    let (mut server, payload) = SrpServer::begin(password);
    let s = payload["s"].as_str().unwrap().to_owned();
    let b = payload["B"].as_str().unwrap().to_owned();
    assert_eq!(payload["stage"], "identifyToClient");
    assert_eq!(payload["securityLevel"], 3);
    assert!(b
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));

    let a = BigUint::from_bytes_be(&random_bytes(32)) % &group();
    let a_public = mod_pow(&BigUint::from(SRP_G), &a, &group());
    let a_hex = a_public.to_str_radix(16).to_uppercase();
    let (m, m2_expected, k_expected) = js_client_handshake(&s, &b, password, &a);

    let m2 = server.verify_proof(&a_hex, &m).expect("proof must verify");
    assert_eq!(m2, m2_expected);
    assert_eq!(server.secret_key().unwrap(), k_expected);
    assert_eq!(secret_bytes(&k_expected).unwrap().len(), KEY_LEN);
}

#[test]
fn srp_rejects_wrong_password_or_tampered_proof() {
    let (mut server, payload) = SrpServer::begin("correct-pw");
    let s = payload["s"].as_str().unwrap().to_owned();
    let b = payload["B"].as_str().unwrap().to_owned();
    let a = BigUint::from_bytes_be(&random_bytes(32)) % &group();
    let a_public = mod_pow(&BigUint::from(SRP_G), &a, &group());
    let a_hex = a_public.to_str_radix(16).to_uppercase();

    let (m, _, _) = js_client_handshake(&s, &b, "wrong-pw", &a);
    assert_eq!(server.verify_proof(&a_hex, &m), Err(RpcError::AuthFailed));

    let (mut server2, payload2) = SrpServer::begin("correct-pw");
    let s2 = payload2["s"].as_str().unwrap().to_owned();
    let b2 = payload2["B"].as_str().unwrap().to_owned();
    let (m2, _, _) = js_client_handshake(&s2, &b2, "correct-pw", &a);
    let tampered = format!("{}{}", if &m2[0..1] == "a" { "b" } else { "a" }, &m2[1..]);
    assert_eq!(
        server2.verify_proof(&a_hex, &tampered),
        Err(RpcError::AuthFailed)
    );
}

#[test]
fn secret_key_requires_finished_handshake() {
    let (server, _) = SrpServer::begin("pw");
    assert_eq!(
        server.secret_key(),
        Err(RpcError::InvalidMessage("SRP 尚未完成".to_owned()))
    );
}

#[test]
fn key_auth_challenge_response_matches_expected_hash() {
    let secret = random_hex(32);
    let sc = random_hex(32);
    let cc = random_hex(32);
    let cr = key_auth_cr(&secret, &sc, &cc);
    let sr = key_auth_sr(&secret, &sc, &cc);
    assert_eq!(cr, sha256_hex(&format!("1{secret}{sc}{cc}")));
    assert_eq!(sr, sha256_hex(&format!("0{secret}{sc}{cc}")));
    assert_ne!(cr, sr);
    let other = key_auth_cr(&random_hex(32), &sc, &cc);
    assert_ne!(cr, other);
}

#[test]
fn frame_round_trip_and_tamper_rejection() {
    let secret = secret_bytes(&random_hex(32)).unwrap();
    let frame = encrypt_frame(&secret, r#"{"jsonrpc":"2.0","id":7}"#);
    let plaintext = decrypt_frame(&secret, &frame).unwrap();
    assert_eq!(plaintext, r#"{"jsonrpc":"2.0","id":7}"#);

    let mut tampered = frame.clone();
    tampered.message.push('=');
    assert_eq!(
        decrypt_frame(&secret, &tampered),
        Err(RpcError::InvalidMessage("密文格式无效".to_owned()))
    );

    let mut flipped = frame.clone();
    let mid = flipped.message.len() / 2;
    let ch = flipped.message.as_bytes()[mid];
    flipped
        .message
        .replace_range(mid..mid + 1, if ch == b'A' { "B" } else { "A" });
    assert_eq!(decrypt_frame(&secret, &flipped), Err(RpcError::AuthFailed));

    let other_secret = secret_bytes(&random_hex(32)).unwrap();
    assert_eq!(
        decrypt_frame(&other_secret, &frame),
        Err(RpcError::AuthFailed)
    );
}

struct MockHost {
    open: bool,
    keys: HashMap<String, Vec<u8>>,
    db: RpcDatabase,
    next_id: u32,
}

impl RpcHost for MockHost {
    fn is_open(&self) -> bool {
        self.open
    }
    fn rpc_key(&self, username: &str) -> Option<Vec<u8>> {
        self.keys.get(username).cloned()
    }
    fn register_rpc_key(&mut self, username: &str, key: Vec<u8>) {
        self.keys.insert(username.to_owned(), key);
    }
    fn database(&self) -> Option<RpcDatabase> {
        self.open.then(|| self.db.clone())
    }
    fn find_logins(
        &self,
        urls: &[String],
        uuid: Option<&str>,
        free_text: Option<&str>,
        username: Option<&str>,
    ) -> Vec<RpcLogin> {
        if !self.open {
            return Vec::new();
        }
        self.db
            .root
            .entries
            .iter()
            .filter(|e| {
                let by_url = urls.iter().any(|u| {
                    let u_host = url_host(u);
                    e.urls.iter().any(|eu| u == eu || url_host(eu) == u_host)
                });
                let by_uuid = uuid.is_some_and(|id| id == e.uuid);
                let by_text =
                    free_text.is_some_and(|t| e.title.contains(t) || e.username.contains(t));
                let by_username = username.is_some_and(|u| !u.is_empty() && e.username.contains(u));
                by_url || by_uuid || by_text || by_username
            })
            .cloned()
            .collect()
    }
    fn add_login(
        &mut self,
        login: &RpcLoginWrite,
        parent_uuid: &str,
    ) -> Result<RpcLogin, RpcError> {
        if !self.open {
            return Err(RpcError::Locked);
        }
        let parent = if parent_uuid == "g-1" {
            RpcGroupRef {
                uuid: "g-1".to_owned(),
                title: "Internet".to_owned(),
                path: "/Internet".to_owned(),
                icon_image_data: String::new(),
            }
        } else {
            RpcGroupRef {
                uuid: self.db.root.uuid.clone(),
                title: self.db.root.title.clone(),
                path: String::new(),
                icon_image_data: String::new(),
            }
        };
        let created = RpcLogin {
            uuid: format!("e-{}", self.next_id),
            title: login.title.clone(),
            username: write_username(login),
            password: write_password(login),
            urls: login.urls.clone(),
            http_realm: login.http_realm.clone(),
            icon_image_data: login.icon_image_data.clone(),
            parent_group: parent,
            match_accuracy: 1,
        };
        self.next_id += 1;
        self.db.root.entries.push(created.clone());
        Ok(created)
    }
    fn update_login(
        &mut self,
        login: &RpcLoginWrite,
        old_uuid: &str,
        url_merge_mode: u8,
    ) -> Result<RpcLogin, RpcError> {
        if !self.open {
            return Err(RpcError::Locked);
        }
        let pos = self
            .db
            .root
            .entries
            .iter()
            .position(|e| e.uuid == old_uuid)
            .ok_or(RpcError::EntryNotFound)?;
        let old = &self.db.root.entries[pos];
        let updated = RpcLogin {
            uuid: old.uuid.clone(),
            title: login.title.clone(),
            username: write_username(login),
            password: write_password(login),
            urls: merge_urls(&old.urls, &login.urls, url_merge_mode),
            http_realm: login.http_realm.clone(),
            icon_image_data: login.icon_image_data.clone(),
            parent_group: old.parent_group.clone(),
            match_accuracy: 1,
        };
        self.db.root.entries[pos] = updated.clone();
        Ok(updated)
    }
}

fn mock_host() -> MockHost {
    let parent = RpcGroupRef {
        uuid: "g-1".to_owned(),
        title: "Internet".to_owned(),
        path: "/Internet".to_owned(),
        icon_image_data: String::new(),
    };
    let login = RpcLogin {
        uuid: "e-1".to_owned(),
        title: "Example".to_owned(),
        username: "alice".to_owned(),
        password: "s3cret".to_owned(),
        urls: vec!["https://example.com/login".to_owned()],
        http_realm: String::new(),
        icon_image_data: String::new(),
        parent_group: parent.clone(),
        match_accuracy: 3,
    };
    let root = RpcGroup {
        uuid: "g-root".to_owned(),
        title: "Root".to_owned(),
        path: String::new(),
        icon_image_data: String::new(),
        entries: vec![login],
        children: Vec::new(),
    };
    let db = RpcDatabase {
        name: "My Vault".to_owned(),
        file_name: "vault.kdbx".to_owned(),
        icon_image_data: String::new(),
        root,
        active: true,
    };
    MockHost {
        open: true,
        keys: HashMap::new(),
        db,
        next_id: 2,
    }
}

#[test]
fn get_all_databases_returns_dto_tree() {
    let mut host = mock_host();
    let result = handle_jsonrpc(&mut host, "GetAllDatabases", None).unwrap();
    let dbs = result.as_array().unwrap();
    assert_eq!(dbs.len(), 1);
    let dto = &dbs[0];
    assert_eq!(dto["fileName"], "vault.kdbx");
    assert_eq!(dto["active"], true);
    assert_eq!(dto["root"]["title"], "Root");
    assert_eq!(
        dto["root"]["childLightEntries"][0]["usernameValue"],
        "alice"
    );
    assert_eq!(dto["root"]["childLightEntries"][0]["uniqueID"], "e-1");
    assert_eq!(
        dto["root"]["childLightEntries"][0]["uRLs"][0],
        "https://example.com/login"
    );
}

#[test]
fn find_logins_matches_url_uuid_and_text() {
    let mut host = mock_host();

    let params = json!([
        ["https://example.com/dashboard"],
        null,
        null,
        "LSTnoForms",
        false,
        null,
        "",
        null,
        null
    ]);
    let result = handle_jsonrpc(&mut host, "FindLogins", Some(&params)).unwrap();
    let entries = result.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["uniqueID"], "e-1");
    assert_eq!(entries[0]["formFieldList"][0]["type"], "FFTusername");
    assert_eq!(entries[0]["formFieldList"][0]["value"], "alice");
    assert_eq!(entries[0]["formFieldList"][1]["value"], "s3cret");
    assert_eq!(entries[0]["db"]["fileName"], "vault.kdbx");
    assert_eq!(entries[0]["parent"]["path"], "/Internet");

    let params = json!([
        ["https://other.example/x"],
        null,
        null,
        "LSTnoForms",
        false,
        "e-1",
        "",
        null,
        null
    ]);
    let result = handle_jsonrpc(&mut host, "FindLogins", Some(&params)).unwrap();
    assert_eq!(result.as_array().unwrap().len(), 1);

    let params = json!([[], null, null, "LSTnoForms", false, null, "", "Examp", null]);
    let result = handle_jsonrpc(&mut host, "FindLogins", Some(&params)).unwrap();
    assert_eq!(result.as_array().unwrap().len(), 1);

    let params = json!([[], null, null, "LSTnoForms", false, null, "", null, "bob"]);
    let result = handle_jsonrpc(&mut host, "FindLogins", Some(&params)).unwrap();
    assert_eq!(result.as_array().unwrap().len(), 0);
}

#[test]
fn password_profiles_and_generation() {
    let mut host = mock_host();
    let result = handle_jsonrpc(&mut host, "GetPasswordProfiles", None).unwrap();
    assert_eq!(result, json!(["Default"]));
    let result =
        handle_jsonrpc(&mut host, "GeneratePassword", Some(&json!(["Default", ""]))).unwrap();
    let pw = result.as_str().unwrap();
    assert_eq!(pw.len(), 20);
    assert!(pw.chars().any(|c| c.is_ascii_uppercase()));
    assert!(pw.chars().any(|c| c.is_ascii_lowercase()));
    assert!(pw.chars().any(|c| c.is_ascii_digit()));
    assert!(pw.chars().any(|c| !c.is_ascii_alphanumeric()));
}

#[test]
fn configured_password_generation_errors_reach_rpc_clients() {
    let mut host = mock_host();
    let settings = crate::config::PasswordGeneratorSettings {
        include_upper: false,
        include_lower: false,
        include_digits: false,
        include_symbols: false,
        ..Default::default()
    };
    assert_eq!(
        handle_jsonrpc_with_generator(
            &mut host,
            "GeneratePassword",
            Some(&json!(["Default", ""])),
            &settings,
        ),
        Err(RpcError::InvalidMessage("密码生成字符池为空".to_owned()))
    );
}

#[test]
fn locked_host_answers_error_and_unsupported_method_errors() {
    let mut host = mock_host();
    host.open = false;
    assert_eq!(
        handle_jsonrpc(&mut host, "GetAllDatabases", None),
        Err(RpcError::Locked)
    );
    assert_eq!(
        handle_jsonrpc(&mut host, "AddLogin", None),
        Err(RpcError::Locked)
    );
    assert_eq!(
        handle_jsonrpc(&mut host, "UpdateLogin", None),
        Err(RpcError::Locked)
    );
    host.open = true;
    assert_eq!(
        handle_jsonrpc(&mut host, "AddGroup", None),
        Err(RpcError::Unsupported("AddGroup".to_owned()))
    );
}

fn login_write(title: &str, username: &str, password: &str, urls: &[&str]) -> Value {
    json!({
        "title": title,
        "uRLs": urls,
        "hTTPRealm": "",
        "iconImageData": "",
        "formFieldList": [
            { "displayName": "KeePass username", "id": "u", "name": "user", "type": "FFTusername", "value": username, "page": 0 },
            { "displayName": "KeePass password", "id": "p", "name": "pass", "type": "FFTpassword", "value": password, "page": 0 },
            { "displayName": "Custom note", "id": "n", "name": "note", "type": "FFTtext", "value": "hello", "page": 0 },
        ],
    })
}

#[test]
fn url_merge_modes_match_keepassrpc_semantics() {
    let old = vec![
        "https://old.example.com".to_owned(),
        "https://alt.example.com".to_owned(),
    ];
    let src = vec![
        "https://new.example.com".to_owned(),
        "https://alt.example.com".to_owned(),
    ];
    // 1: source walked backwards, missing URLs inserted at front; the
    // source primary is promoted when already present.
    assert_eq!(
        merge_urls(&old, &src, 1),
        vec![
            "https://new.example.com",
            "https://old.example.com",
            "https://alt.example.com"
        ]
    );
    // 2: old primary removed first, then merged.
    assert_eq!(
        merge_urls(&old, &src, 2),
        vec!["https://new.example.com", "https://alt.example.com"]
    );
    // 3: keep old, append only new ones.
    assert_eq!(
        merge_urls(&old, &src, 3),
        vec![
            "https://old.example.com",
            "https://alt.example.com",
            "https://new.example.com"
        ]
    );
    // 4: unchanged.
    assert_eq!(merge_urls(&old, &src, 4), old);
    // 5: whole-list replace.
    assert_eq!(merge_urls(&old, &src, 5), src);
    // Unknown modes behave like 4 (plugin switch has no default).
    assert_eq!(merge_urls(&old, &src, 0), old);

    // Source primary promotion: already present but not first.
    let promoted = merge_urls(
        &[
            "https://alt.example.com".to_owned(),
            "https://new.example.com".to_owned(),
        ],
        &["https://new.example.com".to_owned()],
        1,
    );
    assert_eq!(
        promoted,
        vec!["https://new.example.com", "https://alt.example.com"]
    );

    // Mode 2 with an empty source leaves no URL (old primary deleted).
    assert_eq!(
        merge_urls(&old, &Vec::<String>::new(), 2),
        vec!["https://alt.example.com"]
    );
}

#[test]
fn add_login_creates_entry_and_returns_dto() {
    let mut host = mock_host();
    let params = json!([
        login_write(
            "New Site",
            "bob",
            "pw-1",
            &["https://new.example.com/login"]
        ),
        "g-1",
        "vault.kdbx",
    ]);
    let result = handle_jsonrpc(&mut host, "AddLogin", Some(&params)).unwrap();
    assert_eq!(result["uniqueID"], "e-2");
    assert_eq!(result["title"], "New Site");
    assert_eq!(result["uRLs"][0], "https://new.example.com/login");
    assert_eq!(result["formFieldList"][0]["type"], "FFTusername");
    assert_eq!(result["formFieldList"][0]["value"], "bob");
    assert_eq!(result["formFieldList"][1]["value"], "pw-1");
    assert_eq!(result["parent"]["path"], "/Internet");
    assert_eq!(result["db"]["fileName"], "vault.kdbx");

    // The new entry is visible to subsequent reads.
    let params = json!([
        [],
        null,
        null,
        "LSTnoForms",
        false,
        null,
        "",
        "New Site",
        null
    ]);
    let result = handle_jsonrpc(&mut host, "FindLogins", Some(&params)).unwrap();
    let entries = result.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["uniqueID"], "e-2");
    assert_eq!(entries[0]["formFieldList"][0]["value"], "bob");
}

#[test]
fn add_login_without_parent_uses_root_group() {
    let mut host = mock_host();
    let params = json!([
        login_write("Rooted", "u", "p", &["https://root.example.com"]),
        null,
        "vault.kdbx",
    ]);
    let result = handle_jsonrpc(&mut host, "AddLogin", Some(&params)).unwrap();
    assert_eq!(result["parent"]["uniqueID"], "g-root");
    assert_eq!(result["parent"]["path"], "");

    // Unknown parent uuid also falls back to root.
    let params = json!([
        login_write("Rooted 2", "u", "p", &["https://root2.example.com"]),
        "does-not-exist",
        "vault.kdbx",
    ]);
    let result = handle_jsonrpc(&mut host, "AddLogin", Some(&params)).unwrap();
    assert_eq!(result["parent"]["uniqueID"], "g-root");
}

#[test]
fn add_login_with_missing_login_errors() {
    let mut host = mock_host();
    let params = json!([null, "g-1", "vault.kdbx"]);
    assert!(matches!(
        handle_jsonrpc(&mut host, "AddLogin", Some(&params)),
        Err(RpcError::InvalidMessage(_))
    ));
}

#[test]
fn update_login_replaces_or_merges_urls() {
    let mut host = mock_host();
    // Mode 5 (Kee sends this when KPRPC_FEATURE_ENTRY_URL_REPLACEMENT is
    // offered): whole-list replace.
    let params = json!([
        login_write(
            "Example",
            "alice",
            "s3cret",
            &["https://only-new.example.com"]
        ),
        "e-1",
        5,
        "vault.kdbx",
    ]);
    let result = handle_jsonrpc(&mut host, "UpdateLogin", Some(&params)).unwrap();
    assert_eq!(result["uniqueID"], "e-1");
    assert_eq!(result["uRLs"], json!(["https://only-new.example.com"]));
    assert_eq!(result["formFieldList"][0]["value"], "alice");

    // Mode 1: old URL kept, new one promoted to primary.
    let params = json!([
        login_write(
            "Example",
            "alice",
            "s3cret",
            &["https://second.example.com"]
        ),
        "e-1",
        1,
        "vault.kdbx",
    ]);
    let result = handle_jsonrpc(&mut host, "UpdateLogin", Some(&params)).unwrap();
    assert_eq!(
        result["uRLs"],
        json!(["https://second.example.com", "https://only-new.example.com",])
    );
}

#[test]
fn update_login_validates_params_and_unknown_uuid() {
    let mut host = mock_host();
    // Empty oldLoginUUID �?error (plugin ArgumentException mirror).
    let params = json!([login_write("X", "u", "p", &[]), "", 5, "vault.kdbx"]);
    assert!(matches!(
        handle_jsonrpc(&mut host, "UpdateLogin", Some(&params)),
        Err(RpcError::InvalidMessage(_))
    ));
    // Empty dbFileName �?error (plugin ArgumentException mirror).
    let params = json!([login_write("X", "u", "p", &[]), "e-1", 5, ""]);
    assert!(matches!(
        handle_jsonrpc(&mut host, "UpdateLogin", Some(&params)),
        Err(RpcError::InvalidMessage(_))
    ));
    // Unknown uuid �?EntryNotFound.
    let params = json!([
        login_write("X", "u", "p", &["https://x.example.com"]),
        "e-999",
        5,
        "vault.kdbx",
    ]);
    assert_eq!(
        handle_jsonrpc(&mut host, "UpdateLogin", Some(&params)),
        Err(RpcError::EntryNotFound)
    );
}

#[test]
fn envelope_wire_field_names_match_protocol() {
    let envelope = Envelope {
        protocol: "setup".to_owned(),
        srp: Some(SrpMessage {
            stage: Some("proofToClient".to_owned()),
            m2: Some("m2value".to_owned()),
            ..Default::default()
        }),
        key: None,
        jsonrpc: None,
        error: None,
        version: 0x010804,
        features: Some(FEATURES.iter().map(|f| f.to_string()).collect()),
        client_type_id: None,
        client_display_name: None,
        client_display_description: None,
    };
    let v = serde_json::to_value(&envelope).unwrap();
    assert_eq!(v["srp"]["stage"], "proofToClient");
    assert_eq!(v["srp"]["M2"], "m2value");
    assert_eq!(v["version"], 0x010804);
    assert_eq!(v["features"][1], "KPRPC_GENERAL_CLIENTS");
    assert!(v.get("key").unwrap().is_null());

    let msg: Envelope = serde_json::from_value(v).unwrap();
    assert_eq!(msg.srp.unwrap().m2.unwrap(), "m2value");
}
