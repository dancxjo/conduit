use conduit_core::{
    resource_offer, resource_requirement, ArtifactId, BaseEnforcementClass, BaseImplementationId,
    BaseInstanceId, BaseLifecycle, BaseProviderEntry, BaseRegistryRefusal, BootId, CapabilityId,
    CapabilityLimits, ExecutionProfileId, HostBaseId, HostBaseKindId, HostCallContractId,
    HostCallRequirement, HostId, HostProfileId, ImplementationId, OfferGeneration, SignId,
};
use conduit_presentation::{renderer_offer, RendererRealizationOffer};

use crate::test_packages::{test_build_host_image, test_catalog, test_checked_host_profile};
use crate::{
    bind_runtime_offer, BoundHostAdvertisement, BuildInputs, RuntimeBindingDiagnostic,
    RuntimeFacts, RuntimeOfferInputs,
};

const NATIVE_PROFILE: &str =
    include_str!("../../../targets/conduitos/profiles/conduitos-native.host.conduit");

fn presenter() -> conduit_core::CapabilityOffer {
    renderer_offer(RendererRealizationOffer {
        capability_id: CapabilityId::from("presenter/native-base-owned"),
        execution_profile_id: ExecutionProfileId::from("conduitos/native@1"),
        implementation_id: ImplementationId::from("presenter/native-graphical@1"),
        artifact_id: ArtifactId::from("conduitos/native-image@1"),
        host_call: HostCallRequirement {
            contract_id: HostCallContractId::from("conduit.host/present@1"),
            target_kind: Some(conduit_core::kind_id(
                "presentation/base/native-compositor@1",
            )),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_presentation::MAX_RENDERER_VALUE_BYTES,
            maximum_output_bytes: conduit_presentation::MAX_RENDERER_VALUE_BYTES,
        },
        resource_requirement: resource_requirement("presentation/surface", 1),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_presentation::MAX_RENDERER_VALUE_BYTES,
        },
    })
}

fn display_base(lifecycle: BaseLifecycle) -> BaseProviderEntry {
    BaseProviderEntry {
        base_id: HostBaseId::from("boot/display/0"),
        provider_instance_id: BaseInstanceId::from("boot/display/provider/7"),
        provider_generation: 7,
        implementation_id: BaseImplementationId::from("display/linear-framebuffer@1"),
        mechanism_family: HostBaseKindId::from("display/scanout"),
        enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle,
        capabilities: vec![presenter()],
        resources: vec![resource_offer(
            "boot/display/0/surface",
            "presentation/surface",
            1,
        )],
    }
}

fn facts(ready: bool) -> RuntimeFacts {
    RuntimeFacts {
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
    }
}

fn bind(
    bases: Vec<BaseProviderEntry>,
    runtime_facts: RuntimeFacts,
) -> Result<BoundHostAdvertisement, RuntimeBindingDiagnostic> {
    let catalog = test_catalog();
    let profile = test_checked_host_profile(NATIVE_PROFILE);
    let inputs = BuildInputs {
        source_identity: "git:runtime-base-proof".into(),
        toolchain_available: true,
    };
    let (image, bytes) = test_build_host_image(profile, &catalog, &inputs).unwrap();
    bind_runtime_offer(
        &image.manifest,
        &image,
        &bytes,
        &catalog,
        RuntimeOfferInputs {
            host_id: HostId::from("host/native"),
            boot_id: BootId::from("boot/native/9"),
            offer_generation: OfferGeneration(11),
            offer_sign_id: SignId::from("sign/native/offer/11"),
            host_profile: HostProfileId::from(image.manifest.profile_id.clone()),
            candidate_bases: bases,
            candidate_capabilities: vec![],
            planner_capabilities: vec![],
            facts: runtime_facts,
        },
    )
}

#[test]
fn descriptive_readiness_cannot_manufacture_a_base_owned_offer() {
    let bound = bind(vec![], facts(true)).unwrap();
    assert!(bound.advertisement().bases.is_empty());
    assert!(bound.advertisement().resources.is_empty());
    assert!(bound.advertisement().capabilities.is_empty());
}

#[test]
fn ready_base_projects_provenance_resource_and_back_offer_together() {
    let bound = bind(vec![display_base(BaseLifecycle::Ready)], facts(true)).unwrap();
    let advertisement = bound.advertisement();
    assert_eq!(advertisement.bases.len(), 1);
    assert_eq!(advertisement.resources.len(), 1);
    assert_eq!(advertisement.capabilities.len(), 1);
    assert_eq!(advertisement.bases[0].base_id.as_str(), "boot/display/0");
    assert_eq!(
        advertisement.bases[0].resource_pool_ids,
        vec![conduit_core::ResourcePoolId::from("boot/display/0/surface")]
    );
    assert_eq!(
        advertisement.bases[0].capability_ids,
        vec![CapabilityId::from("presenter/native-base-owned")]
    );
    assert_eq!(bound.identity().boot_id.as_str(), "boot/native/9");
    assert_eq!(bound.identity().offer_generation, OfferGeneration(11));
    assert_ne!(bound.identity().profile_id, bound.identity().build_id);
    assert_ne!(bound.identity().build_id, bound.identity().image_id);
}

#[test]
fn lifecycle_or_missing_qualification_suppresses_the_whole_base_projection() {
    for lifecycle in [
        BaseLifecycle::Configured,
        BaseLifecycle::Starting,
        BaseLifecycle::Degraded,
        BaseLifecycle::Lost,
        BaseLifecycle::Stopped,
    ] {
        let bound = bind(vec![display_base(lifecycle)], facts(true)).unwrap();
        assert!(bound.advertisement().bases.is_empty());
        assert!(bound.advertisement().resources.is_empty());
        assert!(bound.advertisement().capabilities.is_empty());
    }
    let unqualified = bind(vec![display_base(BaseLifecycle::Ready)], facts(false)).unwrap();
    assert!(unqualified.advertisement().bases.is_empty());
    assert!(unqualified.advertisement().resources.is_empty());
    assert!(unqualified.advertisement().capabilities.is_empty());
}

#[test]
fn malformed_or_duplicate_provider_truth_refuses_instead_of_flattening() {
    let mut zero_generation = display_base(BaseLifecycle::Ready);
    zero_generation.provider_generation = 0;
    assert_eq!(
        bind(vec![zero_generation], facts(true)),
        Err(RuntimeBindingDiagnostic::InvalidBaseProvider(
            BaseRegistryRefusal::ZeroGeneration
        ))
    );

    let base = display_base(BaseLifecycle::Ready);
    assert_eq!(
        bind(vec![base.clone(), base], facts(true)),
        Err(RuntimeBindingDiagnostic::InvalidBaseProvider(
            BaseRegistryRefusal::DuplicateBase
        ))
    );

    let mut unbuilt = display_base(BaseLifecycle::Ready);
    unbuilt.implementation_id = BaseImplementationId::from("display/invented@1");
    assert_eq!(
        bind(vec![unbuilt], facts(true)),
        Err(RuntimeBindingDiagnostic::UnexpectedBaseImplementation {
            kind: "display/scanout".into(),
            implementation: "display/invented@1".into(),
        })
    );
}
