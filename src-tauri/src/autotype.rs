//! Auto-type: resolve a KeePass-style auto-type sequence and replay it as
//! keystrokes via `enigo` (the official cross-platform input-simulation crate).
//!
//! Sequence syntax (KeePass subset):
//! - `{USERNAME}` `{PASSWORD}` `{TITLE}` `{URL}` `{NOTES}` — entry placeholders
//! - `{REF:<Field>@<SearchIn>:<Text>}` — field references to other entries
//!   (`{REF:U@I:46C9...}` inserts the user name of the entry whose UUID
//!   matches; `T`/`U`/`P`/`A`/`N`/`I` are searchable fields, `O` searches
//!   custom string names; text matching is case-insensitive substring)
//! - `{TAB}` `{ENTER}` `{SPACE}` `{BACKSPACE}` `{DELETE}` `{ESC}` `{UP}`
//!   `{DOWN}` `{LEFT}` `{RIGHT}` `{HOME}` `{END}` `{PAGEUP}` `{PAGEDOWN}` —
//!   special keys
//! - `{{` and `}}` escape literal braces
//! - any other text is typed as-is
//!
//! Parsing is pure logic and unit-tested; execution runs on a background
//! thread so the UI never blocks while keystrokes are being sent.

use std::fmt;
use std::thread;
use std::time::Duration;

use enigo::{Direction, Enigo, Key, Keyboard, Settings};

/// Values available for `{PLACEHOLDER}` substitution.
#[derive(Debug, Clone, Default)]
pub struct AutotypeContext {
    pub username: String,
    pub password: String,
    pub title: String,
    pub url: String,
    pub notes: String,
}

/// One resolved auto-type action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutotypeToken {
    /// Literal text to type.
    Text(String),
    /// A special key to press.
    Key(SpecialKey),
}

/// Special keys understood by the sequence syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    Tab,
    Enter,
    Space,
    Backspace,
    Delete,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
}

/// Errors resolving or executing an auto-type sequence.
#[derive(Debug)]
pub enum AutotypeError {
    /// A `{PLACEHOLDER}` with an unknown name.
    UnknownPlaceholder(String),
    /// The sequence contains an unmatched `{`.
    UnclosedPlaceholder,
    /// A `{REF:...}` field reference is malformed.
    InvalidRef(String),
    /// A well-formed `{REF:...}` did not resolve to any entry.
    RefNotFound(String),
    /// enigo reported a keyboard simulation failure.
    Input(String),
}

impl fmt::Display for AutotypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AutotypeError::UnknownPlaceholder(p) => {
                write!(f, "未知的自动填充占位符: {{{p}}}")
            }
            AutotypeError::UnclosedPlaceholder => {
                write!(f, "自动填充序列中 '{{' 没有对应的 '}}'")
            }
            AutotypeError::InvalidRef(spec) => {
                write!(
                    f,
                    "字段引用格式无效: {{REF:{spec}}}(应为 {{REF:字段@搜索字段:文本}})"
                )
            }
            AutotypeError::RefNotFound(spec) => {
                write!(f, "字段引用未找到匹配条目: {{REF:{spec}}}")
            }
            AutotypeError::Input(msg) => write!(f, "键盘模拟失败: {msg}"),
        }
    }
}

impl std::error::Error for AutotypeError {}

/// A parsed `{REF:<Field>@<SearchIn>:<Text>}` field reference.
///
/// `field` is the value to fetch (`T`/`U`/`P`/`A`/`N`/`I`), `search` the field
/// to match on (`T`/`U`/`P`/`A`/`N`/`I`/`O` — `O` matches custom string
/// names), and `text` the search string (substring, case-insensitive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefSpec<'a> {
    pub field: &'a str,
    pub search: &'a str,
    pub text: &'a str,
}

const REF_FIELDS: &str = "TUPANI";
const REF_SEARCH_FIELDS: &str = "TUPANIO";

/// Replace every `{REF:...}` placeholder in `sequence` using `resolve`.
///
/// `resolve` receives each parsed reference and returns the value to insert
/// (or `None` when nothing matches). Non-`REF` placeholders and `{{`/`}}`
/// escapes are preserved verbatim so `parse_sequence` can process them later.
pub fn expand_refs(
    sequence: &str,
    resolve: impl Fn(RefSpec<'_>) -> Option<String>,
) -> Result<String, AutotypeError> {
    let mut out = String::with_capacity(sequence.len());
    let mut chars = sequence.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    out.push('{');
                    continue;
                }
                let mut name = String::new();
                let mut closed = false;
                for inner in chars.by_ref() {
                    if inner == '}' {
                        closed = true;
                        break;
                    }
                    name.push(inner);
                }
                if !closed {
                    return Err(AutotypeError::UnclosedPlaceholder);
                }
                let upper = name.to_ascii_uppercase();
                if upper.starts_with("REF:") {
                    let spec = parse_ref(&name[4..])?;
                    let value =
                        resolve(spec).ok_or_else(|| AutotypeError::RefNotFound(name.clone()))?;
                    out.push_str(&value);
                } else {
                    out.push('{');
                    out.push_str(&name);
                    out.push('}');
                }
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                }
                out.push('}');
            }
            _ => out.push(c),
        }
    }
    Ok(out)
}

/// Parse the inner text of a `{REF:...}` placeholder (`<Field>@<Search>:<Text>`).
fn parse_ref(rest: &str) -> Result<RefSpec<'_>, AutotypeError> {
    let invalid = || AutotypeError::InvalidRef(rest.to_owned());
    let (field, after_at) = rest.split_once('@').ok_or_else(invalid)?;
    let (search, text) = after_at.split_once(':').ok_or_else(invalid)?;
    let is_code = |code: &str, valid: &str| {
        let code = code.to_ascii_uppercase();
        code.len() == 1 && valid.contains(&code)
    };
    if !is_code(field, REF_FIELDS) || !is_code(search, REF_SEARCH_FIELDS) {
        return Err(invalid());
    }
    Ok(RefSpec {
        field,
        search,
        text,
    })
}

impl SpecialKey {
    /// Map a `{NAME}` token to a special key. Returns `None` for unknown
    /// names so the caller can report a placeholder error.
    fn from_name(name: &str) -> Option<SpecialKey> {
        Some(match name.to_ascii_uppercase().as_str() {
            "TAB" => SpecialKey::Tab,
            "ENTER" => SpecialKey::Enter,
            "SPACE" => SpecialKey::Space,
            "BACKSPACE" => SpecialKey::Backspace,
            "DELETE" => SpecialKey::Delete,
            "ESC" | "ESCAPE" => SpecialKey::Escape,
            "UP" => SpecialKey::Up,
            "DOWN" => SpecialKey::Down,
            "LEFT" => SpecialKey::Left,
            "RIGHT" => SpecialKey::Right,
            "HOME" => SpecialKey::Home,
            "END" => SpecialKey::End,
            "PAGEUP" => SpecialKey::PageUp,
            "PAGEDOWN" => SpecialKey::PageDown,
            _ => return None,
        })
    }

    /// Map the special key to the enigo `Key` variant.
    fn to_enigo(self) -> Key {
        match self {
            SpecialKey::Tab => Key::Tab,
            SpecialKey::Enter => Key::Return,
            SpecialKey::Space => Key::Space,
            SpecialKey::Backspace => Key::Backspace,
            SpecialKey::Delete => Key::Delete,
            SpecialKey::Escape => Key::Escape,
            SpecialKey::Up => Key::UpArrow,
            SpecialKey::Down => Key::DownArrow,
            SpecialKey::Left => Key::LeftArrow,
            SpecialKey::Right => Key::RightArrow,
            SpecialKey::Home => Key::Home,
            SpecialKey::End => Key::End,
            SpecialKey::PageUp => Key::PageUp,
            SpecialKey::PageDown => Key::PageDown,
        }
    }
}

/// Resolve `{PLACEHOLDER}` tokens against the context, producing the token
/// stream to execute. Pure logic — no I/O.
pub fn parse_sequence(
    sequence: &str,
    ctx: &AutotypeContext,
) -> Result<Vec<AutotypeToken>, AutotypeError> {
    let mut tokens: Vec<AutotypeToken> = Vec::new();
    let mut literal = String::new();

    fn flush_literal(tokens: &mut Vec<AutotypeToken>, literal: &mut String) {
        if !literal.is_empty() {
            tokens.push(AutotypeToken::Text(std::mem::take(literal)));
        }
    }

    let mut chars = sequence.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    literal.push('{');
                    continue;
                }
                let mut name = String::new();
                let mut closed = false;
                for inner in chars.by_ref() {
                    if inner == '}' {
                        closed = true;
                        break;
                    }
                    name.push(inner);
                }
                if !closed {
                    return Err(AutotypeError::UnclosedPlaceholder);
                }
                let resolved = if let Some(key) = SpecialKey::from_name(&name) {
                    Some(AutotypeToken::Key(key))
                } else {
                    match name.to_ascii_uppercase().as_str() {
                        "USERNAME" | "USER" => Some(AutotypeToken::Text(ctx.username.clone())),
                        "PASSWORD" | "PASS" => Some(AutotypeToken::Text(ctx.password.clone())),
                        "TITLE" => Some(AutotypeToken::Text(ctx.title.clone())),
                        "URL" => Some(AutotypeToken::Text(ctx.url.clone())),
                        "NOTES" => Some(AutotypeToken::Text(ctx.notes.clone())),
                        _ => None,
                    }
                };
                match resolved {
                    Some(token) => {
                        flush_literal(&mut tokens, &mut literal);
                        tokens.push(token);
                    }
                    None => return Err(AutotypeError::UnknownPlaceholder(name)),
                }
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    literal.push('}');
                } else {
                    literal.push('}');
                }
            }
            _ => literal.push(c),
        }
    }

    flush_literal(&mut tokens, &mut literal);
    Ok(tokens)
}

/// Replay a resolved token stream via enigo keyboard simulation.
pub fn execute_tokens(tokens: &[AutotypeToken]) -> Result<(), AutotypeError> {
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| AutotypeError::Input(e.to_string()))?;
    for token in tokens {
        match token {
            AutotypeToken::Text(text) => {
                enigo
                    .text(text)
                    .map_err(|e| AutotypeError::Input(e.to_string()))?;
            }
            AutotypeToken::Key(key) => {
                enigo
                    .key(key.to_enigo(), Direction::Click)
                    .map_err(|e| AutotypeError::Input(e.to_string()))?;
            }
        }
    }
    Ok(())
}

/// Resolve and replay an auto-type sequence on a background thread.
///
/// A short startup delay gives the user time to focus the target window
/// before keystrokes are sent. Failures are logged rather than propagated,
/// because the replay happens after this call has already returned.
pub fn run_sequence(sequence: &str, ctx: &AutotypeContext) -> Result<(), AutotypeError> {
    let tokens = parse_sequence(sequence, ctx)?;
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        if let Err(e) = execute_tokens(&tokens) {
            eprintln!("auto-type failed: {e}");
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> AutotypeContext {
        AutotypeContext {
            username: "alice".to_owned(),
            password: "s3cret!".to_owned(),
            title: "GitHub".to_owned(),
            url: "https://github.com".to_owned(),
            notes: "primary account".to_owned(),
        }
    }

    #[test]
    fn resolves_standard_login_sequence() {
        let tokens = parse_sequence("{USERNAME}{TAB}{PASSWORD}{ENTER}", &ctx()).expect("parse");
        assert_eq!(
            tokens,
            vec![
                AutotypeToken::Text("alice".to_owned()),
                AutotypeToken::Key(SpecialKey::Tab),
                AutotypeToken::Text("s3cret!".to_owned()),
                AutotypeToken::Key(SpecialKey::Enter),
            ]
        );
    }

    #[test]
    fn mixed_literal_and_placeholder_text() {
        let tokens = parse_sequence("site: {TITLE} user {USERNAME}", &ctx()).expect("parse");
        let combined: String = tokens
            .iter()
            .map(|t| match t {
                AutotypeToken::Text(s) => s.clone(),
                AutotypeToken::Key(k) => format!("[{k:?}]"),
            })
            .collect();
        assert_eq!(combined, "site: GitHub user alice");
    }

    #[test]
    fn all_special_keys_parse() {
        let seq = "{TAB}{ENTER}{SPACE}{BACKSPACE}{DELETE}{ESC}{UP}{DOWN}{LEFT}{RIGHT}{HOME}{END}{PAGEUP}{PAGEDOWN}";
        let tokens = parse_sequence(seq, &ctx()).expect("parse");
        assert_eq!(tokens.len(), 14, "every special key becomes one token");
        assert!(matches!(tokens[0], AutotypeToken::Key(SpecialKey::Tab)));
        assert!(matches!(
            tokens[13],
            AutotypeToken::Key(SpecialKey::PageDown)
        ));
    }

    #[test]
    fn unknown_placeholder_is_rejected() {
        let err = parse_sequence("{NOPE}", &ctx()).expect_err("unknown placeholder");
        assert!(matches!(err, AutotypeError::UnknownPlaceholder(_)));
        assert!(format!("{err}").contains("NOPE"));
    }

    #[test]
    fn unclosed_brace_is_rejected() {
        let err = parse_sequence("abc{USERNAME", &ctx()).expect_err("unclosed brace");
        assert!(matches!(err, AutotypeError::UnclosedPlaceholder));
    }

    #[test]
    fn doubled_braces_are_literal() {
        let tokens = parse_sequence("{{USERNAME}}", &ctx()).expect("parse");
        assert_eq!(tokens, vec![AutotypeToken::Text("{USERNAME}".to_owned())]);
    }

    #[test]
    fn empty_and_plain_text_sequences() {
        assert_eq!(parse_sequence("", &ctx()).expect("empty"), vec![]);
        assert_eq!(
            parse_sequence("just text", &ctx()).expect("plain"),
            vec![AutotypeToken::Text("just text".to_owned())]
        );
    }

    #[test]
    fn expand_refs_resolves_and_preserves_other_placeholders() {
        let resolve = |spec: RefSpec<'_>| match spec.search.to_ascii_uppercase().as_str() {
            "I" => Some(format!("ref-{}", spec.text)),
            "O" => Some("attr-value".to_owned()),
            _ => None,
        };
        assert_eq!(
            expand_refs(
                "user: {REF:U@I:46C9B1FFBD4ABC4BBB260C6190BAD20C} on {USERNAME}",
                resolve,
            )
            .expect("expand"),
            "user: ref-46C9B1FFBD4ABC4BBB260C6190BAD20C on {USERNAME}"
        );
        // O as search field, custom-attribute name as text.
        assert_eq!(
            expand_refs("{REF:P@O:Banking Pin}", resolve).expect("expand"),
            "attr-value"
        );
        // Doubled braces stay literal; unmatched braces preserved for parse_sequence.
        assert_eq!(
            expand_refs("{{REF:U@I:X}} and {TAB}", resolve).expect("expand"),
            "{REF:U@I:X} and {TAB}"
        );
    }

    #[test]
    fn expand_refs_case_insensitive_and_field_codes() {
        let resolve = |spec: RefSpec<'_>| {
            assert!(REF_FIELDS.contains(&spec.field.to_ascii_uppercase()));
            assert!(REF_SEARCH_FIELDS.contains(&spec.search.to_ascii_uppercase()));
            // Placeholder names are case-insensitive but the search text is preserved.
            Some(format!("{}@{}:{}", spec.field, spec.search, spec.text))
        };
        assert_eq!(
            expand_refs("{ref:u@i:abc}", resolve).expect("expand"),
            "u@i:abc"
        );
        assert_eq!(
            expand_refs("{REF:N@T:Notes here}", resolve).expect("expand"),
            "N@T:Notes here"
        );
    }

    #[test]
    fn expand_refs_rejects_malformed_or_unresolved() {
        let never = |_: RefSpec<'_>| None::<String>;
        for bad in ["{REF:U}", "{REF:X@I:1}", "{REF:U@X:1}"] {
            let err = expand_refs(bad, never).expect_err("invalid");
            assert!(matches!(err, AutotypeError::InvalidRef(_)), "{bad}");
        }
        // Empty search text is syntactically valid but matches nothing here.
        let err = expand_refs("{REF:U@I:}", never).expect_err("not found");
        assert!(matches!(err, AutotypeError::RefNotFound(_)));
        let err = expand_refs("{REF:P@I:0000}", never).expect_err("not found");
        assert!(matches!(err, AutotypeError::RefNotFound(_)));
        let err = expand_refs("{REF:U@I:1", never).expect_err("unclosed");
        assert!(matches!(err, AutotypeError::UnclosedPlaceholder));
    }
}
