use super::*;
use std::{
    fs::OpenOptions,
    io::{Seek, SeekFrom, Write},
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
    let file = selected(source.as_bytes());
    let mut alias = file.try_clone().unwrap();
    let captured =
        OwnedSourceCheckInput::from_file(file, 4096, "owner/from-root", 7, "main", context())
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
    let expected = captured.request().clone();
    alias.rewind().unwrap();
    alias.set_len(0).unwrap();
    alias.write_all(b"different bytes after capture").unwrap();
    assert_eq!(captured.source(), source);
    let probe = crate::allocation_probe::begin();
    let request = captured.into_request();
    assert_eq!(probe.finish(), 0);
    assert_eq!(request, expected);
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
fn root_handoff_moves_exact_capture_without_allocating_or_reopening() {
    let captured = OwnedSourceCheckInput::from_file(
        selected(b"plot candidate {}"),
        4096,
        "owner",
        1,
        "candidate",
        context(),
    )
    .unwrap();
    let pointer = captured.source().as_ptr();
    let expected = captured.request().clone();
    let probe = crate::allocation_probe::begin();
    let (source, request) = captured.into_source_and_request();
    assert_eq!(probe.finish(), 0);
    assert_eq!(source.as_ptr(), pointer);
    assert_eq!(request, expected);
    assert_eq!(
        request.snapshot.source_document_id,
        canonical_source_document_id(&source).0
    );
}

#[test]
fn retained_source_accounting_includes_moved_context_spare_capacity() {
    let mut context = context();
    context.catalog.reserve(65_536);
    let spare = context.catalog.capacity();
    let source =
        OwnedSourceCheckInput::from_file(selected(b"x"), 8, "owner", 1, "main", context).unwrap();
    let bytes = source.owned_backing_bytes().unwrap();
    assert!(bytes >= core::mem::size_of::<OwnedSourceCheckInput>() + spare + 2);
    assert!(bytes > source.source().len() + 65_536);
    let probe = crate::allocation_probe::begin();
    assert_eq!(source.owned_backing_bytes(), Some(bytes));
    assert_eq!(probe.finish(), 0);
}
