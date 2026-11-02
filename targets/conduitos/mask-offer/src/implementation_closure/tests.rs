use super::*;

fn provider() -> BaseProviderEntry {
    BaseProviderEntry {
        base_id: "display/base".into(),
        provider_instance_id: "display/actual-instance".into(),
        provider_generation: 7,
        implementation_id: "display/linear-framebuffer@1".into(),
        mechanism_family: "display/scanout".into(),
        enforcement_class: conduit_core::BaseEnforcementClass::Cooperative,
        lifecycle: conduit_core::BaseLifecycle::Ready,
        capabilities: vec![],
        resources: vec![
            resource_offer("physical", "face/surface", 1),
            resource_offer("show", conduit_face::SHOW_RESOURCE_CLASS, 1),
            resource_offer(
                "memory",
                conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS,
                1048576,
            ),
        ],
    }
}

#[test]
fn read_only_closure_uses_actual_backs_and_exact_declared_resources_without_input() {
    let mut provider = provider();
    provider
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    for profile in [
        NativeMaskStorageProfile::Standard,
        NativeMaskStorageProfile::Collection,
    ] {
        let offer = native_read_only_host_advertisement(
            &"host".into(),
            &"boot".into(),
            7,
            &provider,
            profile,
        );
        assert_eq!(offer.resources, provider.resources);
        assert_eq!(offer.offer_generation.0, 7);
        assert_eq!(
            offer.capabilities,
            native_read_only_implementation_closure(profile)
        );
        let input = offer
            .capabilities
            .iter()
            .find(|cap| cap.kind_id.as_str() == conduit_face::FACE_INTERACTION_KIND)
            .unwrap();
        assert!(input.resource_requirements.is_empty());
        assert!(input.authority_requirements.is_empty());
        assert_eq!(
            input.implementation.implementation_id.as_str(),
            "face/conduitos-human-input@1"
        );
        assert_ne!(
            input.implementation.execution_profile_id.as_str(),
            "conduitos/bounded-interaction@1"
        );
        let stage = prepare_stage_for_profile(Adapter::Native, &offer, profile).unwrap();
        assert!(
            stage
                .planned_mask
                .plan
                .fragments
                .iter()
                .all(|fragment| fragment.offer_generation.0 == 7)
        );
        assert!(
            stage
                .planned_mask
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .flat_map(|gear| &gear.resources)
                .all(|binding| provider
                    .resources
                    .iter()
                    .any(|pool| pool.pool_id == binding.pool_id))
        );
    }
}

#[test]
fn read_only_profile_does_not_enlarge_standard_or_mutate_legacy_interactive_offer() {
    let provider = provider();
    let standard = native_read_only_host_advertisement(
        &"host".into(),
        &"boot".into(),
        7,
        &provider,
        NativeMaskStorageProfile::Standard,
    );
    assert!(
        prepare_stage_for_profile(
            Adapter::Native,
            &standard,
            NativeMaskStorageProfile::Collection
        )
        .is_err()
    );
    let interactive = native_host_advertisement(&"host".into(), &"boot".into(), 7, &provider);
    assert!(
        interactive
            .resources
            .iter()
            .any(|pool| pool.class_id.as_str() == "conduit.resource/conduitos-human-input@1")
    );
    let input = interactive
        .capabilities
        .iter()
        .find(|cap| cap.kind_id.as_str() == conduit_face::FACE_INTERACTION_KIND)
        .unwrap();
    assert_eq!(
        input.implementation.execution_profile_id.as_str(),
        "conduitos/bounded-interaction@1"
    );
    assert!(!input.resource_requirements.is_empty());
}
