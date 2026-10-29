use super::*;

#[test]
fn transport_loss_after_durable_batch_resumes_without_duplicate_commitment() {
    let key = SigningKey::from_bytes(&[31; 32]);
    let scope = [6; 96];
    let mut collector =
        Collector::from_authenticated_scope(scope, &key.verifying_key().to_bytes()).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "conduit-history-source-loss-{}",
        std::process::id()
    ));
    let record = ObservedRecord {
        sequence: 1,
        observed_at_boot_ms: 731,
        disposition: conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
        quantities: None,
    }
    .encode()
    .unwrap();
    let batch = signed(&key, scope, collector.forms, 0, 1, &record);
    let mut requested = Vec::new();
    assert!(collector
        .collect_from(&directory, 2, |cursor| {
            requested.push(cursor);
            if requested.len() == 1 {
                Ok(batch.clone())
            } else {
                Err("connection lost before completion acknowledgement".into())
            }
        })
        .is_err());
    assert_eq!(
        requested,
        [
            wire::epoch_cursor(2, 0).unwrap(),
            wire::epoch_cursor(2, 1).unwrap()
        ]
    );
    assert_eq!(collector.after, 1);
    let path = directory.join("observed-history-0000-0001.json");
    assert_eq!(std::fs::read(&path).unwrap(), batch);

    let mut restarted =
        Collector::from_authenticated_scope(scope, &key.verifying_key().to_bytes()).unwrap();
    // A replaying transport cannot commit the previous batch a second time.
    assert!(restarted
        .collect_from(&directory, 2, |cursor| {
            assert_eq!(cursor, wire::epoch_cursor(2, 1).unwrap());
            Ok(batch.clone())
        })
        .is_err());
    assert_eq!(restarted.after, 1);
    let empty = signed(&key, scope, restarted.forms, 1, 1, &[]);
    restarted
        .collect_from(&directory, 2, |cursor| {
            assert_eq!(cursor, wire::epoch_cursor(2, 1).unwrap());
            Ok(empty.clone())
        })
        .unwrap();
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    assert_eq!(std::fs::read(path).unwrap(), batch);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn admitted_transport_cannot_replace_pinned_observation_scope() {
    let key = SigningKey::from_bytes(&[32; 32]);
    let mut collector =
        Collector::from_authenticated_scope([7; 96], &key.verifying_key().to_bytes()).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "conduit-history-source-scope-{}",
        std::process::id()
    ));
    let foreign = signed(&key, [8; 96], collector.forms, 0, 0, &[]);
    assert!(collector
        .collect_from(&directory, 0, |cursor| {
            assert_eq!(cursor, 0);
            Ok(foreign.clone())
        })
        .is_err());
    assert_eq!((collector.after, collector.observed_ms), (0, 0));
    assert!(!directory.exists());
}

#[test]
fn batch_source_cannot_bypass_extent_or_epoch_limits() {
    let key = SigningKey::from_bytes(&[33; 32]);
    let mut collector =
        Collector::from_authenticated_scope([9; 96], &key.verifying_key().to_bytes()).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "conduit-history-source-bound-{}",
        std::process::id()
    ));
    let error = collector
        .collect_from(&directory, 0, |_| Ok(vec![b' '; 2049]))
        .unwrap_err();
    assert_eq!(error.to_string(), "history batch bound");
    assert_eq!(collector.after, 0);
    assert!(!directory.exists());
    assert!(collector
        .collect_from(&directory, 6, |_| {
            panic!("invalid epoch must refuse before transport access")
        })
        .is_err());
}
