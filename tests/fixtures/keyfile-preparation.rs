//! Complete prepare functions with a live-owned keyfile observer. File/KDF/
//! parser/transport/mirror boundaries are stubs; Tokio worker/abort is real.
use std::io::{Cursor, Write};
use std::ops::Deref;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc, Arc, Mutex, OnceLock,
};
use zeroize::Zeroize;
// PREPARATION_IMPORTS
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    None,
    Read,
    Validation,
    Key,
    File,
    Probe,
    Parse,
    Kdf,
    Cipher,
    Compression,
    Save,
    Get,
    Put,
    Mirror,
    Serialize,
    Panic,
    Park,
}
struct Observation {
    fault: Fault,
    drops: AtomicUsize,
    cleared: AtomicBool,
    started: tokio::sync::Notify,
    dropped: tokio::sync::Notify,
    receiver: Mutex<Option<mpsc::Receiver<()>>>,
    release: Mutex<Option<mpsc::Sender<()>>>,
}
static CURRENT: OnceLock<Mutex<Option<Arc<Observation>>>> = OnceLock::new();
fn current() -> Arc<Observation> {
    CURRENT
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone()
}
fn setup(fault: Fault) -> Arc<Observation> {
    let (sender, receiver) = mpsc::channel();
    let result = Arc::new(Observation {
        fault,
        drops: AtomicUsize::new(0),
        cleared: AtomicBool::new(false),
        started: tokio::sync::Notify::new(),
        dropped: tokio::sync::Notify::new(),
        receiver: Mutex::new(Some(receiver)),
        release: Mutex::new(Some(sender)),
    });
    *CURRENT.get_or_init(|| Mutex::new(None)).lock().unwrap() = Some(result.clone());
    result
}
pub(crate) struct ObservedBytes {
    value: Vec<u8>,
    observation: Arc<Observation>,
}
impl Deref for ObservedBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.value
    }
}
impl Zeroize for ObservedBytes {
    fn zeroize(&mut self) {
        self.value.zeroize();
    }
}
impl Drop for ObservedBytes {
    fn drop(&mut self) {
        self.observation
            .cleared
            .store(self.value.iter().all(|byte| *byte == 0), Ordering::SeqCst);
        self.observation.drops.fetch_add(1, Ordering::SeqCst);
        self.observation.dropped.notify_one();
    }
}
// PREPARED_KEYFILE_TYPE
#[derive(Default)]
pub(crate) struct Meta {
    master_key_changed: Option<chrono::NaiveDateTime>,
}
#[derive(Default)]
pub(crate) struct Database {
    meta: Meta,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemoteMode {
    InMemory,
    SaveLocal,
}
pub(crate) trait RemoteStorage: Send + Sync {
    fn get(&self, key: &str) -> Result<Vec<u8>, String>;
    fn put(&self, key: &str, bytes: &[u8]) -> Result<(), String>;
}
struct Storage;
impl RemoteStorage for Storage {
    fn get(&self, _: &str) -> Result<Vec<u8>, String> {
        fail(Fault::Get)?;
        Ok(vec![1, 2, 3])
    }
    fn put(&self, _: &str, _: &[u8]) -> Result<(), String> {
        fail(Fault::Put)
    }
}
pub(crate) struct PersistOutcome {
    pub hash: [u8; 32],
    pub mirror_warning: Option<String>,
}
impl From<[u8; 32]> for PersistOutcome {
    fn from(hash: [u8; 32]) -> Self {
        Self {
            hash,
            mirror_warning: None,
        }
    }
}
fn fail(fault: Fault) -> Result<(), String> {
    if current().fault == fault {
        Err("synthetic boundary refusal".into())
    } else {
        Ok(())
    }
}
impl Database {
    fn new() -> Self {
        Self::default()
    }
    fn parse(_: &[u8], _: ()) -> Result<Self, String> {
        fail(Fault::Parse)?;
        Ok(Self::default())
    }
    fn save(&self, writer: &mut impl Write, _: ()) -> Result<(), String> {
        fail(Fault::Serialize)?;
        writer.write_all(b"synthetic encoded bytes").unwrap();
        Ok(())
    }
}
mod file_io {
    pub fn read(_: &std::path::Path, _: &str) -> Result<Vec<u8>, String> {
        super::fail(super::Fault::File)?;
        Ok(vec![1, 2, 3])
    }
}
struct Probe {
    kind: &'static str,
    note: String,
}
fn probe_vault_bytes(_: &[u8]) -> Probe {
    Probe {
        kind: if current().fault == Fault::Probe {
            "unknown"
        } else {
            "kdbx"
        },
        note: "synthetic probe refusal".into(),
    }
}
fn classify_open_error(error: String) -> String {
    error
}
fn build_database_key(password: &str, keyfile: Option<&[u8]>) -> Result<(), String> {
    assert!(password == "synthetic-only");
    assert!(keyfile.is_none() || keyfile == Some(&[4, 8, 12][..]));
    if current().fault == Fault::Panic {
        panic!("synthetic key construction panic");
    }
    fail(Fault::Key)
}
fn apply_kdf(_: &mut Database, _: &str) -> Result<(), String> {
    fail(Fault::Kdf)
}
fn apply_cipher(_: &mut Database, _: &str) -> Result<(), String> {
    fail(Fault::Cipher)
}
fn apply_compression(_: &mut Database, _: &str) -> Result<(), String> {
    fail(Fault::Compression)
}
fn save_database(_: &Database, _: &Path, _: ()) -> Result<(), String> {
    fail(Fault::Save)
}
fn validate_remote_key(key: &str) -> Result<String, String> {
    fail(Fault::Validation)?;
    Ok(key.into())
}
fn remote_key_basename(key: &str) -> String {
    key.into()
}
fn write_local_copy(_: &Path, _: &str, _: &[u8], _: usize, _: &str) -> Result<(), String> {
    fail(Fault::Mirror)
}
mod persist {
    use super::*;
    type PreparedRemote = (Database, PreparedKeyfile, String, PersistOutcome);
    fn read_keyfile(path: Option<&Path>) -> Result<Option<ObservedBytes>, String> {
        if path.is_none() {
            return Ok(None);
        }
        fail(Fault::Read)?;
        let observation = current();
        let bytes = ObservedBytes {
            value: vec![4, 8, 12],
            observation: observation.clone(),
        };
        if observation.fault == Fault::Park {
            observation.started.notify_one();
            observation
                .receiver
                .lock()
                .unwrap()
                .take()
                .unwrap()
                .recv()
                .unwrap();
        }
        Ok(Some(bytes))
    }
    // PREPARATION_FUNCTIONS
}
#[derive(Clone, Copy, Debug)]
enum Preparation {
    LocalOpen,
    LocalCreate,
    RemoteOpen,
    RemoteCreate,
}
const PREPARATIONS: [Preparation; 4] = [
    Preparation::LocalOpen,
    Preparation::LocalCreate,
    Preparation::RemoteOpen,
    Preparation::RemoteCreate,
];
fn run(preparation: Preparation, has_keyfile: bool) -> Result<PreparedKeyfile, String> {
    let path = Path::new("synthetic.kdbx");
    let keyfile = has_keyfile.then_some(Path::new("synthetic.key"));
    match preparation {
        Preparation::LocalOpen => {
            persist::prepare_local_open(path, "synthetic-only", keyfile).map(|(_, keyfile)| keyfile)
        }
        Preparation::LocalCreate => {
            persist::prepare_local_create(path, "synthetic-only", "Aes", "Aes256", "None", keyfile)
                .map(|(_, keyfile)| keyfile)
        }
        Preparation::RemoteOpen => persist::prepare_remote_open(
            Arc::new(Storage),
            "synthetic.kdbx",
            "synthetic-only",
            keyfile,
            RemoteMode::SaveLocal,
            Path::new("mirror"),
            0,
            "template",
        )
        .map(|(_, keyfile, _, _)| keyfile),
        Preparation::RemoteCreate => persist::prepare_remote_create(
            Arc::new(Storage),
            "synthetic.kdbx",
            "synthetic-only",
            "Aes",
            "Aes256",
            "None",
            keyfile,
            RemoteMode::SaveLocal,
            Path::new("mirror"),
            0,
            "template",
        )
        .map(|(_, keyfile, _, outcome)| {
            assert_eq!(
                outcome.mirror_warning.is_some(),
                current().fault == Fault::Mirror
            );
            let _ = outcome.hash;
            keyfile
        }),
    }
}
fn cleared(observation: &Observation) -> bool {
    assert_eq!(observation.drops.load(Ordering::SeqCst), 1);
    let result = observation.cleared.load(Ordering::SeqCst);
    eprintln!(
        "prepared keyfile cleanup: fault={:?},cleared={result}",
        observation.fault
    );
    result
}
#[test]
fn preparation_refusals_clear_successfully_read_keyfiles() {
    let mut failures = Vec::new();
    for preparation in PREPARATIONS {
        let faults: &[Fault] = match preparation {
            Preparation::LocalOpen => &[Fault::Key, Fault::File, Fault::Probe, Fault::Parse],
            Preparation::LocalCreate => &[
                Fault::Key,
                Fault::Kdf,
                Fault::Cipher,
                Fault::Compression,
                Fault::Save,
            ],
            Preparation::RemoteOpen => &[Fault::Key, Fault::Get, Fault::Parse, Fault::Mirror],
            Preparation::RemoteCreate => &[
                Fault::Key,
                Fault::Kdf,
                Fault::Cipher,
                Fault::Compression,
                Fault::Serialize,
                Fault::Put,
            ],
        };
        for fault in faults {
            let observation = setup(*fault);
            assert!(run(preparation, true).is_err());
            if !cleared(&observation) {
                failures.push((preparation, *fault));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "uncleared preparation refusals: {failures:?}"
    );
}
#[test]
fn successful_prepared_results_keep_values_until_consumer_drop() {
    let mut failures = Vec::new();
    for preparation in PREPARATIONS {
        let observation = setup(Fault::None);
        let result = run(preparation, true).unwrap();
        assert!(result.as_deref() == Some(&[4, 8, 12][..]));
        assert_eq!(observation.drops.load(Ordering::SeqCst), 0);
        drop(result);
        if !cleared(&observation) {
            failures.push(preparation);
        }
    }
    let observation = setup(Fault::Mirror);
    let result = run(Preparation::RemoteCreate, true).unwrap();
    assert!(result.as_deref() == Some(&[4, 8, 12][..]));
    drop(result);
    let mirror = cleared(&observation);
    assert!(
        failures.is_empty() && mirror,
        "uncleared successful outputs: {failures:?},mirror={mirror}"
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn worker_panics_and_discarded_outputs_clear_prepared_keyfiles() {
    let mut failures = Vec::new();
    for preparation in PREPARATIONS {
        let observation = setup(Fault::Panic);
        let Err(error) = tokio::task::spawn_blocking(move || run(preparation, true)).await else {
            panic!("synthetic worker did not panic");
        };
        assert!(error.is_panic());
        if !cleared(&observation) {
            failures.push((preparation, Fault::Panic));
        }
        let observation = setup(Fault::Park);
        let task = tokio::task::spawn_blocking(move || run(preparation, true));
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            observation.started.notified(),
        )
        .await
        .unwrap();
        drop(task);
        observation
            .release
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(())
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while observation.drops.load(Ordering::SeqCst) == 0 {
                observation.dropped.notified().await;
            }
        })
        .await
        .unwrap();
        if !cleared(&observation) {
            failures.push((preparation, Fault::Park));
        }
    }
    assert!(
        failures.is_empty(),
        "uncleared preparation workers: {failures:?}"
    );
}
#[test]
fn absent_keyfiles_and_pre_read_refusals_create_no_observed_buffer() {
    for preparation in PREPARATIONS {
        let observation = setup(Fault::None);
        assert!(run(preparation, false).unwrap().as_deref().is_none());
        assert_eq!(observation.drops.load(Ordering::SeqCst), 0);
        let observation = setup(Fault::Read);
        assert!(run(preparation, true).is_err());
        assert_eq!(observation.drops.load(Ordering::SeqCst), 0);
    }
    for preparation in [Preparation::RemoteOpen, Preparation::RemoteCreate] {
        let observation = setup(Fault::Validation);
        assert!(run(preparation, true).is_err());
        assert_eq!(observation.drops.load(Ordering::SeqCst), 0);
    }
}
