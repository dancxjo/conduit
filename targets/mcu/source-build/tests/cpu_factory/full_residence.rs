//! Pre-existing generic media fixture, never evidence produced by this Play.
use conduit_resource_snapshot::append::{
    DurableRecordIndex, PreparedNorRecordResidence, RecordRead, RecordResidenceId,
    RecordResidenceRefusal,
};
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    Mutex, OnceLock,
};
static SELECTED: OnceLock<bool> = OnceLock::new();
static SEEDING: AtomicBool = AtomicBool::new(false);
pub static SEED_PROGRAMS: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_HASH: Mutex<Option<[u8; 32]>> = Mutex::new(None);
pub fn selected() -> bool {
    *SELECTED.get_or_init(|| std::env::var_os("CPU_FULL_RESIDENCE").is_some())
}
pub fn seeding() -> bool {
    SEEDING.load(SeqCst)
}
pub fn residence_id() -> RecordResidenceId {
    let mut hash = Sha256::new();
    hash.update(b"conduit.root/nor-record-residence@1");
    for field in ["cpu-factory/body", crate::authority::HOST] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field.as_bytes());
    }
    RecordResidenceId(hash.finalize().into())
}
fn payload(index: u32) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..12].copy_from_slice(b"PREEXISTING\0");
    bytes[12..].copy_from_slice(&index.to_le_bytes());
    bytes
}
pub fn prepopulate(
    storage: crate::equipment::RecordingStorage,
) -> crate::equipment::RecordingStorage {
    assert!(!crate::effects::enabled());
    assert!(!SEEDING.swap(true, SeqCst));
    struct Setup;
    impl Drop for Setup {
        fn drop(&mut self) {
            SEEDING.store(false, SeqCst);
        }
    }
    let setup = Setup;
    let mut residence = PreparedNorRecordResidence::prepare(
        storage,
        residence_id(),
        crate::config::RECORDING_PREFIX_BYTES,
        crate::config::RECORDING_SLOTS,
    )
    .ok()
    .unwrap();
    assert_eq!(residence.recover().unwrap().committed, 0);
    for index in 0..crate::config::RECORDING_SLOTS {
        assert_eq!(
            residence.append(&payload(index)).unwrap(),
            DurableRecordIndex(index)
        );
    }
    assert_eq!(
        residence.append(b"no-room"),
        Err(RecordResidenceRefusal::Capacity)
    );
    let recovered = residence.recover().unwrap();
    assert_eq!(recovered.committed, crate::config::RECORDING_SLOTS);
    assert_eq!(recovered.corrupt, 0);
    assert_eq!(recovered.unpublished_or_damaged, 0);
    assert_eq!(recovered.vacant_gaps, 0);
    let storage = residence.into_storage();
    storage.capture_fixture_media("full-residence-before.bin");
    *ORIGINAL_HASH.lock().unwrap() = Some(storage.digest());
    drop(setup);
    println!("CPU_PREEXISTING_MEDIA committed={} seed_program_units={} current_Source_records=0 bytes_sha256={:02x?}",recovered.committed,SEED_PROGRAMS.load(SeqCst),storage.digest());
    storage
}
pub fn verify_after_quiescence() {
    let storage = crate::equipment::RecordingStorage::from_retained(crate::effects::take_media());
    let digest = storage.digest();
    storage.capture_fixture_media("full-residence-after.bin");
    assert_eq!(Some(digest), *ORIGINAL_HASH.lock().unwrap());
    let mut residence = PreparedNorRecordResidence::prepare(
        storage,
        residence_id(),
        crate::config::RECORDING_PREFIX_BYTES,
        crate::config::RECORDING_SLOTS,
    )
    .ok()
    .unwrap();
    let recovered = residence.recover().unwrap();
    assert_eq!(recovered.committed, crate::config::RECORDING_SLOTS);
    assert_eq!(recovered.next_index, crate::config::RECORDING_SLOTS);
    assert_eq!(recovered.corrupt, 0);
    assert_eq!(recovered.unpublished_or_damaged, 0);
    assert_eq!(recovered.vacant_gaps, 0);
    for index in 0..crate::config::RECORDING_SLOTS {
        let RecordRead::Committed {
            payload: actual, ..
        } = residence.read(DurableRecordIndex(index)).unwrap()
        else {
            panic!("old committed record lost")
        };
        assert_eq!(actual, payload(index));
    }
    assert_eq!(crate::effects::PROGRAMS.load(SeqCst), 0);
    println!("CPU_FULL_MEDIA_PRESERVED old_records={} current_Source_records=0 programs_after_Start=0 original_bytes_hash_unchanged=true eviction=0",recovered.committed);
}
pub fn inspect_actual_append_output(output: &[u8]) {
    use conduit_resource_snapshot::append_result::{
        decode, DecodedAppendResult, APPEND_RESULT_BYTES,
    };
    let observed = (1..=APPEND_RESULT_BYTES as usize).find_map(|n| decode(&output[..n]).ok());
    assert_eq!(observed, Some(DecodedAppendResult::Capacity));
    println!("CPU_ACTUAL_APPEND_STATUS Capacity current_Source_records=0 assurance=typed-native-result-not-durable-ACK");
}
