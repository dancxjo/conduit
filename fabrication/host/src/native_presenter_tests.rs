use crate::test_packages::{test_build_host_image, test_catalog};
use crate::*;

const CONDUITOS_NATIVE: &str =
    include_str!("../../../targets/conduitos/profiles/conduitos-native.profile.json");
const CONDUITOS_HEADLESS: &str =
    include_str!("../../../targets/conduitos/profiles/conduitos-headless.profile.json");

#[test]
fn native_presenter_offer_requires_exact_image_and_live_compositor_stack() {
    let catalog = test_catalog();
    let inputs = BuildInputs {
        source_identity: "git:46275f67".into(),
        toolchain_available: true,
    };
    let native_profile = serde_json::from_str(CONDUITOS_NATIVE).unwrap();
    let headless_profile = serde_json::from_str(CONDUITOS_HEADLESS).unwrap();
    let (native, native_bytes) = test_build_host_image(native_profile, &catalog, &inputs).unwrap();
    let (headless, headless_bytes) =
        test_build_host_image(headless_profile, &catalog, &inputs).unwrap();
    let presenter = native_presenter_offer();

    let ready = bind_runtime_offer(
        &native.manifest,
        &native,
        &native_bytes,
        &catalog,
        native_runtime_inputs(presenter.clone(), true),
    )
    .unwrap();
    assert_eq!(ready.advertisement().capabilities, vec![presenter.clone()]);

    for facts_ready in [false, true] {
        let bound = bind_runtime_offer(
            &headless.manifest,
            &headless,
            &headless_bytes,
            &catalog,
            native_runtime_inputs(presenter.clone(), facts_ready),
        )
        .unwrap();
        assert!(bound.advertisement().capabilities.is_empty());
        assert!(bound.advertisement().resources.is_empty());
    }
    let unavailable = bind_runtime_offer(
        &native.manifest,
        &native,
        &native_bytes,
        &catalog,
        native_runtime_inputs(presenter, false),
    )
    .unwrap();
    assert!(unavailable.advertisement().capabilities.is_empty());
}

fn native_presenter_offer() -> conduit_core::CapabilityOffer {
    conduit_presentation::renderer_offer(conduit_presentation::RendererRealizationOffer {
        capability_id: conduit_core::CapabilityId::from("presenter/native"),
        execution_profile_id: conduit_core::ExecutionProfileId::from("conduitos/native@1"),
        implementation_id: conduit_core::ImplementationId::from("presenter/native-graphical@1"),
        artifact_id: conduit_core::ArtifactId::from("conduitos/native-image@1"),
        host_operation: conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from("conduit.host/present@1"),
            target_kind: Some(conduit_core::kind_id(
                "presentation/base/native-compositor@1",
            )),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_presentation::MAX_RENDERER_VALUE_BYTES,
            maximum_output_bytes: conduit_presentation::MAX_RENDERER_VALUE_BYTES,
        },
        resource_requirement: conduit_core::resource_requirement("presentation/surface", 1),
        limits: conduit_core::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_presentation::MAX_RENDERER_VALUE_BYTES,
        },
    })
}

fn native_runtime_inputs(
    presenter: conduit_core::CapabilityOffer,
    ready: bool,
) -> RuntimeOfferInputs {
    RuntimeOfferInputs {
        host_id: conduit_core::HostId::from("conduitos/native"),
        boot_id: conduit_core::BootId::from("conduitos/boot/1"),
        offer_generation: conduit_core::OfferGeneration(4),
        offer_sign_id: conduit_core::SignId::from("conduitos/offer/4"),
        host_profile: conduit_core::HostProfileId::from("conduitos/native@1"),
        candidate_resources: vec![conduit_core::resource_offer(
            "surface/main",
            "presentation/surface",
            1,
        )],
        candidate_capabilities: vec![presenter],
        planner_capabilities: vec![],
        facts: RuntimeFacts {
            ready_resource_classes: ready
                .then(|| "presentation/surface".to_owned())
                .into_iter()
                .collect(),
            initialized_base_kinds: ready
                .then(|| "display/scanout".to_owned())
                .into_iter()
                .collect(),
            initialized_driver_kinds: ready
                .then(|| "display/linear-framebuffer@1".to_owned())
                .into_iter()
                .collect(),
            available_facilities: ready
                .then(|| "compositor/native@1".to_owned())
                .into_iter()
                .collect(),
            authority_ready: false,
        },
    }
}
