//! Persistent equipment must not retain a preceding call's recognition artifacts.
use super::tests::{discover, fixture, pcm};
use super::*;
use std::{fs, os::unix::fs::PermissionsExt};

#[test]
fn completed_call_cannot_supply_previous_transcript_to_next_call() {
    let (root, executable, model) = fixture(
        r#"#!/bin/sh
out=
while [ $# -gt 0 ]; do
  if [ "$1" = --output-file ]; then out=$2; shift 2; else shift; fi
done
if [ ! -f "$0.first" ]; then
  printf 'First turn\n' > "${out}.txt"
  touch "$0.first"
fi
exit 0
"#,
    );
    let mut adapter = discover(&executable, &model)
        .initialize(WhisperLimits::clip_recognition(1, Duration::from_secs(2)))
        .unwrap();
    let audio = pcm(&[1, 2]);
    let language = crate::hosted_language::tests::request("language/english");
    assert!(adapter.recognize(&audio, &language, || false).is_ok());
    assert!(matches!(
        adapter.recognize(&audio, &language, || false),
        Err(WhisperFailure::ReadFailed)
    ));
    assert!(adapter.take_receipt().is_none());
    assert_eq!(fs::read_dir(&adapter.workspace).unwrap().count(), 0);
    assert_eq!(
        fs::metadata(&adapter.workspace)
            .unwrap()
            .permissions()
            .mode()
            & 0o077,
        0
    );
    drop(adapter);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn failing_or_cancelled_call_clears_old_evidence_and_retires_private_files() {
    let (root, executable, model) = fixture(
        r#"#!/bin/sh
out=
while [ $# -gt 0 ]; do
  if [ "$1" = --output-file ]; then out=$2; shift 2; else shift; fi
done
printf 'turn text\n' > "${out}.txt"
[ ! -f "$0.fail" ]
"#,
    );
    let mut adapter = discover(&executable, &model)
        .initialize(WhisperLimits::clip_recognition(1, Duration::from_secs(2)))
        .unwrap();
    let evidence = adapter.enable_evidence_text();
    let audio = pcm(&[1, 2]);
    let language = crate::hosted_language::tests::request("language/english");
    adapter.recognize(&audio, &language, || false).unwrap();
    assert!(!evidence.bytes().unwrap().is_empty());
    assert_eq!(fs::read_dir(&adapter.workspace).unwrap().count(), 0);
    fs::write(executable.with_extension("fail"), b"fail next call").unwrap();
    assert!(matches!(
        adapter.recognize(&audio, &language, || false),
        Err(WhisperFailure::ProviderLost)
    ));
    assert!(adapter.take_receipt().is_none());
    assert!(evidence.bytes().unwrap().is_empty());
    assert_eq!(fs::read_dir(&adapter.workspace).unwrap().count(), 0);
    assert!(matches!(
        adapter.recognize(&audio, &language, || true),
        Err(WhisperFailure::Cancelled)
    ));
    assert_eq!(fs::read_dir(&adapter.workspace).unwrap().count(), 0);
    drop(adapter);
    fs::remove_dir_all(root).unwrap();
}
