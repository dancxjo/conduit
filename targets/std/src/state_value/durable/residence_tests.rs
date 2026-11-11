//! Synchronized publication and finite residence profile proofs.

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

#[test]
fn imported_generation_is_idempotent_and_allows_the_next_commit() {
    let root = tempfile::tempdir().unwrap();
    let residence = FileDurableStateResidence::open(root.path(), binding()).unwrap();
    residence.import_generation(5, b"migrated").unwrap();
    residence.import_generation(5, b"migrated").unwrap();
    assert_eq!(
        residence.import_generation(5, b"different"),
        Err(DurableStateRefusal::ConflictingDigest)
    );
    residence.commit(6, b"next episode").unwrap();
    residence.import_generation(5, b"migrated").unwrap();
    assert_eq!(
        residence.recover_value().unwrap(),
        (
            RecoveryDisposition::Recovered { generation: 6 },
            Some(b"next episode".to_vec())
        )
    );
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

#[test]
fn application_residence_admits_its_prepared_bound_without_widening_the_default_profile() {
    let root =
        std::env::temp_dir().join(format!("conduit-residence-profile-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let mut selected = binding();
    selected.maximum_value_bytes = 8192;
    assert!(matches!(
        FileDurableStateResidence::open(&root, selected.clone()),
        Err(DurableStateRefusal::InvalidBinding)
    ));
    let residence =
        FileDurableStateResidence::open_prepared(&root, selected.clone(), 8192).unwrap();
    let value = vec![b'x'; 8192];
    residence.commit(1, &value).unwrap();
    assert_eq!(
        residence.recover_value().unwrap(),
        (
            RecoveryDisposition::Recovered { generation: 1 },
            Some(value)
        )
    );
    selected.maximum_value_bytes = 8193;
    assert!(matches!(
        FileDurableStateResidence::open_prepared(&root, selected, 8193),
        Err(DurableStateRefusal::InvalidBinding)
    ));
    std::fs::remove_dir_all(root).unwrap();
}
