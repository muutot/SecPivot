//! Complete production change_master_key command with observed password/bytes.
//! State, read, persistence and completion APIs are synthetic boundaries; Tokio
//! worker and cancellation execute. Only live-owned values are inspected.
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc, Arc, Mutex, OnceLock,
};
use zeroize::Zeroize;

// MASTER_TYPES

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    None,
    Read,
    Permit,
    Resolve,
    Plan,
    Poison,
    PermitPark,
    WorkerPanic,
    WorkerPark,
    Persist,
    Persistent,
    Missing,
    Snapshot,
}
struct Fixture {
    fault: Fault,
    password_drops: AtomicUsize,
    password_cleared: AtomicBool,
    keyfile_drops: AtomicUsize,
    keyfile_cleared: AtomicBool,
    read: AtomicBool,
    persisted: AtomicBool,
    adopted: AtomicBool,
    failure_noted: AtomicBool,
    started: tokio::sync::Notify,
    dropped: tokio::sync::Notify,
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
        password_drops: AtomicUsize::new(0),
        password_cleared: AtomicBool::new(false),
        keyfile_drops: AtomicUsize::new(0),
        keyfile_cleared: AtomicBool::new(false),
        read: AtomicBool::new(false),
        persisted: AtomicBool::new(false),
        adopted: AtomicBool::new(false),
        failure_noted: AtomicBool::new(false),
        started: tokio::sync::Notify::new(),
        dropped: tokio::sync::Notify::new(),
        blocker: Mutex::new(Some(receiver)),
        release: Mutex::new(Some(sender)),
    });
    *CURRENT.get_or_init(|| Mutex::new(None)).lock().unwrap() = Some(fixture.clone());
    fixture
}
pub(crate) struct Secret {
    value: String,
    fixture: Arc<Fixture>,
}
impl Deref for Secret {
    type Target = str;
    fn deref(&self) -> &str {
        &self.value
    }
}
impl Zeroize for Secret {
    fn zeroize(&mut self) {
        self.value.zeroize();
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        self.fixture
            .password_cleared
            .store(self.value.is_empty(), Ordering::SeqCst);
        self.fixture.password_drops.fetch_add(1, Ordering::SeqCst);
        self.fixture.dropped.notify_one();
    }
}
struct SecretBytes {
    value: Vec<u8>,
    fixture: Arc<Fixture>,
}
impl Deref for SecretBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.value
    }
}
impl DerefMut for SecretBytes {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.value
    }
}
impl Zeroize for SecretBytes {
    fn zeroize(&mut self) {
        self.value.zeroize();
    }
}
impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.fixture
            .keyfile_cleared
            .store(self.value.iter().all(|byte| *byte == 0), Ordering::SeqCst);
        self.fixture.keyfile_drops.fetch_add(1, Ordering::SeqCst);
        self.fixture.dropped.notify_one();
    }
}
async fn cleared(fixture: &Fixture, has_keyfile: bool) -> bool {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while fixture.password_drops.load(Ordering::SeqCst) == 0
            || (has_keyfile && fixture.keyfile_drops.load(Ordering::SeqCst) == 0)
        {
            fixture.dropped.notified().await;
        }
    })
    .await
    .expect("synthetic change credential was not released");
    assert_eq!(fixture.password_drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        fixture.keyfile_drops.load(Ordering::SeqCst),
        usize::from(has_keyfile)
    );
    let password = fixture.password_cleared.load(Ordering::SeqCst);
    let keyfile = !has_keyfile || fixture.keyfile_cleared.load(Ordering::SeqCst);
    eprintln!(
        "change cleanup: fault={:?},keyfile_present={has_keyfile},password_cleared={password},keyfile_cleared={keyfile}",
        fixture.fault
    );
    password && keyfile
}
mod tauri {
    pub type State<'a, T> = &'a T;
    pub mod async_runtime {
        pub use tokio::task::spawn_blocking;
    }
}
#[derive(Debug)]
pub(crate) struct VaultState;
#[derive(Default)]
pub(crate) struct VaultSession {
    owned_password: Option<Password>,
    owned_keyfile: Option<SecretBytes>,
    copied_password: String,
    copied_keyfile: Option<Vec<u8>>,
}
impl VaultSession {
    fn prepare_change(&self) -> Result<((), (), u64), String> {
        if current().fault == Fault::Plan {
            Err("synthetic plan error".into())
        } else {
            Ok(((), (), 1))
        }
    }
    fn ensure_master_key_expiry_token(&self, _: &str) -> Result<(), String> { Ok(()) }
    fn note_save_failure(&mut self) {
        current().failure_noted.store(true, Ordering::SeqCst);
    }
    fn close(&mut self) {
        if let Some(password) = self.owned_password.as_mut() {
            password.zeroize();
        }
        if let Some(keyfile) = self.owned_keyfile.as_mut() {
            keyfile.zeroize();
        }
        self.owned_password.take();
        self.owned_keyfile.take();
        self.copied_password.zeroize();
        self.copied_keyfile.zeroize();
    }
    // MASTER_COMPLETION
}
impl Drop for VaultSession {
    fn drop(&mut self) {
        self.close();
    }
}
fn check_adoption(password: &str, keyfile: Option<&[u8]>) {
    assert!(
        password == "synthetic-new",
        "synthetic password changed before adoption"
    );
    assert!(
        keyfile == Some(&[2, 5, 7][..]),
        "synthetic keyfile changed before adoption"
    );
    current().adopted.store(true, Ordering::SeqCst);
}
fn finish_adoption() -> Result<VaultState, String> {
    if current().fault == Fault::Snapshot {
        Err("synthetic snapshot error".into())
    } else {
        Ok(VaultState)
    }
}
pub(crate) struct VaultSessions;
impl VaultSessions {
    async fn acquire_persistence_async(&self) -> Result<(), String> {
        match current().fault {
            Fault::Permit => Err("synthetic permit error".into()),
            Fault::PermitPark => {
                current().started.notify_one();
                std::future::pending().await
            }
            _ => Ok(()),
        }
    }
    fn with_resolved_session_mut<T>(
        &self,
        active: &mut VaultSession,
        _: Option<&str>,
        operation: impl FnOnce(&mut VaultSession) -> Result<T, String>,
    ) -> Result<(String, T), String> {
        if current().fault == Fault::Resolve {
            return Err("synthetic resolve error".into());
        }
        Ok(("synthetic-session".into(), operation(active)?))
    }
    fn with_session_mut<T>(
        &self,
        active: &mut VaultSession,
        _: Option<&str>,
        operation: impl FnOnce(&mut VaultSession) -> Result<T, String>,
    ) -> Result<T, String> {
        if current().fault == Fault::Missing {
            return Err("synthetic closed session".into());
        }
        operation(active)
    }
}
mod vault {
    use super::*;
    pub enum SaveErrorClass {
        Persistent,
        Other,
    }
    pub fn classify_save_error(_: &str) -> SaveErrorClass {
        if current().fault == Fault::Persistent {
            SaveErrorClass::Persistent
        } else {
            SaveErrorClass::Other
        }
    }
    pub fn read_keyfile(_: Option<&Path>) -> Result<Option<SecretBytes>, String> {
        let fixture = current();
        if fixture.fault == Fault::Read {
            return Err("synthetic read error".into());
        }
        fixture.read.store(true, Ordering::SeqCst);
        Ok(Some(SecretBytes {
            value: vec![2, 5, 7],
            fixture,
        }))
    }
    pub fn persist_change(
        _: MasterChangeDatabase,
        password: &str,
        keyfile: Option<&[u8]>,
        _: &(),
    ) -> Result<MasterPersistResult, String> {
        assert!(password == "synthetic-new");
        assert!(keyfile == Some(&[2, 5, 7][..]));
        let fixture = current();
        match fixture.fault {
            Fault::WorkerPanic => panic!("synthetic change worker panic"),
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
            Fault::Persist | Fault::Persistent => return Err("synthetic persistence error".into()),
            _ => (),
        }
        fixture.persisted.store(true, Ordering::SeqCst);
        Ok(master_persist_success())
    }
}
// MASTER_HANDLER

fn password(fixture: Arc<Fixture>) -> Password {
    observed(Secret {
        value: "synthetic-new".into(),
        fixture,
    })
}
async fn run(fixture: Arc<Fixture>) -> Result<VaultState, String> {
    let session = Mutex::new(VaultSession::default());
    if fixture.fault == Fault::Poison {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = session.lock().unwrap();
            panic!("synthetic poisoned change lock");
        }));
        assert!(result.is_err());
    }
    change_master_key(
        &VaultSessions,
        &session,
        Some("synthetic-session".into()),
        password(fixture),
        Some("synthetic.key".into()),
        // EXPIRY_TOKEN_ARGUMENT
    )
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unpolled_and_keyfile_read_refusal_clear_the_new_password() {
    let fixture = setup(Fault::None);
    let session = Mutex::new(VaultSession::default());
    let future = change_master_key(
        &VaultSessions,
        &session,
        None,
        password(fixture.clone()),
        Some("synthetic.key".into()),
        // EXPIRY_TOKEN_ARGUMENT
    );
    drop(future);
    assert!(!fixture.read.load(Ordering::SeqCst));
    let unpolled = cleared(&fixture, false).await;
    let fixture = setup(Fault::Read);
    assert!(run(fixture.clone()).await.is_err());
    assert!(!fixture.read.load(Ordering::SeqCst));
    let read = cleared(&fixture, false).await;
    assert!(
        unpolled && read,
        "uncleared password: unpolled={unpolled},read={read}"
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn preflight_errors_and_cancelled_permit_clear_both_credentials() {
    let mut failures = Vec::new();
    for fault in [Fault::Permit, Fault::Resolve, Fault::Plan, Fault::Poison] {
        let fixture = setup(fault);
        assert!(run(fixture.clone()).await.is_err());
        assert!(fixture.read.load(Ordering::SeqCst));
        if !cleared(&fixture, true).await {
            failures.push(fault);
        }
    }
    let fixture = setup(Fault::PermitPark);
    let work = tokio::spawn(run(fixture.clone()));
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        fixture.started.notified(),
    )
    .await
    .unwrap();
    work.abort();
    assert!(work.await.unwrap_err().is_cancelled());
    if !cleared(&fixture, true).await {
        failures.push(Fault::PermitPark);
    }
    assert!(
        failures.is_empty(),
        "uncleared preflight pairs: {failures:?}"
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn worker_panic_and_cancelled_completion_clear_both_credentials() {
    let fixture = setup(Fault::WorkerPanic);
    assert!(run(fixture.clone()).await.is_err());
    let panic = cleared(&fixture, true).await;
    let fixture = setup(Fault::WorkerPark);
    let work = tokio::spawn(run(fixture.clone()));
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        fixture.started.notified(),
    )
    .await
    .unwrap();
    work.abort();
    assert!(work.await.unwrap_err().is_cancelled());
    fixture
        .release
        .lock()
        .unwrap()
        .take()
        .unwrap()
        .send(())
        .unwrap();
    let cancellation = cleared(&fixture, true).await;
    assert!(
        panic && cancellation,
        "uncleared worker pairs: panic={panic},cancelled={cancellation}"
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn success_and_existing_failure_controls_keep_adoption_and_cleanup() {
    for fault in [
        Fault::None,
        Fault::Persist,
        Fault::Persistent,
        Fault::Missing,
        Fault::Snapshot,
    ] {
        let fixture = setup(fault);
        let result = run(fixture.clone()).await;
        assert_eq!(result.is_ok(), fault == Fault::None);
        assert!(
            cleared(&fixture, true).await,
            "uncleared control: {fault:?}"
        );
        assert_eq!(
            fixture.failure_noted.load(Ordering::SeqCst),
            fault == Fault::Persistent
        );
        assert_eq!(
            fixture.adopted.load(Ordering::SeqCst),
            matches!(fault, Fault::None | Fault::Snapshot)
        );
        assert_eq!(
            fixture.persisted.load(Ordering::SeqCst),
            matches!(fault, Fault::None | Fault::Missing | Fault::Snapshot)
        );
    }
}
