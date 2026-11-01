use super::*;
use conduit_core::{KindId, StateId};
use std::{process::Command, time::SystemTime};

const CHILD_MODE: &str = "CONDUIT_DURABLE_STATE_CHILD_MODE";
const CHILD_ROOT: &str = "CONDUIT_DURABLE_STATE_CHILD_ROOT";

fn binding() -> DurableStateBinding {
    DurableStateBinding {
        body: "body/notebook".into(),
        state: StateId::from("note/current"),
        value_kind: KindId::from("value/text"),
        maximum_value_bytes: conduit_data::MAXIMUM_DATA_TEXT_BYTES,
    }
}

fn temporary_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "conduit-durable-state-{label}-{}-{nonce}",
        std::process::id()
    ))
}

fn child(mode: &str, root: &Path) -> ! {
    let residence = FileDurableStateResidence::open(root, binding()).unwrap();
    match mode {
        "commit" => {
            residence
                .commit(1, b"published before abrupt loss")
                .unwrap();
        }
        "torn" => {
            residence
                .synchronize_candidate_without_publication(2, b"unpublished candidate")
                .unwrap();
        }
        _ => panic!("unknown child mode"),
    }
    std::process::abort()
}

fn spawn_abrupt(mode: &str, root: &Path) {
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("state_value::durable::residence::tests::fresh_process_abrupt_loss_recovers_only_published_generation")
        .arg("--nocapture")
        .env(CHILD_MODE, mode)
        .env(CHILD_ROOT, root)
        .status()
        .unwrap();
    assert!(!status.success(), "child must end by abrupt process loss");
}

#[test]
fn fresh_process_abrupt_loss_recovers_only_published_generation() {
    if let (Ok(mode), Ok(root)) = (std::env::var(CHILD_MODE), std::env::var(CHILD_ROOT)) {
        child(&mode, Path::new(&root));
    }
    let root = temporary_root("abrupt");
    spawn_abrupt("commit", &root);
    let residence = FileDurableStateResidence::open(&root, binding()).unwrap();
    assert_eq!(
        residence.recover().unwrap().0,
        RecoveryDisposition::Recovered { generation: 1 }
    );
    spawn_abrupt("torn", &root);
    let reopened = FileDurableStateResidence::open(&root, binding()).unwrap();
    let (disposition, encoded) = reopened.recover().unwrap();
    assert_eq!(
        disposition,
        RecoveryDisposition::Recovered { generation: 1 }
    );
    assert!(encoded.ends_with(b"published before abrupt loss"));
    fs::remove_dir_all(&root).unwrap();
}

fn fail_sync(stage: &'static str) {
    SYNC_FAILURE.with(|failure| failure.replace(Some(stage)));
    SYNC_EVENTS.with(|events| events.borrow_mut().clear());
}
fn sync_events() -> Vec<&'static str> {
    SYNC_EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut()))
}

#[test]
fn uncertain_publication_retry_requires_current_and_directory_barriers() {
    let root = temporary_root("uncertain-publication");
    let mut installed = InstalledDurableStateHost::open(&root, binding()).unwrap();
    fail_sync("journal");
    assert_eq!(installed.commit(b"A"), Err(DurableStateRefusal::Lost));
    assert_eq!(installed.generation, 0);
    assert_eq!(sync_events(), ["journal"]);
    assert_eq!(
        installed.residence.recover().unwrap().0,
        RecoveryDisposition::Recovered { generation: 1 }
    );
    fail_sync("current");
    assert_eq!(installed.commit(b"A"), Err(DurableStateRefusal::Lost));
    assert_eq!(installed.generation, 0);
    assert_eq!(sync_events(), ["current"]);
    fail_sync("journal");
    assert_eq!(installed.commit(b"A"), Err(DurableStateRefusal::Lost));
    assert_eq!(installed.generation, 0);
    assert_eq!(sync_events(), ["current", "journal"]);
    let committed = installed.commit(b"A").unwrap();
    assert_eq!(committed, receipt(1, digest(b"A")));
    assert_eq!(installed.generation, 1);
    assert_eq!(sync_events(), ["current", "journal"]);
    assert_eq!(installed.residence.commit(1, b"A").unwrap(), committed);
    assert_eq!(sync_events(), ["current", "journal"]);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn selected_root_parent_barrier_is_required_on_creation_and_existing_retry() {
    let root_path = temporary_root("selected-parent");
    fs::create_dir(&root_path).unwrap();
    let root = File::open(&root_path).unwrap();
    fail_sync("parent");
    assert!(matches!(
        FileDurableStateResidence::open_selected_root(&root, binding()),
        Err(DurableStateRefusal::Lost)
    ));
    assert_eq!(sync_events(), ["journal", "parent"]);
    assert!(root_path
        .join(FileDurableStateResidence::storage_key(&binding()).unwrap())
        .is_dir());
    fail_sync("parent");
    assert!(matches!(
        FileDurableStateResidence::open_selected_root(&root, binding()),
        Err(DurableStateRefusal::Lost)
    ));
    assert_eq!(sync_events(), ["journal", "parent"]);
    fail_sync("journal");
    assert!(matches!(
        FileDurableStateResidence::open_selected_root(&root, binding()),
        Err(DurableStateRefusal::Lost)
    ));
    assert_eq!(sync_events(), ["journal"]);
    let mut installed = InstalledDurableStateHost::open_selected_root(&root, binding()).unwrap();
    assert_eq!(sync_events(), ["journal", "parent"]);
    assert_eq!(installed.generation, 0);
    let renamed = root_path.with_extension("moved");
    fs::rename(&root_path, &renamed).unwrap();
    installed.commit(b"retained descriptor").unwrap();
    assert_eq!(installed.generation, 1);
    assert_eq!(sync_events(), ["journal"]);
    assert!(installed
        .residence
        .recover()
        .unwrap()
        .1
        .ends_with(b"retained descriptor"));
    fs::remove_dir_all(renamed).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn selected_root_refuses_symlink_journal() {
    let root_path = temporary_root("selected-symlink");
    fs::create_dir(&root_path).unwrap();
    let root = File::open(&root_path).unwrap();
    let name = FileDurableStateResidence::storage_key(&binding()).unwrap();
    std::os::unix::fs::symlink(&root_path, root_path.join(name)).unwrap();
    assert!(matches!(
        FileDurableStateResidence::open_selected_root(&root, binding()),
        Err(DurableStateRefusal::Lost)
    ));
    fs::remove_dir_all(root_path).unwrap();
}

#[test]
fn commit_is_idempotent_and_conflicts_are_exact() {
    let root = temporary_root("conflict");
    let residence = FileDurableStateResidence::open(&root, binding()).unwrap();
    let first = residence.commit(1, b"A").unwrap();
    assert_eq!(residence.commit(1, b"A").unwrap(), first);
    assert_eq!(
        residence.commit(1, b"B"),
        Err(DurableStateRefusal::ConflictingDigest)
    );
    assert_eq!(
        residence.commit(3, b"C"),
        Err(DurableStateRefusal::GenerationGap)
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn exact_recovery_refuses_drift_and_carries_the_full_text_bound() {
    let root = temporary_root("exact-four-kib");
    let mut installed = InstalledDurableStateHost::open(&root, binding()).unwrap();
    let value = vec![b'n'; conduit_data::MAXIMUM_DATA_TEXT_BYTES as usize];
    installed.commit(&value).unwrap();
    let metadata = installed.recover_metadata().unwrap();
    assert_eq!(installed.recover_exact(&metadata).unwrap(), value);

    installed.commit(b"new generation").unwrap();
    assert_eq!(
        installed.recover_exact(&metadata),
        Err(DurableStateRefusal::StaleRecovery)
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn residence_distinguishes_absent_corrupt_incompatible_and_lost() {
    let root = temporary_root("recovery-dispositions");
    let residence = FileDurableStateResidence::open(&root, binding()).unwrap();
    assert_eq!(residence.recover().unwrap().0, RecoveryDisposition::Absent);
    residence.commit(1, b"A").unwrap();
    fs::write(residence.current_path(), b"not a generation").unwrap();
    assert_eq!(residence.recover(), Err(DurableStateRefusal::Corrupt));

    let mut foreign_binding = binding();
    foreign_binding.body = "body/foreign".into();
    fs::write(
        residence.current_path(),
        encode_generation(&foreign_binding, 1, b"A").unwrap(),
    )
    .unwrap();
    assert_eq!(residence.recover(), Err(DurableStateRefusal::Incompatible));

    fs::remove_file(residence.current_path()).unwrap();
    fs::create_dir(residence.current_path()).unwrap();
    assert_eq!(residence.recover(), Err(DurableStateRefusal::Lost));
    fs::remove_dir_all(&root).unwrap();
}
