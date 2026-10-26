//! CPU-only renderer metadata. Planning is not initialized provider possession.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_environment_make::SevenSegmentMaskSource;
use conduit_face as f;
fn host() -> c::HostAdvertisement {
    let limits = c::CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: 8192,
    };
    let implementation = || c::ImplementationOffer {
        execution_profile_id: "fixture/seven-segment".into(),
        implementation_id: "fixture/seven-segment".into(),
        artifact_id: "fixture/seven-segment".into(),
    };
    c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/mask-host".into(),
        boot_id: "fixture/mask-boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/mask".into(),
        bases: vec![],
        planner_capabilities: vec![],
        resources: vec![c::resource_offer("fixture/display", "fixture/display", 1)],
        capabilities: vec![
            f::face_tee_offer("fixture/tee".into(), implementation(), limits.clone()),
            f::face_interaction_close_offer(
                "fixture/interaction".into(),
                implementation(),
                limits.clone(),
            ),
            f::renderer_offer(f::RendererRealizationOffer {
                capability_id: "fixture/render".into(),
                execution_profile_id: "fixture/seven-segment".into(),
                implementation_id: "fixture/seven-segment".into(),
                artifact_id: "fixture/seven-segment".into(),
                host_call: c::HostCallRequirement {
                    contract_id: "fixture/present".into(),
                    target_kind: Some(c::kind_id("fixture/display")),
                    maximum_in_flight: 1,
                    maximum_input_bytes: 8192,
                    maximum_output_bytes: 8192,
                },
                resource_requirement: c::resource_requirement("fixture/display", 1),
                limits,
            }),
        ],
    }
}
fn face() -> f::Face {
    use conduit_environment_runtime::{
        observation::{ObservationContract, PreparedSourceFrameDecoder},
        publication::{PublicationChannel, PublicationSnapshot},
        retained_publication_face::RetainedPublicationFace,
    };
    let contract = ObservationContract::prepare().unwrap();
    let mut decoder = PreparedSourceFrameDecoder::prepare(&contract).unwrap();
    let bytes = decoder.decode(5, [53, 0, 20, 2, 75]).unwrap();
    let mut owner = RetainedPublicationFace::prepare(
        f::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("a".repeat(64).into()),
            checked_plot_id: Some("b".repeat(64).into()),
            expanded_plot_id: Some("c".repeat(64).into()),
            plan_id: Some("d".repeat(64).into()),
            active_play_id: Some("e".repeat(64).into()),
            sign_ids: vec![],
        },
        960,
        120000,
    )
    .unwrap();
    owner
        .publish(
            PublicationSnapshot {
                sequence: 1,
                channel: PublicationChannel::Reading,
                canonical: bytes,
            },
            1000,
        )
        .unwrap();
    owner.current().clone()
}
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
