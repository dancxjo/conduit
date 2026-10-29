//! CPU-only providers: metadata fixtures grant no physical readiness.
use conduit_core as c;
use conduit_face as f;
pub fn host() -> c::HostAdvertisement {
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
pub fn face() -> f::Face {
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
