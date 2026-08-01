//! Auto-type: resolve a KeePass-style auto-type sequence and replay it as
//! keystrokes via `enigo` (the official cross-platform input-simulation crate).
//!
//! Sequence syntax (KeePass subset):
//! - `{USERNAME}` `{PASSWORD}` `{TITLE}` `{URL}` `{NOTES}` — entry placeholders
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
            AutotypeError::Input(msg) => write!(f, "键盘模拟失败: {msg}"),
        }
    }
}

impl std::error::Error for AutotypeError {}

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
}
