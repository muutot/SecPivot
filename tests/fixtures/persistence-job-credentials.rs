//! Complete production job declarations, factories and consumers with observed
//! credential payload types. Database/KDF/storage boundaries are stubs. Tokio
//! queue cancellation and running workers are real; only owned values are read.
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc, Mutex, OnceLock,
};
#[allow(unused_imports)]
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Boundary {
    None,
    Key,
    Get,
    Parse,
    Persist,
    Mirror,
    Merge,
    Panic,
    Park,
}
struct Observation {
    boundary: Boundary,
    drops: Mutex<Vec<(&'static str, bool)>>,
    entered: AtomicUsize,
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
fn setup(boundary: Boundary) -> Arc<Observation> {
    let (sender, receiver) = mpsc::channel();
    let result = Arc::new(Observation {
        boundary,
        drops: Mutex::new(Vec::new()),
        entered: AtomicUsize::new(0),
        started: tokio::sync::Notify::new(),
        dropped: tokio::sync::Notify::new(),
        receiver: Mutex::new(Some(receiver)),
        release: Mutex::new(Some(sender)),
    });
    *CURRENT.get_or_init(|| Mutex::new(None)).lock().unwrap() = Some(result.clone());
    result
}
pub trait Payload: Clone + Zeroize {
    type View: ?Sized;
    fn view(&self) -> &Self::View;
    fn cleared(&self) -> bool;
    fn kind() -> &'static str;
}
impl Payload for String {
    type View = str;
    fn view(&self) -> &str {
        self
    }
    fn cleared(&self) -> bool {
        self.is_empty()
    }
    fn kind() -> &'static str {
        "password"
    }
}
impl Payload for Vec<u8> {
    type View = [u8];
    fn view(&self) -> &[u8] {
        self
    }
    fn cleared(&self) -> bool {
        self.iter().all(|b| *b == 0)
    }
    fn kind() -> &'static str {
        "keyfile"
    }
}
pub struct Observed<T: Payload> {
    value: T,
    tracked: bool,
    observation: Arc<Observation>,
}
type ObservedString = Observed<String>;
type ObservedBytes = Observed<Vec<u8>>;
impl<T: Payload> Clone for Observed<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            tracked: true,
            observation: self.observation.clone(),
        }
    }
}
impl<T: Payload> Deref for Observed<T> {
    type Target = T::View;
    fn deref(&self) -> &Self::Target {
        self.value.view()
    }
}
impl<T: Payload> Zeroize for Observed<T> {
    fn zeroize(&mut self) {
        self.value.zeroize();
    }
}
impl<T: Payload> Drop for Observed<T> {
    fn drop(&mut self) {
        if self.tracked {
            // Observe before cleanup while the payload is still owned. Wipe the
            // synthetic test value afterwards without changing the observation.
            self.observation
                .drops
                .lock()
                .unwrap()
                .push((T::kind(), self.value.cleared()));
            self.observation.dropped.notify_one();
        }
        self.value.zeroize();
    }
}
#[derive(Clone)]
pub(crate) struct Database(u8);
#[derive(Clone)]
struct DatabaseKey;
impl Database {
    fn parse(_: &[u8], _: DatabaseKey) -> Result<Self, String> {
        fail(Boundary::Parse)?;
        Ok(Self(1))
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemoteMode {
    SaveLocal,
}
pub(crate) trait RemoteStorage: Send + Sync {
    fn get(&self, key: &str) -> Result<Vec<u8>, String>;
}
struct Storage;
impl RemoteStorage for Storage {
    fn get(&self, _: &str) -> Result<Vec<u8>, String> {
        fail(Boundary::Get)?;
        Ok(vec![1])
    }
}
#[derive(Clone)]
pub(crate) enum SaveTarget {
    Local(PathBuf),
    Remote {
        storage: Arc<dyn RemoteStorage>,
        key: String,
        mode: RemoteMode,
        local_dir: PathBuf,
        backup_count: usize,
        backup_template: String,
        base_hash: [u8; 32],
    },
}
pub(crate) struct PersistOutcome {
    hash: [u8; 32],
    mirror_warning: Option<String>,
}
pub(crate) struct RemoteRefreshResult {
    db: Database,
    base_hash: [u8; 32],
}
pub(crate) struct RemoteMergeResult {
    db: Database,
    base_hash: [u8; 32],
    mirror_warning: Option<String>,
}
#[derive(Debug)]
pub(crate) struct RemoteMergeError {
    message: String,
    persist_failure: bool,
}
const REMOTE_CHANGED_MARKER: &str = "synthetic conflict";
fn fail(boundary: Boundary) -> Result<(), String> {
    if current().boundary == boundary {
        Err("synthetic boundary refusal".into())
    } else {
        Ok(())
    }
}
fn build_database_key(password: &str, keyfile: Option<&[u8]>) -> Result<DatabaseKey, String> {
    assert!(password == "synthetic-only");
    assert!(keyfile.is_none() || keyfile == Some(&[4, 8, 12][..]));
    let observation = current();
    if observation.boundary == Boundary::Panic {
        panic!("synthetic KDF boundary panic");
    }
    if observation.boundary == Boundary::Park {
        observation.started.notify_one();
        observation
            .receiver
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .recv()
            .unwrap();
        assert!(password == "synthetic-only");
        assert!(keyfile.is_none() || keyfile == Some(&[4, 8, 12][..]));
    }
    fail(Boundary::Key)?;
    Ok(DatabaseKey)
}
fn persist_snapshot(
    db: &Database,
    _: &DatabaseKey,
    _: &SaveTarget,
    _: bool,
) -> Result<PersistOutcome, String> {
    assert_eq!(db.0, 1);
    fail(Boundary::Persist)?;
    Ok(PersistOutcome {
        hash: [7; 32],
        mirror_warning: None,
    })
}
fn classify_open_error(error: String) -> String {
    error
}
fn remote_key_basename(key: &str) -> String {
    key.into()
}
fn write_local_copy(_: &Path, _: &str, _: &[u8], _: usize, _: &str) -> Result<(), String> {
    fail(Boundary::Mirror)
}
mod merge {
    use super::*;
    pub fn merge_databases(left: &Database, right: &Database) -> Result<Database, String> {
        fail(Boundary::Merge)?;
        assert_eq!(left.0, right.0);
        Ok(left.clone())
    }
}
mod persist {
    use super::*;
    // JOB_DECLARATIONS
    // WIPE_HELPERS
    // PERSISTENCE_CONSUMERS
}
use persist::{RemoteMergeJob, RemoteRefreshJob, SaveJob};
struct RemoteContext {
    storage: Arc<dyn RemoteStorage>,
    key: String,
    mode: RemoteMode,
    local_dir: PathBuf,
    backup_count: usize,
    backup_template: String,
}
struct VaultSession {
    db: Database,
    password: ObservedString,
    keyfile: Option<ObservedBytes>,
    remote: Option<RemoteContext>,
    revision: u64,
}
struct DatabaseSettingsPatch {
    kdf: Option<String>,
    cipher: Option<String>,
    compression: Option<String>,
}
fn parse_templates_group(_: &DatabaseSettingsPatch) -> Result<(), String> {
    Ok(())
}
fn apply_database_meta_patch(_: &mut Database, _: &DatabaseSettingsPatch, _: ()) {}
fn apply_kdf(_: &mut Database, _: &str) -> Result<(), String> {
    Ok(())
}
fn apply_cipher(_: &mut Database, _: &str) -> Result<(), String> {
    Ok(())
}
fn apply_compression(_: &mut Database, _: &str) -> Result<(), String> {
    Ok(())
}
impl VaultSession {
    fn new(has_keyfile: bool) -> Self {
        let observation = current();
        Self {
            db: Database(1),
            password: Observed {
                value: "synthetic-only".into(),
                tracked: false,
                observation: observation.clone(),
            },
            keyfile: has_keyfile.then(|| Observed {
                value: vec![4, 8, 12],
                tracked: false,
                observation,
            }),
            remote: Some(RemoteContext {
                storage: Arc::new(Storage),
                key: "synthetic.kdbx".into(),
                mode: RemoteMode::SaveLocal,
                local_dir: PathBuf::from("synthetic"),
                backup_count: 0,
                backup_template: "synthetic".into(),
            }),
            revision: 42,
        }
    }
    fn ensure_writable(&self) -> Result<(), String> {
        Ok(())
    }
    fn require_password(&self) -> Result<&ObservedString, String> {
        Ok(&self.password)
    }
    fn require_db(&self) -> Result<&Database, String> {
        Ok(&self.db)
    }
    fn prepare_change(&self) -> Result<(Database, SaveTarget, u64), String> {
        Ok((
            self.db.clone(),
            SaveTarget::Local(PathBuf::from("synthetic.kdbx")),
            self.revision,
        ))
    }
    // JOB_FACTORIES
}
#[derive(Clone, Copy, Debug)]
enum Preparation {
    Save,
    SaveAs,
    Settings,
    Refresh,
    Merge,
}
const PREPARATIONS: [Preparation; 5] = [
    Preparation::Save,
    Preparation::SaveAs,
    Preparation::Settings,
    Preparation::Refresh,
    Preparation::Merge,
];
enum Job {
    Save(SaveJob),
    Refresh(RemoteRefreshJob),
    Merge(RemoteMergeJob),
}
fn prepare(session: &VaultSession, preparation: Preparation, has_keyfile: bool) -> Job {
    let job = match preparation {
        Preparation::Save => Job::Save(session.prepare_save(true).unwrap()),
        Preparation::SaveAs => Job::Save(
            session
                .prepare_save_as(Path::new("synthetic-as.kdbx"))
                .unwrap(),
        ),
        Preparation::Settings => Job::Save(
            session
                .prepare_database_settings_update(&DatabaseSettingsPatch {
                    kdf: None,
                    cipher: None,
                    compression: None,
                })
                .unwrap(),
        ),
        Preparation::Refresh => Job::Refresh(session.prepare_remote_refresh().unwrap()),
        Preparation::Merge => Job::Merge(session.prepare_remote_merge().unwrap()),
    };
    let (password, keyfile, revision): (&str, Option<&[u8]>, u64) = match &job {
        Job::Save(job) => (&*job.password, job.keyfile.as_deref(), job.revision),
        Job::Refresh(job) => (&*job.password, job.keyfile.as_deref(), job.revision),
        Job::Merge(job) => (&*job.password, job.keyfile.as_deref(), job.revision),
    };
    assert!(password == "synthetic-only");
    assert_eq!(keyfile.is_some(), has_keyfile);
    assert!(keyfile.is_none() || keyfile == Some(&[4, 8, 12][..]));
    assert_eq!(revision, 42);
    assert!(session.password.deref() == "synthetic-only");
    assert_eq!(session.keyfile.is_some(), has_keyfile);
    assert!(current().drops.lock().unwrap().is_empty());
    job
}
fn consume(job: Job) -> Result<(), String> {
    current().entered.fetch_add(1, Ordering::SeqCst);
    match job {
        Job::Save(job) => persist::persist_save_with_db(job).map(|(db, outcome)| {
            assert_eq!(db.0, 1);
            assert_eq!(outcome.hash, [7; 32]);
        }),
        Job::Refresh(job) => persist::persist_remote_refresh(job).map(|outcome| {
            assert_eq!(outcome.db.0, 1);
            assert_eq!(outcome.base_hash, [0; 32]);
        }),
        Job::Merge(job) => persist::persist_remote_merge(job)
            .map(|outcome| {
                assert_eq!(outcome.db.0, 1);
                assert_eq!(outcome.base_hash, [7; 32]);
                assert!(outcome.mirror_warning.is_none());
            })
            .map_err(|error| {
                assert_eq!(
                    error.persist_failure,
                    current().boundary == Boundary::Persist
                );
                error.message
            }),
    }
}
fn cleared(observation: &Observation, preparation: Preparation, has_keyfile: bool) -> bool {
    let drops = observation.drops.lock().unwrap();
    assert_eq!(drops.len(), if has_keyfile { 2 } else { 1 });
    assert_eq!(
        drops.iter().filter(|(kind, _)| *kind == "password").count(),
        1
    );
    assert_eq!(
        drops.iter().filter(|(kind, _)| *kind == "keyfile").count(),
        usize::from(has_keyfile)
    );
    let bad = drops.iter().filter(|(_, clear)| !clear).count();
    eprintln!("job credentials: prep={preparation:?},factor={has_keyfile},boundary={:?},values={},uncleared={bad}", observation.boundary, drops.len());
    bad == 0
}
#[test]
fn discarded_jobs_clear_their_cloned_credentials() {
    let mut failures = Vec::new();
    for preparation in PREPARATIONS {
        for factor in [false, true] {
            let observation = setup(Boundary::None);
            let session = VaultSession::new(factor);
            drop(prepare(&session, preparation, factor));
            assert_eq!(observation.entered.load(Ordering::SeqCst), 0);
            if !cleared(&observation, preparation, factor) {
                failures.push((preparation, factor));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "uncleared discarded jobs: {failures:?}"
    );
}
#[test]
fn completed_and_refused_consumers_clear_their_credentials() {
    for preparation in PREPARATIONS {
        let boundaries: &[Boundary] = match preparation {
            Preparation::Save | Preparation::SaveAs | Preparation::Settings => {
                &[Boundary::None, Boundary::Key, Boundary::Persist]
            }
            Preparation::Refresh => &[
                Boundary::None,
                Boundary::Get,
                Boundary::Key,
                Boundary::Parse,
                Boundary::Mirror,
            ],
            Preparation::Merge => &[
                Boundary::None,
                Boundary::Key,
                Boundary::Get,
                Boundary::Parse,
                Boundary::Merge,
                Boundary::Persist,
            ],
        };
        for boundary in boundaries {
            for factor in [false, true] {
                let observation = setup(*boundary);
                let session = VaultSession::new(factor);
                assert_eq!(
                    consume(prepare(&session, preparation, factor)).is_ok(),
                    *boundary == Boundary::None
                );
                assert!(cleared(&observation, preparation, factor));
                assert!(session.password.deref() == "synthetic-only");
            }
        }
    }
}
#[test]
fn panicking_consumers_clear_their_credentials() {
    let mut failures = Vec::new();
    for preparation in PREPARATIONS {
        for factor in [false, true] {
            let observation = setup(Boundary::Panic);
            let session = VaultSession::new(factor);
            let job = prepare(&session, preparation, factor);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| consume(job))).is_err()
            );
            if !cleared(&observation, preparation, factor) {
                failures.push((preparation, factor));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "uncleared panicking jobs: {failures:?}"
    );
}
#[test]
fn queued_worker_abort_clears_unexecuted_job_credentials() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut failures = Vec::new();
        for preparation in PREPARATIONS {
            for factor in [false, true] {
                let observation = setup(Boundary::None);
                let session = VaultSession::new(factor);
                let job = prepare(&session, preparation, factor);
                let (release, wait) = mpsc::channel();
                let started = Arc::new(tokio::sync::Notify::new());
                let worker_started = started.clone();
                let blocker = tokio::task::spawn_blocking(move || {
                    worker_started.notify_one();
                    wait.recv().unwrap();
                });
                tokio::time::timeout(std::time::Duration::from_secs(2), started.notified())
                    .await
                    .unwrap();
                let worker = tokio::task::spawn_blocking(move || consume(job));
                worker.abort();
                release.send(()).unwrap();
                blocker.await.unwrap();
                assert!(worker.await.unwrap_err().is_cancelled());
                assert_eq!(observation.entered.load(Ordering::SeqCst), 0);
                if !cleared(&observation, preparation, factor) {
                    failures.push((preparation, factor));
                }
            }
        }
        assert!(failures.is_empty(), "uncleared queued jobs: {failures:?}");
    });
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cancelling_running_workers_keeps_credentials_until_completion() {
    for preparation in PREPARATIONS {
        for factor in [false, true] {
            let observation = setup(Boundary::Park);
            let session = VaultSession::new(factor);
            let job = prepare(&session, preparation, factor);
            let worker = tokio::task::spawn_blocking(move || consume(job));
            tokio::time::timeout(
                std::time::Duration::from_secs(2),
                observation.started.notified(),
            )
            .await
            .unwrap();
            worker.abort();
            drop(worker);
            assert!(observation.drops.lock().unwrap().is_empty());
            observation
                .release
                .lock()
                .unwrap()
                .take()
                .unwrap()
                .send(())
                .unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                loop {
                    if observation.drops.lock().unwrap().len() == if factor { 2 } else { 1 } {
                        break;
                    }
                    observation.dropped.notified().await;
                }
            })
            .await
            .unwrap();
            assert!(cleared(&observation, preparation, factor));
        }
    }
}
