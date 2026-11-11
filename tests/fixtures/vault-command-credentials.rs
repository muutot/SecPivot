//! Synthetic command control-flow fixture. The generator substitutes only the
//! password parameter type in four current production handlers. Tauri state,
//! configuration, persistence and adoption APIs are stubs; blocking work uses
//! real Tokio tasks. This is not native Tauri or KDBX/provider interoperability.
#![allow(dead_code)]

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
}

struct Fixture {
    fault: Fault,
    dropped: AtomicUsize,
    cleared_before_drop: AtomicBool,
    saw_password: AtomicBool,
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
struct VaultOpenResult;
struct VaultSession;
struct VaultSessions;
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
    fn open(
        &self,
        active: &mut VaultSession,
        operation: impl FnOnce(&mut VaultSession) -> Result<VaultOpenResult, String>,
    ) -> Result<VaultOpenResult, String> {
        operation(active)
    }
}
impl VaultSession {
    fn adopt_local(
        &mut self,
        _: (),
        _: &Path,
        password: &str,
        _: Option<Vec<u8>>,
    ) -> Result<VaultOpenResult, String> {
        adopt(password)
    }
    #[allow(clippy::too_many_arguments)]
    fn adopt_remote(
        &mut self,
        _: (),
        _: Arc<()>,
        _: &str,
        password: &str,
        _: Option<Vec<u8>>,
        _: RemoteMode,
        _: &Path,
        _: usize,
        _: &str,
        _: (),
    ) -> Result<VaultOpenResult, String> {
        adopt(password)
    }
}
fn adopt(password: &str) -> Result<VaultOpenResult, String> {
    assert!(
        password == "synthetic-only",
        "synthetic password changed before adoption"
    );
    if current().fault == Fault::AdoptError {
        Err("synthetic adoption error".into())
    } else {
        Ok(VaultOpenResult)
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
        _: Option<&Path>,
    ) -> Result<((), Option<Vec<u8>>), String> {
        prepare(password)?;
        Ok(((), None))
    }
    pub fn prepare_local_create(
        _: &Path,
        password: &str,
        _: &str,
        _: &str,
        _: &str,
        _: Option<&Path>,
    ) -> Result<((), Option<Vec<u8>>), String> {
        prepare(password)?;
        Ok(((), None))
    }
    pub fn prepare_remote_open(
        _: Arc<()>,
        key: &str,
        password: &str,
        _: Option<&Path>,
        _: RemoteMode,
        _: &Path,
        _: usize,
        _: &str,
    ) -> Result<((), Option<Vec<u8>>, String, ()), String> {
        prepare(password)?;
        Ok(((), None, key.to_owned(), ()))
    }
    pub fn prepare_remote_create(
        _: Arc<()>,
        key: &str,
        password: &str,
        _: &str,
        _: &str,
        _: &str,
        _: Option<&Path>,
        _: RemoteMode,
        _: &Path,
        _: usize,
        _: &str,
    ) -> Result<((), Option<Vec<u8>>, String, ()), String> {
        prepare(password)?;
        Ok(((), None, key.to_owned(), ()))
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
    match command {
        Command::LocalOpen => Box::pin(open_vault(
            (),
            config,
            vaults,
            session,
            "synthetic.kdbx".into(),
            password,
            None,
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
            None,
        )),
        Command::RemoteOpen => Box::pin(open_remote_vault(
            vaults,
            session,
            (),
            config,
            "webdav/synthetic".into(),
            "synthetic.kdbx".into(),
            password,
            None,
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
            None,
            "memory".into(),
        )),
    }
}
async fn run(command: Command, fixture: Arc<Fixture>) -> Result<VaultOpenResult, String> {
    let session = Mutex::new(VaultSession);
    if fixture.fault == Fault::Poison {
        let result = std::panic::catch_unwind(|| {
            let _guard = session.lock().unwrap();
            panic!("synthetic poisoned lock");
        });
        assert!(result.is_err());
    }
    let config = ConfigStore;
    let vaults = VaultSessions;
    let password = observed(ObservedSecret {
        value: "synthetic-only".into(),
        fixture,
    });
    invoke(command, password, &config, &vaults, &session).await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unpolled_command_futures_clear_the_owned_password() {
    let mut failures = Vec::new();
    for command in COMMANDS {
        let fixture = setup(Fault::None);
        let config = ConfigStore;
        let vaults = VaultSessions;
        let session = Mutex::new(VaultSession);
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
