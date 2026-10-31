use super::*;
use conduit_environment_runtime::environmental_reading::source_observation::{
    PreparedObservationCodec, SourceFailure, SourceObservation, SourceOutcome,
};
fn scope() -> CollectorScope {
    CollectorScope {
        host: "host/test".into(),
        boot: "boot/test".into(),
    }
}
fn source() -> SourceSession {
    SourceSession {
        source: "equipment/green".into(),
        session: "base/acquisition-1".into(),
    }
}
fn execution() -> ExecutionProvenance {
    ExecutionProvenance {
        plan: "plan/actual".into(),
        play: "play/actual".into(),
        source: "source/actual".into(),
        checked: "checked/actual".into(),
        expanded: "expanded/actual".into(),
    }
}
fn bytes(ms: u64) -> Vec<u8> {
    let c = ObservationContract::prepare().unwrap();
    PreparedObservationCodec::prepare(&c)
        .unwrap()
        .encode(SourceObservation {
            observed_at_host_boot_ms: ms,
            outcome: SourceOutcome::Unavailable(SourceFailure::CaptureTimeout),
        })
        .unwrap()
        .to_vec()
}
fn directory() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::temp_dir().join(format!(
        "conduit-source-history-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}
#[test]
fn recovery_preserves_exact_bytes_and_bounded_window() {
    let dir = directory();
    let mut store = SourceHistoryStore::open(&dir, scope(), 18).unwrap();
    for seq in 1..=18 {
        store
            .append_committed(source(), seq, execution(), &bytes(seq))
            .unwrap();
    }
    assert_eq!(store.series()[0].records.len(), 16);
    assert_eq!(store.series()[0].samples_outside_view, 2);
    assert!(matches!(
        store.append_committed(source(), 19, execution(), &bytes(19)),
        Err(Error::Capacity)
    ));
    drop(store);
    let recovered = SourceHistoryStore::open(&dir, scope(), 18).unwrap();
    assert_eq!(recovered.retained_records(), 18);
    assert_eq!(
        recovered.series()[0].records.front().unwrap().observation,
        bytes(3)
    );
    drop(recovered);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn foreign_clock_duplicate_sequence_and_invalid_value_do_not_advance() {
    let dir = directory();
    let mut store = SourceHistoryStore::open(&dir, scope(), 10).unwrap();
    assert!(SourceHistoryStore::open(&dir, scope(), 10).is_err());
    store
        .append_committed(source(), 1, execution(), &bytes(10))
        .unwrap();
    assert!(matches!(
        store.append_committed(source(), 1, execution(), &bytes(11)),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        store.append_committed(source(), 2, execution(), &bytes(9)),
        Err(Error::ClockReversed)
    ));
    assert!(matches!(
        store.append_committed(source(), 2, execution(), b"junk"),
        Err(Error::Invalid)
    ));
    assert_eq!(store.retained_records(), 1);
    drop(store);
    let mut foreign = scope();
    foreign.boot = "boot/other".into();
    assert!(matches!(
        SourceHistoryStore::open(&dir, foreign, 10),
        Err(Error::ForeignScope)
    ));
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn interrupted_pending_is_ignored_but_missing_committed_sequence_refuses() {
    let dir = directory();
    let mut store = SourceHistoryStore::open(&dir, scope(), 10).unwrap();
    store
        .append_committed(source(), 1, execution(), &bytes(10))
        .unwrap();
    drop(store);
    fs::write(dir.join(".pending"), b"interrupted").unwrap();
    let store = SourceHistoryStore::open(&dir, scope(), 10).unwrap();
    assert_eq!(store.retained_records(), 1);
    drop(store);
    fs::rename(
        dir.join("0000000000000000001.json"),
        dir.join("0000000000000000002.json"),
    )
    .unwrap();
    assert!(SourceHistoryStore::open(&dir, scope(), 10).is_err());
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn sequence_is_per_session_and_fifth_session_refuses_without_bridging() {
    let dir = directory();
    let mut store = SourceHistoryStore::open(&dir, scope(), 20).unwrap();
    for i in 0..4 {
        let mut src = source();
        src.session = format!("base/session-{i}");
        store
            .append_committed(src.clone(), 1, execution(), &bytes(i))
            .unwrap();
        assert_eq!(store.last_sequence(&src), Some(1));
    }
    let mut fifth = source();
    fifth.session = "base/session-5".into();
    assert!(matches!(
        store.append_committed(fifth, 1, execution(), &bytes(5)),
        Err(Error::Capacity)
    ));
    drop(store);
    let store = SourceHistoryStore::open(&dir, scope(), 20).unwrap();
    assert_eq!(store.series().len(), 4);
    assert_eq!(store.retained_records(), 4);
    drop(store);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn occupied_sequence_never_replaces_bytes_and_requires_recovery() {
    let dir = directory();
    let mut store = SourceHistoryStore::open(&dir, scope(), 10).unwrap();
    // Simulate contrary external filesystem evidence after admission/open.
    let occupied = dir.join("0000000000000000001.json");
    fs::write(&occupied, b"original-evidence").unwrap();
    assert!(matches!(
        store.append_committed(source(), 1, execution(), &bytes(10)),
        Err(Error::Io(_))
    ));
    assert_eq!(fs::read(&occupied).unwrap(), b"original-evidence");
    assert!(matches!(
        store.append_committed(source(), 1, execution(), &bytes(10)),
        Err(Error::Uncertain)
    ));
    assert_eq!(store.retained_records(), 0);
    drop(store);
    assert!(SourceHistoryStore::open(&dir, scope(), 10).is_err());
    fs::remove_dir_all(dir).unwrap();
}
