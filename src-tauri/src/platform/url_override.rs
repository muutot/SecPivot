//! KeePass global URL scheme overrides (`Tools → Options → Integration →
//! URL Overrides`) and the `{BASE}` / `{URL}` placeholder family.
//!
//! Resolution order mirrors KeePass `AceUrlSchemeOverrides.GetOverrideForUrl`:
//! enabled built-ins first (in source order), then enabled custom rules; the
//! first rule whose scheme matches the URL prefix wins. `None` means "no
//! override" — the caller opens the URL with the system handler.
//!
//! Expansion is pure logic: the caller supplies already-resolved context
//! values (entry fields, application directory, browser paths, encrypted
//! password), so this module never touches the OS or the vault session.

use std::collections::HashMap;
use std::fmt;

use crate::config::settings::{IntegrationSettings, BUILT_IN_URL_OVERRIDES};

/// Values used when expanding an override template. All secrets arrive from
/// the caller in resolved form; nothing is read from disk or config here.
#[derive(Debug, Clone, Default)]
pub struct UrlOverrideContext {
    pub app_dir: String,
    /// `{PASSWORD_ENC}` — DPAPI-protected password, base64 (Windows only).
    pub password_enc: String,
    /// Browser executable paths keyed by placeholder name without braces
    /// (e.g. `EDGE`, `FIREFOX`, `GOOGLECHROME`).
    pub browsers: HashMap<String, String>,
}

#[derive(Debug)]
pub enum ExpandError {
    UnclosedPlaceholder,
    UnknownPlaceholder(String),
}

impl fmt::Display for ExpandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExpandError::UnclosedPlaceholder => write!(f, "URL 覆盖模板中 '{{' 没有对应的 '}}'"),
            ExpandError::UnknownPlaceholder(_) => write!(f, "URL 覆盖模板包含不支持的占位符"),
        }
    }
}

impl std::error::Error for ExpandError {}

/// Extract the scheme of a URL (lowercase, no trailing `:`).
/// Simple prefix scan — works even for non-hierarchical schemes such as
/// `kdbx://C:\path` that `url::Url` cannot parse on all platforms.
pub fn scheme_of(url: &str) -> &str {
    let url = url.trim_start();
    match url.find(':') {
        Some(i) if i > 0 => &url[..i],
        _ => "",
    }
}

/// Return the first enabled override template matching `url`, or `None` when
/// no rule matches. The global-template switch does not gate scheme rules.
/// Built-ins are checked before
/// custom rules, each group in insertion order (KeePass semantics).
pub fn resolve_override<'a>(integration: &'a IntegrationSettings, url: &str) -> Option<&'a str> {
    let scheme = scheme_of(url);
    if scheme.is_empty() {
        return None;
    }
    let wanted = scheme.to_ascii_lowercase();
    for built in BUILT_IN_URL_OVERRIDES {
        if integration.built_in_overrides_enabled & built.flag == 0 {
            continue;
        }
        if built.scheme.eq_ignore_ascii_case(&wanted) {
            return Some(built.url_override);
        }
    }
    for rule in &integration.custom_overrides {
        if !rule.enabled {
            continue;
        }
        if rule.scheme.eq_ignore_ascii_case(&wanted) {
            return Some(rule.url_override.as_str());
        }
    }
    None
}

/// The set of `{URL:...}` / `{BASE:...}` part selectors KeePass documents.
fn url_part(url: &str, selector: &str) -> Option<String> {
    if selector.is_empty() {
        return Some(url.to_owned());
    }
    // Try the standard parser first; it handles percent-decoding, port and
    // userinfo exactly as KeePass expects for http(s)-style URLs.
    if let Ok(parsed) = url::Url::parse(url) {
        let value = match selector {
            "" => parsed.to_string(),
            "SCM" | "SCHEME" => parsed.scheme().to_owned(),
            "RMVSCM" | "WITHOUTSCHEME" => {
                let s = parsed.as_str();
                let prefix = format!("{}:", parsed.scheme());
                s.strip_prefix(&prefix)
                    .unwrap_or(s)
                    .trim_start_matches("//")
                    .to_owned()
            }
            "HOST" => parsed.host_str().unwrap_or("").to_owned(),
            "PORT" => parsed.port().map(|p| p.to_string()).unwrap_or_default(),
            "PATH" => parsed.path().to_owned(),
            "QUERY" => parsed.query().map(|q| format!("?{q}")).unwrap_or_default(),
            "USERINFO" => {
                // url crate has no direct accessor; reconstruct from username
                // and password, preserving the colon between them.
                let u = parsed.username();
                let p = parsed.password().unwrap_or("");
                if p.is_empty() {
                    u.to_owned()
                } else {
                    format!("{u}:{p}")
                }
            }
            "USERNAME" => parsed.username().to_owned(),
            "PASSWORD" => parsed.password().unwrap_or("").to_owned(),
            _ => return None,
        };
        return Some(value);
    }
    // Fallback for URIs the standard parser rejects (e.g. `kdbx://C:\x.y`):
    // remove the scheme and expose the remainder as RMVSCM/BASE.
    match selector {
        "" => Some(url.to_owned()),
        "SCM" | "SCHEME" => Some(scheme_of(url).to_owned()),
        "RMVSCM" | "WITHOUTSCHEME" => Some(
            url.trim_start()
                .split_once(':')
                .map(|(_, rest)| rest.trim_start_matches("//").to_owned())
                .unwrap_or_default(),
        ),
        _ => None,
    }
}

/// Expand every KeePass placeholder supported by the URL override chain.
///
/// - `{BASE}` / `{BASE:PART}`: the URL being overridden
/// - `{URL}` / `{URL:PART}`: the entry URL field (same value in the global
///   override path, but kept distinct for entry-level overrides)
/// - `{USERNAME}` `{PASSWORD}` `{TITLE}` `{NOTES}`: entry fields
/// - `{APPDIR}`: application directory
/// - `{EDGE}` `{FIREFOX}` `{GOOGLECHROME}` `{INTERNETEXPLORER}` `{OPERA}`
///   `{SAFARI}`: browser paths supplied through `ctx.browsers`
/// - `{PASSWORD_ENC}`: encrypted password supplied through `ctx.password_enc`
///
/// Unsupported placeholders are rejected rather than silently emptied, so a
/// mis-typed rule fails visibly instead of launching a wrong command.
pub fn expand(
    template: &str,
    url: &str,
    entry: &crate::platform::autotype::AutotypeContext,
    ctx: &UrlOverrideContext,
) -> Result<String, ExpandError> {
    let mut out = String::with_capacity(template.len());
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '}' {
            if i + 1 < chars.len() && chars[i + 1] == '}' {
                out.push('}');
                i += 2;
            } else {
                out.push('}');
                i += 1;
            }
            continue;
        }
        if chars[i] != '{' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        if i + 1 < chars.len() && chars[i + 1] == '{' {
            out.push('{');
            i += 2;
            continue;
        }
        let start = i + 1;
        let mut end = start;
        while end < chars.len() && chars[end] != '}' {
            end += 1;
        }
        if end == chars.len() {
            return Err(ExpandError::UnclosedPlaceholder);
        }
        let name: String = chars[start..end].iter().collect();
        let upper = name.to_ascii_uppercase();
        let value = if let Some(rest) = upper.strip_prefix("BASE:") {
            url_part(url, rest)
        } else if upper == "BASE" {
            url_part(url, "")
        } else if let Some(rest) = upper.strip_prefix("URL:") {
            url_part(&entry.url, rest)
        } else if upper == "URL" {
            url_part(&entry.url, "")
        } else {
            match upper.as_str() {
                "USERNAME" | "USER" => Some(entry.username.clone()),
                "PASSWORD" | "PASS" => Some(entry.password.clone()),
                "TITLE" => Some(entry.title.clone()),
                "NOTES" => Some(entry.notes.clone()),
                "APPDIR" => Some(ctx.app_dir.clone()),
                "PASSWORD_ENC" => Some(ctx.password_enc.clone()),
                other if ctx.browsers.contains_key(other) => Some(ctx.browsers[other].clone()),
                _ => None,
            }
        };
        match value {
            Some(v) => out.push_str(&v),
            None => return Err(ExpandError::UnknownPlaceholder(name)),
        }
        i = end + 1;
    }
    Ok(out)
}

/// A compiled destination; raw vault URLs never authorize command execution.
#[derive(Debug, PartialEq, Eq)]
pub struct CompiledUrl {
    pub url: String,
    pub allow_command: bool,
}

/// Match KeePass OpenEntryUrl/CompileUrl: entry template, enabled global
/// template, or raw URL; expand first, then apply one scheme rule to that
/// intermediate URL. In the second stage BASE is intermediate, URL is original.
/// None means empty URL or an unchanged plain URL for the frontend handler.
pub fn compile_entry_url(
    integration: &IntegrationSettings,
    entry_override: Option<&str>,
    entry: &crate::platform::autotype::AutotypeContext,
    ctx: &UrlOverrideContext,
) -> Result<Option<CompiledUrl>, ExpandError> {
    if entry.url.trim().is_empty() {
        return Ok(None);
    }
    let template = entry_override
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            (integration.url_override_enabled && !integration.url_override.trim().is_empty())
                .then_some(integration.url_override.as_str())
        });
    let intermediate = expand(
        template.unwrap_or(&entry.url).trim_start(),
        &entry.url,
        entry,
        ctx,
    )?;
    if let Some(scheme_template) = resolve_override(integration, &intermediate) {
        return expand(scheme_template.trim_start(), &intermediate, entry, ctx).map(|url| {
            Some(CompiledUrl {
                url,
                allow_command: true,
            })
        });
    }
    if template.is_none() && intermediate == entry.url {
        return Ok(None);
    }
    Ok(Some(CompiledUrl {
        url: intermediate,
        allow_command: template.is_some(),
    }))
}

/// Split a KeePass `cmd://` command line into program + arguments. Double
/// quotes group arguments containing spaces; a quote can start anywhere in a
/// token (KeePass/ShellExecute convention). Escaped quotes are intentionally
/// not supported — the official docs warn that `\"` is interpreted
/// inconsistently by target applications.
pub fn parse_command_line(line: &str) -> Result<(String, Vec<String>), String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_token = false;
    for c in line.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_token {
                    tokens.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            c => {
                current.push(c);
                has_token = true;
            }
        }
    }
    if in_quotes {
        return Err("命令行引号未闭合".to_owned());
    }
    if has_token {
        tokens.push(current);
    }
    if tokens.is_empty() || tokens[0].is_empty() {
        return Err("命令行程序名为空".to_owned());
    }
    let program = tokens.remove(0);
    Ok((program, tokens))
}

/// KeePass `{PASSWORD_ENC}`: DPAPI-protect the password with the fixed public
/// entropy KeePass uses, then base64-encode. The ciphertext is only valid for
/// the current Windows user and is meant for KeePass's `-pw-enc` parameter.
/// On non-Windows platforms the placeholder stays empty (no OS key store).
#[cfg(target_os = "windows")]
pub fn password_enc(plain: &str) -> Result<String, String> {
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;

    if plain.is_empty() {
        return Ok(String::new());
    }
    // KeePass StrUtil.m_pbOptEnt, published constant (not a secret).
    const ENTROPY: &[u8] = &[0xA5, 0x74, 0x2E, 0xEC];
    let cipher = super::dpapi::protect_bytes_with_entropy(plain.as_bytes(), ENTROPY)?;
    Ok(BASE64.encode(cipher))
}

#[cfg(not(target_os = "windows"))]
pub fn password_enc(_plain: &str) -> Result<String, String> {
    Ok(String::new())
}

/// Best-effort browser executable detection for the built-in browser
/// overrides. Returns paths only for executables that actually exist, so a
/// missing browser yields an empty placeholder and a clear launch failure
/// instead of silently opening the wrong application.
pub fn detect_browser_paths() -> HashMap<String, String> {
    let mut out = HashMap::new();
    let candidates: &[(&str, &[&str])] = &[
        (
            "EDGE",
            &[
                "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
                "C:/Program Files/Microsoft/Edge/Application/msedge.exe",
            ],
        ),
        (
            "FIREFOX",
            &[
                "C:/Program Files/Mozilla Firefox/firefox.exe",
                "C:/Program Files (x86)/Mozilla Firefox/firefox.exe",
            ],
        ),
        (
            "GOOGLECHROME",
            &[
                "C:/Program Files/Google/Chrome/Application/chrome.exe",
                "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe",
            ],
        ),
        (
            "INTERNETEXPLORER",
            &["C:/Program Files (x86)/Internet Explorer/iexplore.exe"],
        ),
        (
            "OPERA",
            &[
                "C:/Program Files/Opera/launcher.exe",
                "C:/Program Files (x86)/Opera/launcher.exe",
            ],
        ),
    ];
    // Also honor LOCALAPPDATA for per-user installs.
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let local_candidates: Vec<(String, String)> = if local.is_empty() {
        Vec::new()
    } else {
        vec![
            (
                "FIREFOX".to_owned(),
                format!("{local}/Mozilla Firefox/firefox.exe"),
            ),
            (
                "GOOGLECHROME".to_owned(),
                format!("{local}/Google/Chrome/Application/chrome.exe"),
            ),
            (
                "OPERA".to_owned(),
                format!("{local}/Programs/Opera/launcher.exe"),
            ),
        ]
    };
    for (name, paths) in candidates {
        for path in *paths {
            if std::path::Path::new(path).is_file() {
                out.insert((*name).to_owned(), (*path).to_owned());
                break;
            }
        }
    }
    for (name, path) in &local_candidates {
        if !out.contains_key(name) && std::path::Path::new(path).is_file() {
            out.insert(name.clone(), path.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::{IntegrationSettings, UrlSchemeOverride};

    fn entry() -> crate::platform::autotype::AutotypeContext {
        crate::platform::autotype::AutotypeContext {
            username: "alice".into(),
            password: "s3cret!".into(),
            title: "Example".into(),
            url: "https://user:pw@keepass.info:80/path/example.php?q=e&s=t".into(),
            notes: "note".into(),
        }
    }

    #[test]
    fn scheme_extraction_is_case_insensitive_and_handles_no_scheme() {
        assert_eq!(scheme_of("  HTTP://x"), "HTTP");
        assert_eq!(scheme_of("kdbx://C:\\vault.kdbx"), "kdbx");
        assert_eq!(scheme_of("not a url"), "");
    }

    #[test]
    fn builtin_first_match_wins_and_custom_rules_follow() {
        // Two http built-ins enabled: EDGE (0x4000) comes before FIREFOX (0x8).
        let mut integration = IntegrationSettings {
            built_in_overrides_enabled: 0x4000 | 0x8,
            ..IntegrationSettings::default()
        };
        let tpl = resolve_override(&integration, "http://example.com").unwrap();
        assert_eq!(tpl, "cmd://{EDGE} \"{BASE}\"");

        // A custom ssh rule is used when no built-in is enabled for ssh.
        integration.custom_overrides.push(UrlSchemeOverride {
            enabled: true,
            scheme: "ssh".into(),
            url_override: "cmd://PuTTY.exe -ssh {USERNAME}@{BASE:RMVSCM}".into(),
        });
        let tpl = resolve_override(&integration, "ssh://10.0.0.1").unwrap();
        assert_eq!(tpl, "cmd://PuTTY.exe -ssh {USERNAME}@{BASE:RMVSCM}");
    }

    #[test]
    fn global_switch_does_not_disable_scheme_rules() {
        let mut integration = IntegrationSettings {
            built_in_overrides_enabled: 0x8,
            ..IntegrationSettings::default()
        };
        integration.custom_overrides.push(UrlSchemeOverride {
            enabled: false,
            scheme: "http".into(),
            url_override: "cmd://x".into(),
        });
        assert_eq!(
            resolve_override(&integration, "http://example.com"),
            Some("cmd://{FIREFOX} \"{BASE}\"")
        );
        integration.url_override_enabled = false;
        assert_eq!(
            resolve_override(&integration, "http://example.com"),
            Some("cmd://{FIREFOX} \"{BASE}\"")
        );
        integration.built_in_overrides_enabled = 0;
        assert_eq!(resolve_override(&integration, "http://example.com"), None);
    }

    #[test]
    fn url_part_selectors_match_keepass_documentation() {
        let url = "https://user:pw@keepass.info:80/path/example.php?q=e&s=t";
        assert_eq!(url_part(url, "").unwrap(), url);
        assert_eq!(url_part(url, "SCM").unwrap(), "https");
        assert_eq!(
            url_part(url, "RMVSCM").unwrap(),
            "user:pw@keepass.info:80/path/example.php?q=e&s=t"
        );
        assert_eq!(url_part(url, "HOST").unwrap(), "keepass.info");
        assert_eq!(url_part(url, "PORT").unwrap(), "80");
        assert_eq!(url_part(url, "PATH").unwrap(), "/path/example.php");
        assert_eq!(url_part(url, "QUERY").unwrap(), "?q=e&s=t");
        assert_eq!(url_part(url, "USERINFO").unwrap(), "user:pw");
        assert_eq!(url_part(url, "USERNAME").unwrap(), "user");
        assert_eq!(url_part(url, "PASSWORD").unwrap(), "pw");
    }

    #[test]
    fn expand_base_and_entry_placeholders() {
        let tpl = "{EDGE} \"{BASE}\" {BASE:HOST} {USERNAME} {PASSWORD_ENC} {URL:SCM}";
        let mut ctx = UrlOverrideContext {
            app_dir: "C:\\Program Files\\SecPivot".into(),
            password_enc: "ENC".into(),
            ..UrlOverrideContext::default()
        };
        ctx.browsers.insert("EDGE".into(), "msedge.exe".into());
        let expanded = expand(tpl, "https://example.com/p", &entry(), &ctx).unwrap();
        assert_eq!(
            expanded,
            "msedge.exe \"https://example.com/p\" example.com alice ENC https"
        );
    }

    #[test]
    fn expand_rejects_unknown_placeholder_and_unclosed_brace() {
        let ctx = UrlOverrideContext::default();
        assert!(matches!(
            expand("{NOPE}", "https://x", &entry(), &ctx),
            Err(ExpandError::UnknownPlaceholder(_))
        ));
        assert!(matches!(
            expand("{BASE", "https://x", &entry(), &ctx),
            Err(ExpandError::UnclosedPlaceholder)
        ));
    }

    #[test]
    fn escaped_braces_stay_literal() {
        let ctx = UrlOverrideContext::default();
        assert_eq!(
            expand("{{BASE}}", "https://x", &entry(), &ctx).unwrap(),
            "{BASE}"
        );
    }

    #[test]
    fn plain_url_uses_enabled_scheme_rule() {
        let integration = IntegrationSettings {
            built_in_overrides_enabled: 0x8, // FIREFOX http
            ..IntegrationSettings::default()
        };
        let mut ctx = UrlOverrideContext::default();
        ctx.browsers.insert("FIREFOX".into(), "firefox.exe".into());
        let mut entry = entry();
        entry.url = "http://example.com/".into();
        let out = compile_entry_url(&integration, None, &entry, &ctx)
            .unwrap()
            .unwrap();
        assert_eq!(out.url, "cmd://firefox.exe \"http://example.com/\"");
        assert!(out.allow_command);
    }

    #[test]
    fn entry_then_global_template_priority_and_independent_switch() {
        let mut integration = IntegrationSettings {
            url_override: "https://global.test/{USERNAME}".into(),
            ..IntegrationSettings::default()
        };
        let ctx = UrlOverrideContext::default();
        for enabled in [true, false] {
            integration.url_override_enabled = enabled;
            let out = compile_entry_url(
                &integration,
                Some("https://entry.test/{TITLE}"),
                &entry(),
                &ctx,
            )
            .unwrap()
            .unwrap();
            assert_eq!(out.url, "https://entry.test/Example");
        }
        assert_eq!(
            compile_entry_url(&integration, None, &entry(), &ctx).unwrap(),
            None
        );
        integration.url_override_enabled = true;
        assert_eq!(
            compile_entry_url(&integration, Some(" "), &entry(), &ctx)
                .unwrap()
                .unwrap()
                .url,
            "https://global.test/alice"
        );
        integration.url_override.clear();
        assert_eq!(
            compile_entry_url(&integration, None, &entry(), &ctx).unwrap(),
            None
        );
    }

    #[test]
    fn scheme_matches_expanded_template_and_base_changes_only_in_second_stage() {
        let mut integration = IntegrationSettings {
            url_override: " {USERNAME}://relay.test/path".into(),
            custom_overrides: vec![
                UrlSchemeOverride {
                    enabled: true,
                    scheme: "https".into(),
                    url_override: "cmd://wrong".into(),
                },
                UrlSchemeOverride {
                    enabled: true,
                    scheme: "alice".into(),
                    url_override: "cmd://client \"{BASE}\" \"{URL}\" {BASE:HOST} {URL:HOST}".into(),
                },
                UrlSchemeOverride {
                    enabled: true,
                    scheme: "cmd".into(),
                    url_override: "cmd://recursive".into(),
                },
            ],
            ..IntegrationSettings::default()
        };
        let ctx = UrlOverrideContext::default();
        let expected = format!(
            "cmd://client \"alice://relay.test/path\" \"{}\" relay.test keepass.info",
            entry().url
        );
        assert_eq!(
            compile_entry_url(&integration, None, &entry(), &ctx)
                .unwrap()
                .unwrap()
                .url,
            expected
        );
        // Entry-level templates undergo the same single scheme pass even when global is off.
        integration.url_override_enabled = false;
        assert_eq!(
            compile_entry_url(
                &integration,
                Some("{USERNAME}://relay.test/path"),
                &entry(),
                &ctx
            )
            .unwrap()
            .unwrap()
            .url,
            expected
        );
    }

    #[test]
    fn template_base_and_url_preserve_original_bytes_and_command_spacing() {
        let mut entry = entry();
        entry.url = "HTTPS://EXAMPLE.test:443/?q=%2f&x=1".into();
        let integration = IntegrationSettings {
            url_override: "  cmd://client  \"{BASE}\"  \"{URL}\"  ".into(),
            ..IntegrationSettings::default()
        };
        let out = compile_entry_url(&integration, None, &entry, &UrlOverrideContext::default())
            .unwrap()
            .unwrap();
        assert_eq!(
            out.url,
            format!("cmd://client  \"{}\"  \"{}\"  ", entry.url, entry.url)
        );
    }

    #[test]
    fn empty_entry_url_never_opens_even_with_templates() {
        let mut entry = entry();
        entry.url.clear();
        let integration = IntegrationSettings {
            url_override: "cmd://client".into(),
            ..IntegrationSettings::default()
        };
        assert_eq!(
            compile_entry_url(
                &integration,
                Some("cmd://other"),
                &entry,
                &UrlOverrideContext::default()
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn invalid_selected_templates_fail_without_falling_back_or_exposing_content() {
        let integration = IntegrationSettings {
            url_override: "{INVALID_SECRET}".into(),
            ..IntegrationSettings::default()
        };
        let ctx = UrlOverrideContext::default();
        let err = compile_entry_url(&integration, None, &entry(), &ctx).unwrap_err();
        assert!(!err.to_string().contains("INVALID_SECRET"));
        assert!(compile_entry_url(&integration, Some("{BASE"), &entry(), &ctx).is_err());
    }

    #[test]
    fn raw_placeholder_expansion_does_not_authorize_a_command() {
        let mut entry = entry();
        entry.url = "{NOTES}".into();
        entry.notes = "cmd://client".into();
        let out = compile_entry_url(
            &IntegrationSettings::default(),
            None,
            &entry,
            &UrlOverrideContext::default(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(out.url, "cmd://client");
        assert!(!out.allow_command);
    }
}
