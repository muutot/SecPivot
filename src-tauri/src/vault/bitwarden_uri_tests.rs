use super::*;

#[test]
fn six_modes_and_source_domain_host_projections_match_the_independent_official_node_oracle() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/bitwarden-uri-official.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["source"]["commit"],
        "4035e2332dab338d9046a1d7319c644b4368682c"
    );
    let cases = fixture["cases"].as_array().unwrap();
    assert!(cases.len() >= 60);
    prepare_domain_engine().unwrap();
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let settings: BitwardenUriSettings =
            serde_json::from_value(case["settings"].clone()).unwrap();
        let mode = case["match"]
            .as_u64()
            .map(|n| n as u8)
            .unwrap_or(settings.default_match);
        let saved = case["uri"].as_str().unwrap();
        let target = case["target"].as_str().unwrap();
        let policy = super::super::ValidatedWebsiteRules::new(super::super::WebsiteRuleSet {
            version: 2,
            bitwarden: Some(settings),
            rules: vec![WebsiteRule {
                url: saved.into(),
                mode: super::mode(mode).unwrap(),
            }],
        })
        .unwrap();
        assert_eq!(
            policy.matches(target),
            case["expected"].as_bool().unwrap(),
            "{name}"
        );
        assert_eq!(
            serde_json::to_value(host(saved)).unwrap(),
            case["savedHost"],
            "{name}"
        );
        assert_eq!(
            serde_json::to_value(host(target)).unwrap(),
            case["targetHost"],
            "{name}"
        );
        let projections = domains(
            &[saved, target],
            Instant::now() + Duration::from_millis(100),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&projections[0]).unwrap(),
            case["savedDomain"],
            "{name}"
        );
        assert_eq!(
            serde_json::to_value(&projections[1]).unwrap(),
            case["targetDomain"],
            "{name}"
        );
    }
}

#[test]
fn source_profiles_and_domain_regex_resources_fail_closed_without_broadening_rules() {
    let profile = BitwardenUriSettings {
        default_match: 0,
        equivalent_domains: vec![],
    };
    let rule = WebsiteRule {
        url: "https://example.com".into(),
        mode: WebsiteMode::Site,
    };
    assert!(!profile.matches(
        std::slice::from_ref(&rule),
        "https://example.com",
        Instant::now()
    ));
    assert!(!profile.matches(
        &[WebsiteRule {
            url: "".into(),
            mode: WebsiteMode::Regex
        }],
        "https://example.com",
        Instant::now() + Duration::from_secs(1)
    ));
    assert!(domains(&["https://example.com"], Instant::now()).is_err());
    assert!(domains(
        &["a".repeat(4097).as_str()],
        Instant::now() + Duration::from_secs(1)
    )
    .is_err());
    assert!(domains(
        &vec!["https://example.com"; 130],
        Instant::now() + Duration::from_secs(1)
    )
    .is_err());
    let v1 = super::super::WebsiteRuleSet {
        version: 1,
        bitwarden: Some(profile.clone()),
        rules: vec![rule],
    };
    assert!(super::super::ValidatedWebsiteRules::new(v1).is_err());
    for raw in [
        r#"{}"#,
        r#"{"defaultMatch":6,"equivalentDomains":[]}"#,
        r#"{"defaultMatch":0,"equivalentDomains":[],"other":true}"#,
    ] {
        assert!(
            serde_json::from_str::<BitwardenUriSettings>(raw).is_err()
                || serde_json::from_str::<BitwardenUriSettings>(raw)
                    .unwrap()
                    .validate()
                    .is_err()
        );
    }
    let invalid = BitwardenUriSettings {
        default_match: 0,
        equivalent_domains: vec![vec!["example.com secret".into()]],
    };
    assert!(invalid.validate().is_err());
    let deny = WebsiteRule {
        url: "https://example.com".into(),
        mode: WebsiteMode::Never,
    };
    let allow = WebsiteRule {
        url: "https://example.com".into(),
        mode: WebsiteMode::Exact,
    };
    assert!(profile.matches(
        &[deny, allow],
        "https://example.com",
        Instant::now() + Duration::from_secs(1)
    ));
}

#[test]
fn initialized_domain_worker_handles_the_maximum_batch_and_recovers_after_expired_queries() {
    prepare_domain_engine().unwrap();
    for _ in 0..4 {
        assert!(domains(&["https://synthetic.example"], Instant::now()).is_err());
        let started = Instant::now();
        let result = domains(
            &vec!["https://a.example.com/path"; 129],
            started + Duration::from_millis(100),
        )
        .unwrap();
        assert!(result.iter().all(|d| d.as_deref() == Some("example.com")));
        assert!(started.elapsed() < Duration::from_millis(100));
    }
}

#[test]
fn official_domain_parser_keeps_private_tenants_localhost_and_unknown_host_boundaries() {
    let input = [
        "https://a.github.io",
        "https://b.github.io",
        "http://localhost:80/a",
        "https://intranet/a",
        "https://foo.local",
        "https://www.食狮.com.cn",
    ];
    let started = Instant::now();
    let parsed = domains(&input, started + Duration::from_secs(2)).unwrap();
    assert_eq!(
        parsed,
        [
            Some("a.github.io".into()),
            Some("b.github.io".into()),
            Some("localhost".into()),
            None,
            Some("foo.local".into()),
            Some("食狮.com.cn".into())
        ]
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn source_host_normalizes_default_ports_and_js_whitespace_without_changing_native_rules() {
    assert_eq!(
        host("\u{feff}https://Example.com:443/a\u{feff}"),
        Some("example.com".into())
    );
    assert_eq!(host("example.com:80/a"), Some("example.com".into()));
    assert_eq!(host("localhost"), None);
    assert_eq!(host("http://localhost:80"), Some("localhost".into()));
    assert_eq!(host("http://[::1]:8080"), Some("[::1]:8080".into()));
    assert_eq!(unicode_domain("xn--mnchen-3ya.de"), "münchen.de");
    assert_eq!(unicode_domain("食狮.com.cn"), "食狮.com.cn");
}
