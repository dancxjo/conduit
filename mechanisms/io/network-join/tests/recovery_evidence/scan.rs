use super::*;
use conduit_net::*;
fn payload() -> [u8; MAXIMUM_SCAN_OUTPUT_BYTES as usize] {
    let records = [ScanObservation::new(
        b"\xffobserved\0ssid",
        ObservedNetworkSecurity::Unknown,
        -52,
        5,
    )
    .unwrap(); 16];
    let mut bytes = [0; MAXIMUM_SCAN_OUTPUT_BYTES as usize];
    assert_eq!(
        ScanCompletion {
            generation: 3,
            completed_ms: 6,
            count: 16,
            total: 17
        }
        .encode(&records, &mut bytes)
        .unwrap(),
        bytes.len()
    );
    bytes
}
fn scan_page(events: u16) -> Result<[u8; MAXIMUM_BYTES], Refusal> {
    let mut log =
        k::FixedSignLog::<128>::new((128 * core::mem::size_of::<k::KernelEvent>()) as u32).unwrap();
    for _ in 0..events {
        log.record(
            k::NodeId(0),
            None,
            Some(k::RequestId(0)),
            k::KernelEventKind::HostCallCompleted,
        )
        .unwrap();
    }
    let mut scope = scope();
    scope.configuration_revision = 0;
    let mut out = [0; MAXIMUM_BYTES];
    encode_scan(
        scope,
        log.events(),
        &log,
        ScanRecord {
            request: NetworkScanRequest {
                generation: 3,
                scan_deadline_ms: 100,
                cleanup_deadline_ms: 200,
            },
            evidence_seen: true,
            output: &payload(),
        },
        &mut out,
    )?;
    Ok(out)
}
#[test]
fn borrowed_scan_validation_and_owned_decode_agree_without_partial_publication() {
    let bytes = payload();
    let view = ScanCompletionView::parse(&bytes).unwrap();
    assert!(view.completion().truncated());
    assert_eq!(view.observations().len(), 16);
    assert_eq!(
        view.observations().next().unwrap().ssid.as_ptr(),
        bytes[25..].as_ptr()
    );
    let mut decoded = [ScanObservation::EMPTY; 16];
    assert_eq!(
        ScanCompletion::decode(&bytes, &mut decoded).unwrap(),
        view.completion()
    );
    for (borrowed, owned) in view.observations().zip(decoded) {
        assert_eq!(borrowed.ssid, owned.ssid());
        assert_eq!(borrowed.security, owned.security);
        assert_eq!(borrowed.rssi_dbm, owned.rssi_dbm);
        assert_eq!(borrowed.observed_ms, owned.observed_ms);
    }
    let original = decoded;
    let mut bad = bytes;
    bad[24 + 15 * 44 + 35] = 255;
    assert!(ScanCompletionView::parse(&bad).is_err());
    assert!(ScanCompletion::decode(&bad, &mut decoded).is_err());
    assert_eq!(decoded, original);
}
#[test]
fn scan_and_legacy_join_share_unchanged_journal_geometry_and_exact_restart() {
    let legacy = page();
    let scan = scan_page(MAXIMUM_FULL_SCAN_EVENTS as u16).unwrap();
    assert_eq!(Page::parse(&legacy).unwrap().kind(), RecordKind::Join);
    assert!(Page::parse(&legacy).unwrap().scan().is_none());
    let parsed = Page::parse(&scan).unwrap();
    assert_eq!(parsed.kind(), RecordKind::Scan);
    assert_eq!(parsed.scope().configuration_revision, 0);
    assert_eq!(parsed.scan().unwrap().output, payload());
    assert_eq!(parsed.events().count(), MAXIMUM_FULL_SCAN_EVENTS);
    assert_eq!(
        scan_page(MAXIMUM_FULL_SCAN_EVENTS as u16 + 1),
        Err(Refusal::Capacity)
    );
    let mut work = [0; MAXIMUM_BYTES];
    let mut journal = open(Flash::erased(), &mut work);
    journal.append(&legacy).unwrap();
    let receipt = journal.append(&scan).unwrap();
    let mut journal = open(journal.into_storage(), &mut work);
    journal.read_slot(0, &mut work).unwrap();
    assert_eq!(work, legacy);
    journal.read_slot(1, &mut work).unwrap();
    assert_eq!(work, scan);
    assert_eq!(journal.append(&scan).unwrap(), receipt);
    assert_eq!(journal.committed_count(), 2);
}
#[test]
fn scan_torn_boundaries_keep_legacy_and_retry_original_evidence() {
    let legacy = page();
    let scan = scan_page(24).unwrap();
    let mut work = [0; MAXIMUM_BYTES];
    let mut journal = open(Flash::erased(), &mut work);
    journal.append(&legacy).unwrap();
    let base = journal.into_storage();
    for budget in [
        0,
        4,
        232,
        256,
        728,
        DATA_END,
        DATA_END + 3,
        DATA_END + 4,
        MAXIMUM_BYTES - 1,
        MAXIMUM_BYTES,
    ] {
        let mut storage = base.clone();
        storage.budget = budget;
        let mut journal = open(storage, &mut work);
        let _ = journal.append(&scan);
        let mut storage = journal.into_storage();
        storage.budget = usize::MAX;
        let mut journal = open(storage, &mut work);
        journal.read_slot(0, &mut work).unwrap();
        assert_eq!(work, legacy);
        let receipt = journal.append(&scan).unwrap();
        journal.read_slot(receipt.slot(), &mut work).unwrap();
        assert_eq!(work, scan);
        assert_eq!(journal.append(&scan).unwrap(), receipt);
    }
}
#[test]
fn scan_context_and_completion_refuse_mismatch_without_invented_configuration_revision() {
    let mut log =
        k::FixedSignLog::<128>::new((128 * core::mem::size_of::<k::KernelEvent>()) as u32).unwrap();
    log.record(k::NodeId(0), None, None, k::KernelEventKind::RunCancelled)
        .unwrap();
    let bytes = payload();
    let mut out = [0; MAXIMUM_BYTES];
    for which in 0..5 {
        let mut scope = scope();
        scope.configuration_revision = 0;
        let mut request = NetworkScanRequest {
            generation: 3,
            scan_deadline_ms: 100,
            cleanup_deadline_ms: 200,
        };
        let mut output: &[u8] = &bytes;
        match which {
            0 => scope.configuration_revision = 1,
            1 => request.generation = 4,
            2 => request.cleanup_deadline_ms = 99,
            3 => request.scan_deadline_ms = 6,
            _ => output = &[],
        }
        assert!(encode_scan(
            scope,
            log.events(),
            &log,
            ScanRecord {
                request,
                evidence_seen: true,
                output
            },
            &mut out
        )
        .is_err());
    }
}
