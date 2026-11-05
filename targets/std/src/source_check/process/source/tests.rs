use super::*;
use std::{
    fs::OpenOptions,
    io::{Cursor, Write},
    sync::atomic::{AtomicU64, Ordering},
};
fn selected(bytes: &[u8]) -> File {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "conduit-source-selection-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    file.write_all(bytes).unwrap();
    std::fs::remove_file(path).unwrap();
    file
}
fn context() -> CheckContextRevision {
    CheckContextRevision {
        grammar_edition: "conduitese-v1".into(),
        catalog: "fixture/catalog@1".into(),
        kinds_fores: "fixture/kinds@1".into(),
        types_laws: "fixture/types@1".into(),
        backs: "fixture/backs@1".into(),
    }
}
#[test]
fn immutable_source_comes_from_the_actual_unlinked_descriptor_without_parsing() {
    let source = "# α 🦀\nunfinished {";
    let captured = OwnedSourceCheckInput::from_file(
        selected(source.as_bytes()),
        4096,
        "owner/from-root",
        7,
        "main",
        context(),
    )
    .unwrap();
    assert_eq!(captured.source(), source);
    assert_eq!(
        captured.request().snapshot.source_document_id,
        canonical_source_document_id(source).as_str()
    );
    assert_eq!(
        captured.request().snapshot.sha256,
        format!("{:x}", Sha256::digest(source.as_bytes()))
    );
    assert_eq!(captured.request().snapshot.owner, "owner/from-root");
    assert_eq!(captured.request().snapshot.generation, 7);
    assert_eq!(captured.request().entry, "main");
    let empty =
        OwnedSourceCheckInput::from_file(selected(&[]), 1, "owner", 1, "main", context()).unwrap();
    assert!(empty.source().is_empty());
}
#[test]
fn invalid_scope_refuses_before_read_or_seek_and_cannot_select_a_path() {
    for (maximum, owner, generation) in [(0, "owner", 1), (4096, "", 1), (4096, "owner", 0)] {
        let mut file = selected(b"abcdefgh");
        file.seek(SeekFrom::Start(3)).unwrap();
        let mut alias = file.try_clone().unwrap();
        assert!(OwnedSourceCheckInput::from_file(
            file,
            maximum,
            owner,
            generation,
            "main",
            context()
        )
        .is_err());
        assert_eq!(alias.stream_position().unwrap(), 3);
    }
}
#[test]
fn nonregular_oversized_and_non_utf8_sources_refuse_distinctly() {
    assert!(matches!(
        OwnedSourceCheckInput::from_file(
            File::open("/dev/null").unwrap(),
            8,
            "owner",
            1,
            "main",
            context()
        ),
        Err(SourceFileRefusal::NotRegular)
    ));
    assert!(matches!(
        OwnedSourceCheckInput::from_file(selected(b"123456789"), 8, "owner", 1, "main", context()),
        Err(SourceFileRefusal::Bytes)
    ));
    assert!(matches!(
        OwnedSourceCheckInput::from_file(selected(&[0xff]), 8, "owner", 1, "main", context()),
        Err(SourceFileRefusal::Utf8)
    ));
}
#[test]
fn changed_extent_cannot_grow_storage_and_interruption_is_not_an_unbounded_retry() {
    let mut growing = Cursor::new([1u8; 1024]);
    assert!(matches!(
        capture_extent(&mut growing, 2),
        Err(SourceFileRefusal::ExtentChanged)
    ));
    assert_eq!(
        growing.position(),
        3,
        "only the original extent plus sentinel is read"
    );
    assert!(matches!(
        capture_extent(Cursor::new([1u8]), 2),
        Err(SourceFileRefusal::ExtentChanged)
    ));
    struct Interrupted(usize);
    impl Read for Interrupted {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            self.0 += 1;
            Err(std::io::ErrorKind::Interrupted.into())
        }
    }
    let mut interrupted = Interrupted(0);
    assert!(matches!(
        capture_extent(&mut interrupted, 2),
        Err(SourceFileRefusal::Read(_))
    ));
    assert_eq!(interrupted.0, 1);
}
