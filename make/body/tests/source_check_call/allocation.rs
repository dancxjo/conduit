use super::*;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
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
