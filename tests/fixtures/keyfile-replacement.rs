//! Exact session replacement/close methods with Vec<u8> bound to an observed
//! live-owned buffer. Database/storage/snapshot/UUID boundaries are stubs.
//! The old and adopted new values are inspected before their Drop returns.
use std::collections::HashMap;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex, OnceLock,
};
use zeroize::Zeroize;

struct Observation {
    fail_snapshot: bool,
    old_drops: AtomicUsize,
    old_cleared: AtomicBool,
    new_drops: AtomicUsize,
    new_cleared: AtomicBool,
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
fn observation(fail_snapshot: bool) -> Arc<Observation> {
    let result = Arc::new(Observation {
        fail_snapshot,
        old_drops: AtomicUsize::new(0),
        old_cleared: AtomicBool::new(false),
        new_drops: AtomicUsize::new(0),
        new_cleared: AtomicBool::new(false),
    });
    *CURRENT.get_or_init(|| Mutex::new(None)).lock().unwrap() = Some(result.clone());
    result
}
pub(crate) struct ObservedVec<T: Zeroize + Default + PartialEq> {
    value: std::vec::Vec<T>,
    old: bool,
    observation: Arc<Observation>,
}
type Vec<T> = ObservedVec<T>;
impl ObservedVec<u8> {
    fn old() -> Self {
        Self {
            value: vec![3, 6, 9],
            old: true,
            observation: current(),
        }
    }
}
impl From<&[u8]> for ObservedVec<u8> {
    fn from(value: &[u8]) -> Self {
        Self {
            value: value.to_vec(),
            old: false,
            observation: current(),
        }
    }
}
impl<T: Zeroize + Default + PartialEq> Deref for ObservedVec<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.value
    }
}
impl<T: Zeroize + Default + PartialEq> Zeroize for ObservedVec<T> {
    fn zeroize(&mut self) {
        self.value.zeroize();
    }
}
impl<T: Zeroize + Default + PartialEq> Drop for ObservedVec<T> {
    fn drop(&mut self) {
        let cleared = self.value.iter().all(|value| value == &T::default());
        if self.old {
            self.observation
                .old_cleared
                .store(cleared, Ordering::SeqCst);
            self.observation.old_drops.fetch_add(1, Ordering::SeqCst);
        } else {
            self.observation
                .new_cleared
                .store(cleared, Ordering::SeqCst);
            self.observation.new_drops.fetch_add(1, Ordering::SeqCst);
        }
    }
}
mod uuid {
    #[derive(Default, PartialEq, Eq)]
    pub struct Uuid(u8);
    impl Uuid {
        pub fn new_v4() -> Self {
            Self(1)
        }
        pub fn nil() -> Self {
            Self(0)
        }
    }
}
#[derive(Default)]
pub(crate) struct Meta {
    master_key_changed: Option<chrono::NaiveDateTime>,
    master_key_change_force_once: Option<bool>,
}
pub(crate) struct Database {
    id: u8,
    meta: Meta,
}
impl Database {
    fn new(id: u8) -> Self {
        Self {
            id,
            meta: Meta::default(),
        }
    }
}
pub(crate) struct VaultState;
pub(crate) trait RemoteStorage {}
struct Storage;
impl RemoteStorage for Storage {}
#[derive(Clone, Copy)]
pub(crate) enum RemoteMode {
    InMemory,
}
pub(crate) struct PersistOutcome {
    hash: [u8; 32],
    mirror_warning: Option<String>,
}
impl From<[u8; 32]> for PersistOutcome {
    fn from(hash: [u8; 32]) -> Self {
        Self {
            hash,
            mirror_warning: None,
        }
    }
}
const REMOTE_URI_PREFIX: &str = "remote://";
#[allow(dead_code)]
struct RemoteTarget {
    storage: Arc<dyn RemoteStorage>,
    key: String,
    mode: RemoteMode,
    local_dir: PathBuf,
    backup_count: usize,
    backup_template: String,
    base_hash: [u8; 32],
}
fn now_iso() -> String {
    "synthetic-time".into()
}
#[derive(Default)]
pub(crate) struct VaultSession {
    db: Option<Database>,
    path: Option<String>,
    password: Option<String>,
    keyfile: Option<Vec<u8>>,
    remote: Option<RemoteTarget>,
    mirror_warning: Option<String>,
    history_lifetime: uuid::Uuid,
    dirty: bool,
    modified_at: String,
    save_failures: u32,
    cached_snapshot: Option<VaultState>,
    revision: u64,
    bridge_keys: HashMap<String, Vec<u8>>,
    rpc_keys: HashMap<String, Vec<u8>>,
    rpc_keys_expiry: Option<std::time::Instant>,
    pending_autotype_window: Option<String>,
    tcato_last_window: HashMap<String, String>,
}
impl VaultSession {
    fn require_db_mut(&mut self) -> Result<&mut Database, String> {
        self.db
            .as_mut()
            .ok_or_else(|| "synthetic closed database".into())
    }
    fn snapshot(&mut self) -> Result<VaultState, String> {
        if current().fail_snapshot {
            Err("synthetic snapshot refusal".into())
        } else {
            Ok(VaultState)
        }
    }
    fn adopt_persisted(&mut self, outcome: PersistOutcome) {
        self.mirror_warning = outcome.mirror_warning;
        if let Some(remote) = self.remote.as_mut() {
            remote.base_hash = outcome.hash;
        }
    }
    // SESSION_METHODS
}
impl Drop for VaultSession {
    fn drop(&mut self) {
        self.close_impl(true);
    }
}
// SESSION_HELPERS

#[derive(Clone, Copy, Debug)]
enum Boundary {
    Local,
    Remote,
    Master,
}
fn exercise(boundary: Boundary, old: bool, new: bool, fail_snapshot: bool) -> bool {
    let observation = observation(fail_snapshot);
    let mut session = VaultSession::default();
    session.db = Some(Database::new(1));
    session.path = Some("old.kdbx".into());
    session.password = Some("synthetic-old".into());
    session.keyfile = old.then(Vec::old);
    session.dirty = true;
    session.revision = 7;
    let next = new.then_some(&[2, 5, 7][..]);
    let result = match boundary {
        Boundary::Local => session.adopt_local(
            Database::new(2),
            Path::new("new.kdbx"),
            "synthetic-new",
            next.map(Vec::from),
        ),
        Boundary::Remote => session.adopt_remote(
            Database::new(2),
            Arc::new(Storage),
            "new.kdbx",
            "synthetic-new",
            next.map(Vec::from),
            RemoteMode::InMemory,
            Path::new("mirror"),
            3,
            "template",
            [8; 32],
        ),
        Boundary::Master => session.complete_change(
            "synthetic-new",
            next,
            7,
            [8; 32], // MASTER_TIMESTAMP_ARGUMENT
        ),
    };
    assert_eq!(result.is_err(), fail_snapshot);
    assert!(session.password.as_deref() == Some("synthetic-new"));
    assert!(session.keyfile.as_deref() == next);
    assert_eq!(
        session.db.as_ref().unwrap().id,
        if matches!(boundary, Boundary::Master) {
            1
        } else {
            2
        }
    );
    assert!(!session.dirty);
    assert_eq!(session.revision, 7);
    assert_eq!(
        observation.old_drops.load(Ordering::SeqCst),
        usize::from(old)
    );
    assert_eq!(observation.new_drops.load(Ordering::SeqCst), 0);
    let cleared = !old || observation.old_cleared.load(Ordering::SeqCst);
    eprintln!("replacement cleanup: boundary={boundary:?},old={old},new={new},snapshot_refused={fail_snapshot},old_cleared={cleared}");
    session.close_impl(true);
    assert_eq!(
        observation.new_drops.load(Ordering::SeqCst),
        usize::from(new)
    );
    assert!(!new || observation.new_cleared.load(Ordering::SeqCst));
    assert!(session.keyfile.is_none() && session.password.is_none() && session.db.is_none());
    cleared
}
const BOUNDARIES: [Boundary; 3] = [Boundary::Local, Boundary::Remote, Boundary::Master];
#[test]
fn replacing_with_a_keyfile_wipes_the_old_value_before_drop() {
    let failed: std::vec::Vec<_> = BOUNDARIES
        .into_iter()
        .filter(|boundary| !exercise(*boundary, true, true, false))
        .collect();
    assert!(
        failed.is_empty(),
        "old keyfiles were not cleared: {failed:?}"
    );
}
#[test]
fn replacing_without_a_keyfile_wipes_the_old_value_before_drop() {
    let failed: std::vec::Vec<_> = BOUNDARIES
        .into_iter()
        .filter(|boundary| !exercise(*boundary, true, false, false))
        .collect();
    assert!(
        failed.is_empty(),
        "removed keyfiles were not cleared: {failed:?}"
    );
}
#[test]
fn snapshot_refusals_after_adoption_still_wipe_the_old_keyfile() {
    let mut failed = std::vec::Vec::new();
    for boundary in BOUNDARIES {
        for new in [false, true] {
            if !exercise(boundary, true, new, true) {
                failed.push((boundary, new));
            }
        }
    }
    assert!(
        failed.is_empty(),
        "snapshot refusal left old keyfiles uncleared: {failed:?}"
    );
}
#[test]
fn absence_and_explicit_close_keep_adopted_values_and_cleanup() {
    for boundary in BOUNDARIES {
        for new in [false, true] {
            for fail in [false, true] {
                assert!(exercise(boundary, false, new, fail));
            }
        }
    }
    let observation = observation(false);
    let mut session = VaultSession::default();
    session.keyfile = Some(Vec::old());
    session.close_impl(true);
    assert_eq!(observation.old_drops.load(Ordering::SeqCst), 1);
    assert!(observation.old_cleared.load(Ordering::SeqCst));
}
