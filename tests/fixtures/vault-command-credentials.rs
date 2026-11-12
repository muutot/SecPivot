//! Synthetic command control-flow fixture. The generator substitutes only the
//! password parameter type in four current production handlers. Tauri state,
//! configuration, persistence and adoption APIs are stubs. Registry open is
//! extracted unchanged; blocking work uses real Tokio tasks. This is not native
//! Tauri or KDBX/provider interoperability.
#![allow(dead_code, unexpected_cfgs)]

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc, Arc, Mutex, OnceLock,
};
use zeroize::Zeroize;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fault {
    None,
    Profile,
    Transport,
    Mode,
    Directory,
    PermitError,
    PermitPark,
    PrepareError,
    WorkerPanic,
    WorkerPark,
    Poison,
    AdoptError,
    RegistryError,
}

struct Fixture {
    fault: Fault,
    dropped: AtomicUsize,
    cleared_before_drop: AtomicBool,
    saw_password: AtomicBool,
    registry: Mutex<Option<Arc<Mutex<SessionsInner>>>>,
    track_keyfile: AtomicBool,
    keyfile_drops: AtomicUsize,
    keyfile_cleared: AtomicBool,
    started: tokio::sync::Notify,
    dropped_signal: tokio::sync::Notify,
    blocker: Mutex<Option<mpsc::Receiver<()>>>,
    release: Mutex<Option<mpsc::Sender<()>>>,
}
static CURRENT: OnceLock<Mutex<Option<Arc<Fixture>>>> = OnceLock::new();
fn current() -> Arc<Fixture> {
    CURRENT
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone()
}
fn setup(fault: Fault) -> Arc<Fixture> {
    let (sender, receiver) = mpsc::channel();
    let fixture = Arc::new(Fixture {
        fault,
        dropped: AtomicUsize::new(0),
        cleared_before_drop: AtomicBool::new(false),
        saw_password: AtomicBool::new(false),
        registry: Mutex::new(None),
        track_keyfile: AtomicBool::new(false),
        keyfile_drops: AtomicUsize::new(0),
        keyfile_cleared: AtomicBool::new(false),
        started: tokio::sync::Notify::new(),
        dropped_signal: tokio::sync::Notify::new(),
        blocker: Mutex::new(Some(receiver)),
        release: Mutex::new(Some(sender)),
    });
    *CURRENT.get_or_init(|| Mutex::new(None)).lock().unwrap() = Some(fixture.clone());
    fixture
}
struct ObservedSecret {
    value: String,
    fixture: Arc<Fixture>,
}
impl Deref for ObservedSecret {
    type Target = str;
    fn deref(&self) -> &str {
        &self.value
    }
}
impl Zeroize for ObservedSecret {
    fn zeroize(&mut self) {
        self.value.zeroize();
    }
}
impl Drop for ObservedSecret {
    fn drop(&mut self) {
        // Inspect an owned, still-live value. No raw pointer or freed memory.
        self.fixture
            .cleared_before_drop
            .store(self.value.is_empty(), Ordering::SeqCst);
        self.fixture.dropped.fetch_add(1, Ordering::SeqCst);
        self.fixture.dropped_signal.notify_one();
    }
}
async fn dropped(fixture: &Fixture) -> bool {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while fixture.dropped.load(Ordering::SeqCst) == 0 {
            fixture.dropped_signal.notified().await;
        }
    })
    .await
    .expect("synthetic credential was not released");
    assert_eq!(fixture.dropped.load(Ordering::SeqCst), 1);
    fixture.cleared_before_drop.load(Ordering::SeqCst)
}
struct ObservedBytes {
    value: Vec<u8>,
    fixture: Arc<Fixture>,
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
        self.fixture
            .keyfile_cleared
            .store(self.value.iter().all(|byte| *byte == 0), Ordering::SeqCst);
        self.fixture.keyfile_drops.fetch_add(1, Ordering::SeqCst);
        self.fixture.dropped_signal.notify_one();
    }
}
// PREPARED_KEYFILE_TYPE
fn prepared_keyfile(path: Option<&Path>) -> PreparedKeyfile {
    let fixture = current();
    path.is_some()
        .then(|| ObservedBytes {
            value: vec![4, 8, 12],
            fixture,
        })
        .into()
}
mod tauri {
    pub type State<'a, T> = &'a T;
    pub type AppHandle = ();
    pub mod async_runtime {
        pub use tokio::task::spawn_blocking;
    }
}
struct ConfigStore;
#[derive(Clone)]
struct RemoteConfig {
    backup_count: i32,
    backup_template: String,
}
impl ConfigStore {
    fn remote_profile(&self, _: &str) -> Result<(String, RemoteConfig), String> {
        if current().fault == Fault::Profile {
            return Err("synthetic profile error".into());
        }
        Ok((
            "webdav/synthetic".into(),
            RemoteConfig {
                backup_count: 0,
                backup_template: String::new(),
            },
        ))
    }
}
fn make_storage(_: &RemoteConfig) -> Result<Arc<()>, String> {
    if current().fault == Fault::Transport {
        Err("synthetic transport error".into())
    } else {
        Ok(Arc::new(()))
    }
}
fn local_storage_dir(_: &(), _: &str) -> Result<PathBuf, String> {
    if current().fault == Fault::Directory {
        Err("synthetic directory error".into())
    } else {
        Ok(PathBuf::new())
    }
}
fn normalize_backup_template(template: &str) -> String {
    template.to_owned()
}
fn apply_capture_guard(_: &(), _: &ConfigStore) {}
#[derive(Clone, Copy)]
struct RemoteMode;
impl RemoteMode {
    fn parse(_: &str) -> Result<Self, String> {
        if current().fault == Fault::Mode {
            Err("synthetic mode error".into())
        } else {
            Ok(Self)
        }
    }
}
#[derive(Debug)]
struct VaultState;
#[derive(Debug)]
struct VaultOpenResult {
    session_id: String,
    state: VaultState,
}
#[derive(Default)]
struct VaultSession {
    keyfile: Option<ObservedBytes>,
    opened: bool,
    match_registrable_domain: bool,
    rpc_session_timeout_secs: u64,
    rpc_keys: std::collections::HashMap<String, Vec<u8>>,
}
impl VaultSession {
    fn close(&mut self) {
        self.keyfile.zeroize();
        self.opened = false;
    }
    fn is_open(&self) -> bool {
        self.opened
    }
    fn reset_rpc_key_expiry(&mut self) {}
}
#[derive(Default)]
struct SessionsInner {
    active_id: Option<String>,
    parked: std::collections::HashMap<String, VaultSession>,
    order: Vec<String>,
    match_registrable_domain: bool,
    rpc_session_timeout_secs: u64,
    next: u64,
}
impl SessionsInner {
    fn alloc_id(&mut self) -> String {
        self.next += 1;
        self.next.to_string()
    }
}
#[derive(Default)]
struct VaultSessions {
    inner: Arc<Mutex<SessionsInner>>,
}
impl VaultSessions {
    async fn acquire_persistence_async(&self) -> Result<(), String> {
        let fixture = current();
        match fixture.fault {
            Fault::PermitError => Err("synthetic permit error".into()),
            Fault::PermitPark => {
                fixture.started.notify_one();
                std::future::pending().await
            }
            _ => Ok(()),
        }
    }
    // REGISTRY_OPEN
}
impl VaultSession {
    fn adopt_local(
        &mut self,
        _: (),
        _: &Path,
        password: &str,
        keyfile: Option<ObservedBytes>,
    ) -> Result<VaultState, String> {
        self.keyfile = keyfile;
        self.opened = true;
        adopt(password)
    }
    #[allow(clippy::too_many_arguments)]
    fn adopt_remote(
        &mut self,
        _: (),
        _: Arc<()>,
        _: &str,
        password: &str,
        keyfile: Option<ObservedBytes>,
        _: RemoteMode,
        _: &Path,
        _: usize,
        _: &str,
        _: (),
    ) -> Result<VaultState, String> {
        self.keyfile = keyfile;
        self.opened = true;
        adopt(password)
    }
}
fn adopt(password: &str) -> Result<VaultState, String> {
    assert!(
        password == "synthetic-only",
        "synthetic password changed before adoption"
    );
    if current().fault == Fault::RegistryError {
        let registry = current().registry.lock().unwrap().as_ref().unwrap().clone();
        let result = std::panic::catch_unwind(|| {
            let _guard = registry.lock().unwrap();
            panic!("synthetic poisoned registry after adoption");
        });
        assert!(result.is_err());
    }
    if current().fault == Fault::AdoptError {
        Err("synthetic adoption error".into())
    } else {
        Ok(VaultState)
    }
}
mod vault {
    use super::*;
    fn prepare(password: &str) -> Result<(), String> {
        let fixture = current();
        fixture
            .saw_password
            .store(password == "synthetic-only", Ordering::SeqCst);
        match fixture.fault {
            Fault::WorkerPanic => panic!("synthetic worker panic"),
            Fault::PrepareError => return Err("synthetic preparation error".into()),
            Fault::WorkerPark => {
                fixture.started.notify_one();
                fixture
                    .blocker
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap()
                    .recv()
                    .unwrap();
            }
            _ => (),
        }
        Ok(())
    }
    pub fn prepare_local_open(
        _: &Path,
        password: &str,
        keyfile: Option<&Path>,
    ) -> Result<((), PreparedKeyfile), String> {
        prepare(password)?;
        Ok(((), prepared_keyfile(keyfile)))
    }
    pub fn prepare_local_create(
        _: &Path,
        password: &str,
        _: &str,
        _: &str,
        _: &str,
        keyfile: Option<&Path>,
    ) -> Result<((), PreparedKeyfile), String> {
        prepare(password)?;
        Ok(((), prepared_keyfile(keyfile)))
    }
    pub fn prepare_remote_open(
        _: Arc<()>,
        key: &str,
        password: &str,
        keyfile: Option<&Path>,
        _: RemoteMode,
        _: &Path,
        _: usize,
        _: &str,
    ) -> Result<((), PreparedKeyfile, String, ()), String> {
        prepare(password)?;
        Ok(((), prepared_keyfile(keyfile), key.to_owned(), ()))
    }
    pub fn prepare_remote_create(
        _: Arc<()>,
        key: &str,
        password: &str,
        _: &str,
        _: &str,
        _: &str,
        keyfile: Option<&Path>,
        _: RemoteMode,
        _: &Path,
        _: usize,
        _: &str,
    ) -> Result<((), PreparedKeyfile, String, ()), String> {
        prepare(password)?;
        Ok(((), prepared_keyfile(keyfile), key.to_owned(), ()))
    }
}

// PASSWORD_IMPLEMENTATION
// PASSWORD_TYPE
// PRODUCTION_HANDLERS

#[derive(Clone, Copy, Debug)]
enum Command {
    LocalOpen,
    LocalCreate,
    RemoteOpen,
    RemoteCreate,
}
const COMMANDS: [Command; 4] = [
    Command::LocalOpen,
    Command::LocalCreate,
    Command::RemoteOpen,
    Command::RemoteCreate,
];
const PERMIT_COMMANDS: [Command; 3] = [
    Command::LocalCreate,
    Command::RemoteOpen,
    Command::RemoteCreate,
];
fn invoke<'a>(
    command: Command,
    password: ObservedPassword,
    config: &'a ConfigStore,
    vaults: &'a VaultSessions,
    session: &'a Mutex<VaultSession>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<VaultOpenResult, String>> + Send + 'a>>
{
    let keyfile = current()
        .track_keyfile
        .load(Ordering::SeqCst)
        .then(|| "synthetic.key".to_owned());
    match command {
        Command::LocalOpen => Box::pin(open_vault(
            (),
            config,
            vaults,
            session,
            "synthetic.kdbx".into(),
            password,
            keyfile,
        )),
        Command::LocalCreate => Box::pin(create_vault(
            (),
            config,
            vaults,
            session,
            "synthetic.kdbx".into(),
            password,
            "aes".into(),
            "aes".into(),
            "none".into(),
            keyfile,
        )),
        Command::RemoteOpen => Box::pin(open_remote_vault(
            vaults,
            session,
            (),
            config,
            "webdav/synthetic".into(),
            "synthetic.kdbx".into(),
            password,
            keyfile,
            "memory".into(),
        )),
        Command::RemoteCreate => Box::pin(create_remote_vault(
            vaults,
            session,
            (),
            config,
            "webdav/synthetic".into(),
            "synthetic.kdbx".into(),
            password,
            "aes".into(),
            "aes".into(),
            "none".into(),
            keyfile,
            "memory".into(),
        )),
    }
}
async fn run(command: Command, fixture: Arc<Fixture>) -> Result<VaultOpenResult, String> {
    let session = Mutex::new(VaultSession::default());
    if fixture.fault == Fault::Poison {
        let result = std::panic::catch_unwind(|| {
            let _guard = session.lock().unwrap();
            panic!("synthetic poisoned lock");
        });
        assert!(result.is_err());
    }
    let config = ConfigStore;
    let vaults = VaultSessions::default();
    *fixture.registry.lock().unwrap() = Some(vaults.inner.clone());
    let password = observed(ObservedSecret {
        value: "synthetic-only".into(),
        fixture,
    });
    let result = invoke(command, password, &config, &vaults, &session).await;
    if let Ok(mut active) = session.lock() {
        active.close();
    }
    result
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unpolled_command_futures_clear_the_owned_password() {
    let mut failures = Vec::new();
    for command in COMMANDS {
        let fixture = setup(Fault::None);
        let config = ConfigStore;
        let vaults = VaultSessions::default();
        let session = Mutex::new(VaultSession::default());
        let password = observed(ObservedSecret {
            value: "synthetic-only".into(),
            fixture: fixture.clone(),
        });
        let future = invoke(command, password, &config, &vaults, &session);
        drop(future);
        assert!(!fixture.saw_password.load(Ordering::SeqCst));
        if !dropped(&fixture).await {
            failures.push(command);
        }
    }
    assert!(
        failures.is_empty(),
        "uncleared unpolled scenarios: {failures:?}"
    );
}
#[test]
fn wire_password_keeps_the_string_contract() {
    for text in ["", "synthetic-only", "  synthetic  ", "synthetic\r\n😀"] {
        let json = serde_json::to_string(text).unwrap();
        let password: WirePassword = serde_json::from_str(&json).unwrap();
        let borrowed: &str = &password;
        assert!(borrowed == text, "synthetic wire password changed");
    }
    for json in ["null", "true", "7", "[]", "{}"] {
        assert!(serde_json::from_str::<WirePassword>(json).is_err());
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn normal_success_and_refusals_clear_the_temporary_password() {
    let mut failures = Vec::new();
    for command in COMMANDS {
        for fault in [Fault::None, Fault::PrepareError, Fault::AdoptError] {
            let fixture = setup(fault);
            let result = run(command, fixture.clone()).await;
            assert_eq!(result.is_ok(), fault == Fault::None);
            assert!(fixture.saw_password.load(Ordering::SeqCst));
            if !dropped(&fixture).await {
                failures.push((command, fault));
            }
        }
    }
    for command in PERMIT_COMMANDS {
        let fixture = setup(Fault::PermitError);
        assert!(run(command, fixture.clone()).await.is_err());
        assert!(!fixture.saw_password.load(Ordering::SeqCst));
        if !dropped(&fixture).await {
            failures.push((command, Fault::PermitError));
        }
    }
    assert!(failures.is_empty(), "uncleared scenarios: {failures:?}");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn remote_setup_failures_clear_the_password_before_drop() {
    let mut failures = Vec::new();
    for command in [Command::RemoteOpen, Command::RemoteCreate] {
        for fault in [
            Fault::Profile,
            Fault::Transport,
            Fault::Mode,
            Fault::Directory,
        ] {
            let fixture = setup(fault);
            assert!(run(command, fixture.clone()).await.is_err());
            assert!(!fixture.saw_password.load(Ordering::SeqCst));
            if !dropped(&fixture).await {
                failures.push((command, fault));
            }
        }
    }
    assert!(failures.is_empty(), "uncleared scenarios: {failures:?}");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn worker_panics_and_poisoned_adoption_clear_the_password() {
    let mut failures = Vec::new();
    for command in COMMANDS {
        for fault in [Fault::WorkerPanic, Fault::Poison] {
            let fixture = setup(fault);
            assert!(run(command, fixture.clone()).await.is_err());
            assert!(fixture.saw_password.load(Ordering::SeqCst));
            if !dropped(&fixture).await {
                failures.push((command, fault));
            }
        }
    }
    assert!(failures.is_empty(), "uncleared scenarios: {failures:?}");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cancelling_pending_commands_clears_the_owned_password() {
    let mut failures = Vec::new();
    for (command, fault) in COMMANDS
        .into_iter()
        .map(|c| (c, Fault::WorkerPark))
        .chain(PERMIT_COMMANDS.into_iter().map(|c| (c, Fault::PermitPark)))
    {
        let fixture = setup(fault);
        let work = tokio::spawn(run(command, fixture.clone()));
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            fixture.started.notified(),
        )
        .await
        .expect("synthetic work did not start");
        work.abort();
        assert!(work.await.unwrap_err().is_cancelled());
        if fault == Fault::WorkerPark {
            fixture
                .release
                .lock()
                .unwrap()
                .take()
                .unwrap()
                .send(())
                .unwrap();
        }
        if !dropped(&fixture).await {
            failures.push((command, fault));
        }
    }
    assert!(failures.is_empty(), "uncleared scenarios: {failures:?}");
}

async fn keyfile_dropped(fixture: &Fixture) -> bool {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while fixture.keyfile_drops.load(Ordering::SeqCst) == 0 {
            fixture.dropped_signal.notified().await;
        }
    })
    .await
    .expect("synthetic prepared keyfile was not released");
    assert_eq!(fixture.keyfile_drops.load(Ordering::SeqCst), 1);
    let cleared = fixture.keyfile_cleared.load(Ordering::SeqCst);
    eprintln!(
        "command keyfile cleanup: fault={:?},cleared={cleared}",
        fixture.fault
    );
    cleared
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn prepared_command_keyfiles_keep_adoption_and_cleanup_on_refusal() {
    let mut failures = Vec::new();
    for command in COMMANDS {
        for fault in [
            Fault::None,
            Fault::AdoptError,
            Fault::RegistryError,
            Fault::Poison,
        ] {
            let fixture = setup(fault);
            fixture.track_keyfile.store(true, Ordering::SeqCst);
            assert_eq!(
                run(command, fixture.clone()).await.is_ok(),
                fault == Fault::None
            );
            assert!(dropped(&fixture).await);
            if !keyfile_dropped(&fixture).await {
                failures.push((command, fault));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "uncleared prepared command keyfiles: {failures:?}"
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cancelled_commands_clear_discarded_prepared_keyfiles() {
    let mut failures = Vec::new();
    for command in COMMANDS {
        let fixture = setup(Fault::WorkerPark);
        fixture.track_keyfile.store(true, Ordering::SeqCst);
        let task = tokio::spawn(run(command, fixture.clone()));
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            fixture.started.notified(),
        )
        .await
        .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        fixture
            .release
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(())
            .unwrap();
        assert!(dropped(&fixture).await);
        if !keyfile_dropped(&fixture).await {
            failures.push(command);
        }
    }
    assert!(
        failures.is_empty(),
        "uncleared cancelled prepared keyfiles: {failures:?}"
    );
}
