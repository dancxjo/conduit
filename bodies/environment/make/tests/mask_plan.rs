//! CPU-only renderer metadata. Planning is not initialized provider possession.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_environment_make::SevenSegmentMaskSource;
use conduit_face as f;
#[path = "support/mask.rs"]
mod fixture;
use fixture::{face, host};
#[test]
fn ordinary_mask_requires_current_renderer_and_resource_then_exposes_its_memory_cost() {
    let source = SevenSegmentMaskSource::compile().unwrap();
    let host = host();
    let mut missing = host.clone();
    missing.resources.clear();
    assert!(source.plan(&missing, 8192, &[]).is_err());
    missing = host.clone();
    missing
        .capabilities
        .retain(|c| c.kind_id.as_str() != f::RENDERER_KIND);
    assert!(source.plan(&missing, 8192, &[]).is_err());
    missing = host.clone();
    missing.offer_generation = c::OfferGeneration(0);
    assert!(source.plan(&missing, 8192, &[]).is_err());
    let (planned, memory) = allocation_probe::observe(|| source.plan(&host, 8192, &[]).unwrap());
    assert_eq!(planned.mask, source.role);
    assert_eq!(planned.plan.fragments.len(), 1);
    assert_eq!(planned.plan.fragments[0].placements.len(), 3);
    let face = face();
    let active = c::bind_active_play(&planned.plan.plan_id, &host.host_id, &host.boot_id, 1);
    let (show, show_memory) = allocation_probe::observe(|| {
        f::MaskShow::prepared(
            &planned,
            &face,
            active,
            "environment".into(),
            "fixture/display".into(),
            "fixture/prepared".into(),
        )
        .unwrap()
    });
    show.validate(&face).unwrap();
    let (wire, wire_memory) = allocation_probe::observe(|| serde_json::to_vec(&show).unwrap());
    let face_bytes = serde_json::to_vec(&face).unwrap().len();
    println!("Mask Plan retained={} peak={}; prepared MaskShow additional retained={} peak={}; MaskShow wire={} serialization peak={}; Face wire={}",memory.retained_bytes(),memory.peak_bytes,show_memory.retained_bytes(),show_memory.peak_bytes,wire.len(),wire_memory.peak_bytes,face_bytes);
}

#[test]
fn prepared_mask_json_preserves_complete_wire_without_encoding_allocations() {
    let source = SevenSegmentMaskSource::compile().unwrap();
    let host = host();
    let planned = source.plan(&host, 8192, &[]).unwrap();
    let face = face();
    let active = c::bind_active_play(&planned.plan.plan_id, &host.host_id, &host.boot_id, 1);
    let show = f::MaskShow::prepared(
        &planned,
        &face,
        active,
        "environment".into(),
        "fixture/display".into(),
        "fixture/prepared".into(),
    )
    .unwrap();
    let expected = serde_json::to_vec(&show).unwrap();
    let (cache, cache_memory) =
        allocation_probe::observe(|| f::PreparedMaskPlanJson::prepare(&planned, 8192).unwrap());
    let (encoder, encoder_memory) =
        allocation_probe::observe(|| cache.bind(&show, expected.len()).unwrap());
    assert!(matches!(
        f::PreparedMaskPlanJson::prepare(&planned, 0),
        Err(f::MaskJsonRefusal::Capacity)
    ));
    assert!(matches!(
        f::PreparedMaskPlanJson::prepare(&planned, cache.bytes().len() - 1),
        Err(f::MaskJsonRefusal::Capacity)
    ));
    assert_eq!(encoder.encoded_bytes(), expected.len());
    assert!(matches!(
        cache.bind(&show, expected.len() - 1),
        Err(f::MaskJsonRefusal::Capacity)
    ));
    let mut changed = show.clone();
    changed.planned_mask.plan.fragments[0].placements[0].implementation_id = "different".into();
    assert!(matches!(
        cache.bind(&changed, 8192),
        Err(f::MaskJsonRefusal::DifferentPlan)
    ));
    let mut short = vec![0x55; expected.len() - 1];
    assert_eq!(
        encoder.encode(&mut short),
        Err(f::MaskJsonRefusal::Capacity)
    );
    assert!(short.iter().all(|b| *b == 0x55));
    // These lifecycle values are codec fixtures, not claims of hardware delivery.
    let available = show
        .transition(
            f::ManifestationLifecycle::Available,
            "fixture/available".into(),
        )
        .unwrap();
    let closed = available
        .transition(f::ManifestationLifecycle::Closed, "fixture/closed".into())
        .unwrap();
    let failed = show
        .fail(
            f::ManifestationFailure::OutputRejected,
            "fixture/failed".into(),
        )
        .unwrap();
    for snapshot in [&available, &closed, &failed] {
        let expected = serde_json::to_vec(snapshot).unwrap();
        let encoding = cache.bind(snapshot, 8192).unwrap();
        let mut bytes = vec![0; encoding.encoded_bytes()];
        encoding.encode(&mut bytes).unwrap();
        assert_eq!(bytes, expected);
    }
    drop(show);
    drop(planned);
    let mut output = vec![0; expected.len()];
    let (_, encoding_memory) = allocation_probe::observe(|| {
        for _ in 0..960 {
            assert_eq!(encoder.encode(&mut output).unwrap(), expected.len());
        }
    });
    assert_eq!(encoding_memory.allocations, 0);
    assert_eq!(encoding_memory.reallocations, 0);
    assert_eq!(encoding_memory.peak_bytes, 0);
    assert_eq!(output, expected);
    let decoded: f::MaskShow = serde_json::from_slice(&output).unwrap();
    decoded.validate(&face).unwrap();
    println!(
        "Prepared Mask JSON cache retained={}; snapshot retained={}; repeated encoding peak={}",
        cache_memory.retained_bytes(),
        encoder_memory.retained_bytes(),
        encoding_memory.peak_bytes
    );
}

#[test]
fn ordinary_mask_plays_with_two_shared_value_slots_and_preserves_egress_pressure() {
    use conduit_environment_runtime::mask_kernel::PreparedMaskKernel;
    use conduit_kernel::{self as k, scheduler::SchedulerStatus, ValueStorage};
    for cancel_at in [None, Some(0), Some(4), Some(12), Some(30)] {
        let source = SevenSegmentMaskSource::compile().unwrap();
        let host = host();
        let planned = source.plan(&host, 8192, &[]).unwrap();
        let face = face();
        let active = c::bind_active_play(&planned.plan.plan_id, &host.host_id, &host.boot_id, 1);
        // CPU renderer service returns a Prepared Show. No physical effect or
        // Available claim is made by these metadata-only providers.
        let show = f::MaskShow::prepared(
            &planned,
            &face,
            active,
            "environment".into(),
            "fixture/display".into(),
            "fixture/prepared".into(),
        )
        .unwrap();
        let expected = serde_json::to_vec(&show).unwrap();
        let face_wire = serde_json::to_vec(&face).unwrap();
        let cache = f::PreparedMaskPlanJson::prepare(&planned, 8192).unwrap();
        let encoder = cache.bind(&show, 8192).unwrap();
        let mut output = vec![0; encoder.encoded_bytes()];
        let (mut prepared, preparation) =
            allocation_probe::observe(|| PreparedMaskKernel::prepare(&planned).unwrap());
        let backing = prepared.kernel.values().reserved_storage_bytes();
        let mut calls = 0;
        let mut peak_items = 0;
        let mut held = None;
        let mut delivered = false;
        let mut drained = false;
        let ((), play_memory) = allocation_probe::observe(|| {
            prepared
                .kernel
                .admit_remote_input(prepared.face.endpoint, prepared.face.cord, 0, &face_wire)
                .unwrap();
            prepared
                .kernel
                .close_remote_input(prepared.face.endpoint, prepared.face.cord)
                .unwrap();
            for step in 0..96 {
                if cancel_at == Some(step) {
                    prepared.kernel.cancel().unwrap();
                    assert_eq!(prepared.kernel.values().used_items(), 0);
                    return;
                }
                let status = prepared.kernel.step().unwrap();
                if let Some(request) = prepared.kernel.next_host_request() {
                    assert_eq!(
                        prepared.kernel.host_value(request.input.value).unwrap(),
                        face_wire
                    );
                    assert_eq!(calls, 0);
                    calls += 1;
                    encoder.encode(&mut output).unwrap();
                    let value = prepared.kernel.store_host_value(&output).unwrap();
                    prepared
                        .kernel
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
                peak_items = peak_items.max(prepared.kernel.values().used_items());
                if !delivered {
                    if held.is_none() || step == 32 {
                        if let Some(offer) = prepared
                            .kernel
                            .remote_egress_offer(prepared.show.endpoint, prepared.show.cord)
                            .unwrap()
                        {
                            if let Some(prior) = held {
                                assert_eq!(offer, prior);
                            }
                            held = Some(offer);
                        }
                    }
                    if let Some(offer) = held {
                        assert_eq!(prepared.kernel.host_value(offer.value).unwrap(), expected);
                        if step < 32 {
                            assert_ne!(status, SchedulerStatus::Drained);
                        } else {
                            prepared
                                .kernel
                                .remote_egress_accept(offer.endpoint, offer.cord, offer.sequence)
                                .unwrap();
                            prepared
                                .kernel
                                .remote_egress_delivered(offer.endpoint, offer.cord, offer.sequence)
                                .unwrap();
                            delivered = true;
                        }
                    }
                }
                if status == SchedulerStatus::Drained {
                    drained = true;
                    break;
                }
            }
        });
        assert_eq!((play_memory.allocations, play_memory.reallocations), (0, 0));
        assert!(peak_items <= 2);
        if cancel_at.is_none() {
            assert!(drained);
            assert!(held.is_some());
            assert_eq!(calls, 1);
            assert_eq!(prepared.kernel.values().used_items(), 0);
            println!("Ordinary Mask kernel retained={} preparation peak={} backing={} peak live values={} Play allocation peak={}",preparation.retained_bytes(),preparation.peak_bytes,backing,peak_items,play_memory.peak_bytes);
        }
    }
}

#[test]
fn renderer_failure_emits_no_show_and_cancellation_releases_its_face() {
    use conduit_environment_runtime::mask_kernel::PreparedMaskKernel;
    use conduit_kernel::{self as k, scheduler::SchedulerError, ValueStorage};
    let source = SevenSegmentMaskSource::compile().unwrap();
    let planned = source.plan(&host(), 8192, &[]).unwrap();
    let mut prepared = PreparedMaskKernel::prepare(&planned).unwrap();
    let bytes = serde_json::to_vec(&face()).unwrap();
    prepared
        .kernel
        .admit_remote_input(prepared.face.endpoint, prepared.face.cord, 0, &bytes)
        .unwrap();
    prepared
        .kernel
        .close_remote_input(prepared.face.endpoint, prepared.face.cord)
        .unwrap();
    let mut failed = false;
    for _ in 0..48 {
        match prepared.kernel.step() {
            Err(SchedulerError::BackFailed(_)) => {
                failed = true;
                break;
            }
            result => {
                result.unwrap();
            }
        }
        if let Some(request) = prepared.kernel.next_host_request() {
            prepared
                .kernel
                .complete_host_call(
                    request.node,
                    request.request,
                    k::HostCallOutcome {
                        disposition: k::HostCallDisposition::Failed,
                        output: None,
                        failure: Some(k::Failure {
                            code: k::FailureCode::HostCallFailed,
                            detail: 1,
                        }),
                    },
                )
                .unwrap();
        }
    }
    assert!(failed);
    assert!(prepared
        .kernel
        .remote_egress_offer(prepared.show.endpoint, prepared.show.cord)
        .unwrap()
        .is_none());
    prepared.kernel.cancel().unwrap();
    assert_eq!(prepared.kernel.values().used_items(), 0);
}
