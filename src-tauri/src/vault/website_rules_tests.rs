use super::*;
use crate::vault::VaultSession;
use keepass::db::Value;
use serde_json::json;

fn policy(rules: &[(&str, WebsiteMode)]) -> ValidatedWebsiteRules {
    ValidatedWebsiteRules::new(WebsiteRuleSet {
        version: 1,
        rules: rules
            .iter()
            .map(|(url, mode)| WebsiteRule {
                url: (*url).to_owned(),
                mode: *mode,
            })
            .collect(),
    })
    .unwrap()
}

#[test]
fn mixed_modes_are_independent_and_never_does_not_veto_allowed_websites() {
    let rules = policy(&[
        ("https://appleid.apple.com", WebsiteMode::Host),
        ("https://developer.apple.com", WebsiteMode::Host),
        ("https://appleid.apple.com", WebsiteMode::Never),
        ("https://login.site.example", WebsiteMode::Site),
        (
            "https://exact.example/Account?Key=Yes#A",
            WebsiteMode::Exact,
        ),
        ("https://ignored.example", WebsiteMode::Never),
    ]);
    for url in [
        "https://appleid.apple.com/path",
        "https://developer.apple.com/path",
        "https://site.example",
        "https://sub.site.example/path",
        "https://exact.example/Account?Key=Yes#A",
    ] {
        assert!(rules.matches(url));
    }
    for url in [
        "https://apple.com",
        "https://www.appleid.apple.com",
        "https://icloud.com",
        "https://ignored.example",
        "https://exact.example/Account?Key=yes#A",
        "https://exact.example/Account?Key=Yes#B",
        "https://unrelated.example",
    ] {
        assert!(!rules.matches(url));
    }
}

#[test]
fn host_mode_keeps_ports_and_canonical_host_identity() {
    let rules = policy(&[
        ("https://EXAMPLE.com:443/A", WebsiteMode::Host),
        ("https://[2001:db8::1]:8443/A", WebsiteMode::Host),
        ("https://bücher.de", WebsiteMode::Host),
    ]);
    for url in [
        "https://example.com:0443/B",
        "https://[2001:0db8:0:0:0:0:0:1]:8443/B",
        "https://xn--bcher-kva.de/path",
    ] {
        assert!(rules.matches(url));
    }
    for url in [
        "https://example.com/B",
        "https://example.com:444/B",
        "https://www.example.com:443/B",
        "https://[2001:db8::1]/B",
        "https://[2001:db8::2]:8443/B",
    ] {
        assert!(!rules.matches(url));
    }
}

#[test]
fn site_mode_preserves_psl_private_tenants_and_nonregistrable_identity() {
    let rules = policy(&[
        ("https://a.github.io", WebsiteMode::Site),
        ("https://www.city.kawasaki.jp", WebsiteMode::Site),
        ("http://localhost:8080", WebsiteMode::Site),
        ("http://127.0.0.1", WebsiteMode::Site),
    ]);
    for url in [
        "https://login.a.github.io",
        "https://city.kawasaki.jp:8443",
        "https://sub.city.kawasaki.jp",
        "http://localhost:9090",
        "http://127.0.0.1:9090",
    ] {
        assert!(rules.matches(url));
    }
    for url in [
        "https://b.github.io",
        "https://github.io",
        "https://other.kawasaki.jp",
        "http://differenthost",
        "http://127.0.0.2",
    ] {
        assert!(!rules.matches(url));
    }
}

#[test]
fn malformed_unknown_duplicate_and_partial_policies_reject_without_content_echo() {
    for source in [
        "{",
        r#"{"version":1,"rules":[],"unknown":true}"#,
        r#"{"version":1,"version":1,"rules":[]}"#,
        r#"{"version":1,"rules":[{"url":"https://synthetic.example","url":"https://other.example","mode":"host"}]}"#,
        r#"{"version":1,"rules":[{"url":"https://synthetic.example","mode":"host","mode":"never"}]}"#,
        r#"{"version":2,"rules":[]}"#,
        r#"{"version":1,"rules":[{"url":"https://synthetic.example","mode":"future"}]}"#,
        r#"{"version":1,"rules":[{"url":"https://synthetic.example","mode":"host"},{"url":"invalid","mode":"never"}]}"#,
    ] {
        let error = ValidatedWebsiteRules::from_json(source).err().unwrap();
        assert_eq!(error, INVALID);
        assert!(!error.contains("synthetic"));
    }
}

#[test]
fn invalid_or_ambiguous_destinations_cannot_become_partial_matches() {
    for url in [
        "https://safe.example https://other.example",
        " https://safe.example",
        "https://safe.example\r\n",
        "https://safe.example\\path",
        "https://synthetic:fixture@safe.example",
        "javascript:synthetic",
        "file:///synthetic",
        "safe.example",
        "//safe.example",
        "https://",
        "https://safe.example:70000",
    ] {
        let source = json!({"version":1,"rules":[{"url":url,"mode":"site"}]}).to_string();
        assert!(ValidatedWebsiteRules::from_json(&source).is_err());
        assert!(!policy(&[("https://safe.example", WebsiteMode::Site)]).matches(url));
    }
    let rules = policy(&[("https://safe.example", WebsiteMode::Host)]);
    assert!(!rules.matches("https://safe.example@other.example"));
}

#[test]
fn empty_all_never_and_bounded_policies_have_explicit_results() {
    assert!(!policy(&[]).matches("https://safe.example"));
    assert!(
        !policy(&[("https://safe.example", WebsiteMode::Never)]).matches("https://safe.example")
    );
    let valid = policy(&vec![
        ("https://safe.example", WebsiteMode::Host);
        MAX_RULES
    ]);
    assert_eq!(valid.config().rules.len(), MAX_RULES);
    let mut too_many = valid.config().clone();
    too_many.rules.push(too_many.rules[0].clone());
    assert_eq!(ValidatedWebsiteRules::new(too_many).err().unwrap(), LIMIT);
    assert_eq!(
        ValidatedWebsiteRules::from_json(&" ".repeat(MAX_JSON_BYTES + 1))
            .err()
            .unwrap(),
        LIMIT
    );
    let too_long = json!({"version":1,"rules":[{"url":format!("https://safe.example/{}", "a".repeat(MAX_URL_BYTES)),"mode":"exact"}]}).to_string();
    assert_eq!(
        ValidatedWebsiteRules::from_json(&too_long).err().unwrap(),
        LIMIT
    );
    let near_cap_url = format!("https://safe.example/{}", "a".repeat(3819));
    let near_cap = WebsiteRuleSet {
        version: 1,
        rules: vec![
            WebsiteRule {
                url: near_cap_url,
                mode: WebsiteMode::Exact
            };
            17
        ],
    };
    // URLs fit individually and in aggregate, but the complete JSON does not.
    assert_eq!(ValidatedWebsiteRules::new(near_cap).err().unwrap(), LIMIT);
}

#[test]
fn protected_synthetic_kdbx_round_trip_preserves_rules_bytes_order_and_matching() {
    let rules = policy(&[
        (
            "https://exact.example/测试?Case=YES#One",
            WebsiteMode::Exact,
        ),
        ("https://host.example:8443/Path", WebsiteMode::Host),
        ("https://host.example:8443/Path", WebsiteMode::Never),
    ]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic-website.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let id = {
        let db = session.db.as_mut().unwrap();
        let mut root = db.root_mut();
        let mut entry = root.add_entry();
        entry.set("Title", Value::unprotected("Synthetic".to_owned()));
        entry.set("Password", Value::protected("synthetic-fixture".to_owned()));
        entry.set(
            "Synthetic Website Policy",
            Value::protected(rules.as_json().to_owned()),
        );
        entry.id().uuid().to_string()
    };
    session.mark_dirty();
    let wire = serde_json::to_string(&session.state().unwrap()).unwrap();
    assert!(!wire.contains("exact.example") && !wire.contains("synthetic-fixture"));
    session.save().unwrap();
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    let stored = session
        .get_custom_field_value(&id, "Synthetic Website Policy")
        .unwrap()
        .unwrap();
    let reopened = ValidatedWebsiteRules::from_json(&stored).unwrap();
    assert!(reopened.config() == rules.config());
    assert_eq!(reopened.as_json(), rules.as_json());
    assert!(reopened.matches("https://exact.example/测试?Case=YES#One"));
    assert!(!reopened.matches("https://exact.example/测试?Case=yes#One"));
    assert!(reopened.matches("https://host.example:8443/Other"));
    assert!(!reopened.matches("https://host.example/Other"));
}
