//! Command-layer unit tests (extracted from commands.rs).

use super::*;
use std::io::Write;
use tempfile::TempDir;

fn write_file(dir: &TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn read_text_file_accepts_csv_and_xml_and_rejects_others() {
    let dir = TempDir::new().unwrap();
    let csv = write_file(&dir, "import.csv", "title,username,password\n");
    assert_eq!(read_text_file(csv).unwrap(), "title,username,password\n");

    let xml = write_file(&dir, "vault.kdbx.xml", "<KeePassFile/>");
    assert_eq!(read_text_file(xml).unwrap(), "<KeePassFile/>");

    let json = write_file(&dir, "bitwarden.json", "{\"items\":[]}");
    assert_eq!(read_text_file(json).unwrap(), "{\"items\":[]}");

    let txt = write_file(&dir, "notes.txt", "secret local text");
    let err = read_text_file(txt).unwrap_err();
    assert!(err.contains(".csv"), "unexpected error: {err}");

    let no_ext = write_file(&dir, "config", "{}");
    assert!(read_text_file(no_ext).unwrap_err().contains(".csv"));
}

#[test]
fn read_text_file_rejects_missing_path() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("nope.csv").to_string_lossy().into_owned();
    assert!(read_text_file(missing).unwrap_err().contains("失败"));
}

#[test]
fn parse_proxy_server_handles_wininet_forms() {
    assert_eq!(
        parse_proxy_server("127.0.0.1:51400").as_deref(),
        Some("127.0.0.1:51400")
    );
    assert_eq!(
        parse_proxy_server("host:8080;secure=10.0.0.1:8443").as_deref(),
        Some("10.0.0.1:8443")
    );
    assert_eq!(
        parse_proxy_server("http=127.0.0.1:7890;https=127.0.0.1:7891").as_deref(),
        Some("127.0.0.1:7891")
    );
    assert_eq!(
        parse_proxy_server("https=proxy.local:3128").as_deref(),
        Some("proxy.local:3128")
    );
    assert_eq!(
        parse_proxy_server("ftp=ftp.local:21;http=127.0.0.1:8080").as_deref(),
        Some("127.0.0.1:8080")
    );
    assert_eq!(
        parse_proxy_server("http://127.0.0.1:51400").as_deref(),
        Some("127.0.0.1:51400")
    );
    assert_eq!(parse_proxy_server("").as_deref(), None);
    assert_eq!(parse_proxy_server("ftp=ftp.local:21").as_deref(), None);
}
