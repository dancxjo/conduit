//! Ordinary Mask reuse preserves Host request and output generations.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "support/mask.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_face as f;
use conduit_kernel::{self as k, scheduler::*, SignSink, ValueStorage};
#[test]
fn ordinary_mask_reuses_its_kernel_without_resetting_requests_or_discarding_evidence() {
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let host = fixture::host();
    let planned = source.plan(&host, 8192, &[]).unwrap();
    let face = fixture::face();
    let bytes = serde_json::to_vec(&face).unwrap();
    let shows: Vec<_> = (1..=4)
        .map(|sequence| {
            f::MaskShow::prepared(
                &planned,
                &face,
                c::bind_active_play(
                    &planned.plan.plan_id,
                    &host.host_id,
                    &host.boot_id,
                    sequence,
                ),
                "environment".into(),
                "fixture/display".into(),
                "fixture/prepared".into(),
            )
            .unwrap()
        })
        .collect();
    let cache = f::PreparedMaskPlanJson::prepare(&planned, 8192).unwrap();
    let expected: Vec<_> = shows
        .iter()
        .map(|show| serde_json::to_vec(show).unwrap())
        .collect();
    let encoders: Vec<_> = shows
        .iter()
        .map(|show| cache.bind(show, 8192).unwrap())
        .collect();
    let prepared =
        conduit_environment_runtime::mask_kernel::PreparedMaskKernel::prepare(&planned).unwrap();
    use prepared_session::{OwnedValueBoundary as Boundary, PreparedKernelSession};
    let boundary = |fore: &conduit_plan_lowering::lowering::LoweredForePort| Boundary {
        endpoint: fore.endpoint,
        cord: fore.cord,
    };
    let face_fore = boundary(&prepared.face);
    let show_fore = boundary(&prepared.show);
    let interaction = boundary(&prepared.interaction);
    let mut session = PreparedKernelSession::new_with_owned_optional_value_outputs(
        prepared.kernel,
        &[face_fore, show_fore, interaction],
        &[interaction],
        12,
    )
    .unwrap_or_else(|(r, _)| panic!("{r:?}"));
    let fresh_log = || {
        k::FixedSignLog::<64, 12>::new_with_remote_storage(
            (64 * core::mem::size_of::<k::KernelEvent>()) as u32,
            12,
            k::remote_sign_storage_bytes(12).unwrap(),
        )
        .unwrap()
    };
    let mut evidence = [0; conduit_environment_runtime::display_evidence::MAXIMUM_BYTES];
    for (sequence, encoder) in encoders.iter().enumerate() {
        let new_log = fresh_log(); // cold reservation, admitted before Play
        let (_, memory) = allocation_probe::observe(|| {
            if sequence > 0 {
                let retired = session
                    .begin_next([(); 3], new_log)
                    .unwrap_or_else(|r| panic!("{:?}", r.reason));
                // Exact previous signs move to their caller; no silent clear.
                assert!(!retired.signs.is_empty());
                let length = conduit_environment_runtime::display_evidence::encode(
                    (sequence - 1) as u64,
                    retired.signs.events(),
                    &retired.signs,
                    &mut evidence,
                )
                .unwrap();
                let page =
                    conduit_environment_runtime::display_evidence::Page::parse(&evidence[..length])
                        .unwrap();
                assert_eq!(page.invocation(), (sequence - 1) as u64);
            }
            session
                .admit_owned_input_encoded(
                    face_fore.endpoint,
                    face_fore.cord,
                    sequence as u64,
                    8192,
                    |out| {
                        out[..bytes.len()].copy_from_slice(&bytes);
                        Ok::<_, ()>(bytes.len())
                    },
                )
                .unwrap();
            session
                .close_owned_input(face_fore.endpoint, face_fore.cord)
                .unwrap();
            let mut drained = false;
            let mut emitted = false;
            for _ in 0..64 {
                let status = session.step().unwrap();
                if let Some(request) = session.next_host_request() {
                    assert_eq!(request.request, k::RequestId(sequence as u32));
                    assert_eq!(
                        session.kernel().host_value(request.input.value).unwrap(),
                        bytes
                    );
                    let value = session
                        .store_host_value_encoded(encoder.encoded_bytes(), |out| {
                            encoder.encode(out)
                        })
                        .unwrap();
                    session
                        .complete_host_call(
                            request.node,
                            request.request,
                            k::HostCallOutcome {
                                disposition: k::HostCallDisposition::Completed,
                                output: Some(k::BoundedValueRef::new(value, 8192).unwrap()),
                                failure: None,
                            },
                        )
                        .unwrap();
                }
                if !emitted {
                    if let Some(offer) = session
                        .owned_output(show_fore.endpoint, show_fore.cord)
                        .unwrap()
                    {
                        assert_eq!(offer.sequence, sequence as u64);
                        assert_eq!(
                            session.kernel().host_value(offer.value).unwrap(),
                            expected[sequence]
                        );
                        session
                            .accept_owned_output(offer.endpoint, offer.cord, offer.sequence)
                            .unwrap();
                        emitted = true;
                    }
                }
                if status == SchedulerStatus::Drained {
                    drained = true;
                    break;
                }
            }
            assert!(drained && emitted);
            assert_eq!(session.kernel().values().used_items(), 0);
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
}
