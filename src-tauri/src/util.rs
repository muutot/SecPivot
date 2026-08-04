//! Small shared helpers that don't belong to a domain module: URL host
//! extraction and (later) atomic file writes.

/// Lower-cased host of a URL, with scheme/port/path stripped.
///
/// Any `://` scheme is dropped, then the leading host portion up to the first
/// of `/`, `:`, `?`, `#` is taken, trimmed, and lower-cased. Plain hostnames
/// and `host:port` inputs pass through unchanged (everything after the host is
/// a delimiter). Returns `None` when nothing remains, so callers can distinguish
/// "no host" from a literal `""`.
pub fn url_host(url: &str) -> Option<String> {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest
        .split(['/', ':', '?', '#'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_host_strips_scheme_port_and_path() {
        assert_eq!(
            url_host("https://github.com/login"),
            Some("github.com".into())
        );
        assert_eq!(url_host("http://a.b.c:8080/x?y=1"), Some("a.b.c".into()));
        assert_eq!(url_host("ftp://EXAMPLE.com:21"), Some("example.com".into()));
        assert_eq!(url_host("plain-host"), Some("plain-host".into()));
        assert_eq!(url_host("https://"), None);
        assert_eq!(url_host(""), None);
        assert_eq!(url_host("   "), None);
    }
}
