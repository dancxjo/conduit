use crate::test_packages::{test_build_host_image, test_catalog, test_checked_host_profile};
use crate::*;

const CONDUITOS_NATIVE: &str =
    include_str!("../../../targets/conduitos/profiles/conduitos-native.host.conduit");
const CONDUITOS_HEADLESS: &str =
    include_str!("../../../targets/conduitos/profiles/conduitos-x86_64-pc.host.conduit");

#[test]
fn native_presenter_offer_requires_exact_image_and_live_compositor_stack() {
    let catalog = test_catalog();
    let inputs = BuildInputs {
        source_identity: "git:46275f67".into(),
        toolchain_available: true,
    };
    let native_profile = test_checked_host_profile(CONDUITOS_NATIVE);
    let headless_profile = test_checked_host_profile(CONDUITOS_HEADLESS);
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
        let refusal = bind_runtime_offer(
            &headless.manifest,
            &headless,
            &headless_bytes,
            &catalog,
            native_runtime_inputs(presenter.clone(), facts_ready),
        )
        .unwrap_err();
        assert!(matches!(
            refusal,
            RuntimeBindingDiagnostic::UnexpectedBaseImplementation { .. }
        ));
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
        host_call: conduit_core::HostCallRequirement {
            contract_id: conduit_core::HostCallContractId::from("conduit.host/present@1"),
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
        candidate_bases: vec![conduit_core::BaseProviderEntry {
            base_id: conduit_core::HostBaseId::from("conduitos/display/0"),
            provider_instance_id: conduit_core::BaseInstanceId::from(
                "conduitos/display/provider/4",
            ),
            provider_generation: 4,
            implementation_id: conduit_core::BaseImplementationId::from(
                "display/linear-framebuffer@1",
            ),
            mechanism_family: conduit_core::HostBaseKindId::from("display/scanout"),
            enforcement_class: conduit_core::BaseEnforcementClass::ConduitOsKernelEnforced,
            lifecycle: conduit_core::BaseLifecycle::Ready,
            capabilities: vec![presenter],
            resources: vec![conduit_core::resource_offer(
                "surface/main",
                "presentation/surface",
                1,
            )],
        }],
        candidate_capabilities: vec![],
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
