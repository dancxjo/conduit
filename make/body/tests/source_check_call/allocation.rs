use super::*;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
pub(super) mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn prepared_provider_success_staleness_and_short_slot_do_not_allocate() {
    let (mut owner, request) = support::owner(VALID, 7);
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let input = schema
        .encode_request(&request, bounds().request_bytes as usize)
        .unwrap();
    let expected = schema
        .encode_reply(
            SourceCheckReply::Checked(&owner.project(&request).unwrap()),
            bounds().report_bytes as usize,
        )
        .unwrap();
    let checker = PreparedSourceChecker::prepare(
        &mut [SourceCheckPreparation {
            owner: &mut owner,
            request: &request,
        }],
        bounds(),
    )
    .unwrap();
    assert!(checker.retained_bytes() <= bounds().retained_bytes);
    let mut provider = checker.into_provider();
    let mut stale = request.clone();
    stale.snapshot.generation += 1;
    let stale = schema
        .encode_request(&stale, bounds().request_bytes as usize)
        .unwrap();
    let stale_reply = schema
        .encode_reply(
            SourceCheckReply::Refused(SourceCheckRefusal::StaleSource),
            bounds().report_bytes as usize,
        )
        .unwrap();
    let invalid_reply = schema
        .encode_reply(
            SourceCheckReply::Refused(SourceCheckRefusal::InvalidSnapshot),
            bounds().report_bytes as usize,
        )
        .unwrap();
    let mut unprepared = request.clone();
    unprepared.entry = "other".into();
    let unprepared = schema
        .encode_request(&unprepared, bounds().request_bytes as usize)
        .unwrap();
    let unprepared_reply = schema
        .encode_reply(
            SourceCheckReply::NotPrepared,
            bounds().report_bytes as usize,
        )
        .unwrap();
    let mut output = vec![0; bounds().report_bytes as usize];
    let (_, measured) = allocation_probe::observe(|| {
        let len = provider.read(&input, &mut output).unwrap();
        assert_eq!(&output[..len], expected);
        let len = provider.read(&stale, &mut output).unwrap();
        assert_eq!(&output[..len], stale_reply);
        let len = provider.read(b"invalid", &mut output).unwrap();
        assert_eq!(&output[..len], invalid_reply);
        let len = provider.read(&unprepared, &mut output).unwrap();
        assert_eq!(&output[..len], unprepared_reply);
        assert!(provider.read(&input, &mut output[..1]).is_err());
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
}
#[test]
fn preparation_rejects_stale_source_and_retained_or_output_pressure() {
    let (mut owner, request) = support::owner(VALID, 7);
    let mut stale = request.clone();
    stale.snapshot.generation += 1;
    let error = PreparedSourceChecker::prepare(
        &mut [SourceCheckPreparation {
            owner: &mut owner,
            request: &stale,
        }],
        bounds(),
    )
    .err()
    .unwrap();
    assert!(error.contains("StaleSource"), "{error}");
    for small in [
        SourceCheckPreparationBounds {
            retained_bytes: 0,
            ..bounds()
        },
        SourceCheckPreparationBounds {
            report_bytes: 1,
            ..bounds()
        },
    ] {
        assert!(PreparedSourceChecker::prepare(
            &mut [SourceCheckPreparation {
                owner: &mut owner,
                request: &request
            }],
            small
        )
        .is_err());
    }
    assert_eq!(owner.snapshot(), &request.snapshot);
    assert_eq!(owner.context(), &request.context);
}

#[test]
fn complete_preparation_costs_include_actual_catalog_and_retire_before_play() {
    for (label, source) in [("valid", VALID), ("invalid", INVALID)] {
        let (prepared, measured) = allocation_probe::observe(|| {
            let (mut owner, request) = support::owner(source, 7);
            PreparedSourceChecker::prepare(
                &mut [SourceCheckPreparation {
                    owner: &mut owner,
                    request: &request,
                }],
                bounds(),
            )
            .unwrap()
        });
        // The scoped peak includes catalogue construction, immutable Source,
        // compiler/cache, schema and packet admission. It excludes stack,
        // allocator bookkeeping, pre-existing process state and other threads.
        assert!(measured.peak_bytes >= prepared.retained_bytes());
        eprintln!(
            "source-check-preparation {}",
            serde_json::json!({
                "case": label,
                "source_bytes": source.len(),
                "retained_provider_payload_bytes": prepared.retained_bytes(),
                "scoped_heap_peak_bytes": measured.peak_bytes,
                "requested_bytes": measured.requested_bytes,
                "allocations": measured.allocations,
                "reallocations": measured.reallocations,
                "enforced_preparation_memory_limit": false,
            })
        );
        let mut provider = prepared.into_provider();
        let mut output = vec![0; bounds().report_bytes as usize];
        let (_, execution) =
            allocation_probe::observe(|| provider.read(b"not-a-request", &mut output).unwrap());
        assert_eq!((execution.allocations, execution.reallocations), (0, 0));
    }
}
