use super::*;

#[test]
fn ecmascript_regex_matches_the_independent_node_oracle() {
    let cases: Vec<(String, String, bool)> = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/ecma-uri-regex.json"
    ))
    .unwrap();
    for (pattern, text, expected) in cases {
        assert_eq!(
            matches(&pattern, &text, Instant::now() + Duration::from_secs(1)),
            expected,
            "pattern={pattern:?}, text={text:?}"
        );
    }
}

#[test]
fn syntax_limits_injection_and_deadlines_never_escape_to_script_execution() {
    for pattern in ["(", "[", "(?", "a{5,2}", "(?<same>a)(?<same>b)"] {
        assert!(!valid(pattern));
        assert!(!matches(
            pattern,
            "synthetic",
            Instant::now() + Duration::from_secs(1)
        ));
    }
    assert!(!valid(""));
    assert!(!valid(&"a".repeat(MAX_BYTES + 1)));
    assert!(!matches(
        "a",
        &"a".repeat(MAX_BYTES + 1),
        Instant::now() + Duration::from_secs(1)
    ));
    assert!(!matches(".*", "synthetic", Instant::now()));
    // Quotes and JavaScript-looking text are only RegExp input, never evaluated.
    let text = "');throw new Error('synthetic injection');//";
    assert!(matches(
        "synthetic injection",
        text,
        Instant::now() + Duration::from_secs(1)
    ));
    assert!(valid("synthetic injection"));
}

#[test]
fn catastrophic_backtracking_is_interrupted_and_does_not_poison_the_next_call() {
    let started = Instant::now();
    assert!(!matches(
        "^(a+)+$",
        &format!("{}!", "a".repeat(256)),
        started + Duration::from_secs(1)
    ));
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(matches(
        "^synthetic$",
        "SYNTHETIC",
        Instant::now() + Duration::from_secs(1)
    ));
}
