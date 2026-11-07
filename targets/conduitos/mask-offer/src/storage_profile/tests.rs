use super::*;
use conduit_core::*;
use conduit_face::*;

#[test]
fn collection_plans_use_exact_capacity_and_refuse_smaller_offers() {
    let provider = BaseProviderEntry {
        base_id: "collection/surface".into(),
        provider_instance_id: "collection/provider".into(),
        provider_generation: 1,
        implementation_id: "conduitos/framebuffer@1".into(),
        mechanism_family: "conduitos.base/framebuffer@1".into(),
        enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: BaseLifecycle::Ready,
        capabilities: Vec::new(),
        resources: vec![resource_offer(
            "collection/resource",
            SHOW_RESOURCE_CLASS,
            1,
        )],
    };
    let host_id = HostId::from("collection/host");
    let boot_id = BootId::from("collection/boot");
    let small = native_host_advertisement(&host_id, &boot_id, 1, &provider);
    let large = collection_native_host_advertisement(&host_id, &boot_id, 1, &provider);
    assert!(prepare_stage_from_offer(Adapter::Native, &large).is_err());
    assert!(prepare_collection_stage_from_offer(&small).is_err());
    NativeMaskStorageProfile::Collection
        .admit_host(&large, "renderer-conduitos")
        .unwrap();
    let generic = prepare_collection_stage_from_offer(&large).unwrap();
    assert!(verify_plan(&generic.planned_mask.plan));
    assert_eq!(
        prepare_advertised_native_stage_from_offer(&large, None)
            .unwrap()
            .planned_mask,
        generic.planned_mask
    );
    assert_eq!(
        prepare_advertised_native_stage_from_offer(&small, None)
            .unwrap()
            .planned_mask,
        prepare_stage_from_offer(Adapter::Native, &small)
            .unwrap()
            .planned_mask
    );
    assert!(
        generic
            .planned_mask
            .plan
            .fragments
            .iter()
            .flat_map(|f| &f.fore_ports)
            .any(|p| p.front_port_id.as_str() == "face" && p.byte_capacity == 256 * 1024)
    );
    let selection = ComponentVisualSelection::default();
    let (_, original) =
        prepare_selected_component_stage_from_offer(&small, selection, TodoComponentRecipe::Flat)
            .unwrap();
    let (configured, collection) =
        prepare_collection_component_stage_from_offer(&large, selection, TodoComponentRecipe::Flat)
            .unwrap();
    assert!(verify_plan(&configured.planned_mask.plan));
    assert_eq!(
        prepare_advertised_native_stage_from_offer(
            &large,
            Some((selection, TodoComponentRecipe::Flat))
        )
        .unwrap()
        .planned_mask,
        configured.planned_mask
    );
    let mut unknown = large.clone();
    unknown.profile = "unsupported/native-profile".into();
    assert_eq!(
        prepare_advertised_native_stage_from_offer(&unknown, None)
            .unwrap()
            .planned_mask,
        generic.planned_mask,
        "general Host label is independent of its exact initialized native Backs"
    );
    assert_eq!(original.component, collection.component);
    assert_eq!(original.recipe, collection.recipe);
    assert_eq!(original.look, collection.look);
    let renderer = configured
        .planned_mask
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .find(|p| p.capability_id.as_str() == "renderer-conduitos-component")
        .unwrap();
    assert_eq!(renderer.host_calls.len(), 1);
    assert_eq!(renderer.host_calls[0].maximum_input_bytes, 256 * 1024);
    let mut relabelled = small.clone();
    relabelled.profile = large.profile.clone();
    assert!(prepare_collection_stage_from_offer(&relabelled).is_err());
    for id in [
        "renderer-conduitos",
        "renderer-conduitos-tee",
        "renderer-conduitos-resource-source",
        "renderer-conduitos-input",
    ] {
        let mut insufficient = large.clone();
        insufficient
            .capabilities
            .iter_mut()
            .find(|o| o.capability_id.as_str() == id)
            .unwrap()
            .limits
            .max_queue_bytes = MASK_BYTES as u32;
        assert!(prepare_collection_stage_from_offer(&insufficient).is_err());
    }
    let mut short_call = large.clone();
    short_call
        .capabilities
        .iter_mut()
        .find(|o| o.capability_id.as_str() == "renderer-conduitos-component")
        .unwrap()
        .host_calls[0]
        .maximum_input_bytes = MASK_BYTES as u32;
    assert!(
        prepare_collection_component_stage_from_offer(
            &short_call,
            selection,
            TodoComponentRecipe::Flat
        )
        .is_err()
    );
}

#[test]
fn explicit_native_profile_checks_actual_backs_without_relabelling_image_host() {
    let provider = BaseProviderEntry {
        base_id: "actual/display".into(),
        provider_instance_id: "actual/provider".into(),
        provider_generation: 7,
        implementation_id: "display/linear-framebuffer@1".into(),
        mechanism_family: "display/scanout".into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: Vec::new(),
        resources: vec![resource_offer("actual/show", SHOW_RESOURCE_CLASS, 1)],
    };
    for profile in [
        NativeMaskStorageProfile::Standard,
        NativeMaskStorageProfile::Collection,
    ] {
        let mut host = native_read_only_host_advertisement(
            &"actual/image-host".into(),
            &"actual/boot".into(),
            7,
            &provider,
            profile,
        );
        host.profile = "checked/raspberry-pi-3-host@1".into();
        let original = host.clone();
        let stage = prepare_native_stage_with_profile(&host, profile).unwrap();
        assert_eq!(
            NativeMaskStorageProfile::from_native_implementations(&host),
            Ok(profile)
        );
        assert_eq!(
            NativeMaskStorageProfile::owner_document_profile(&host),
            profile
        );
        assert_eq!(
            prepare_advertised_native_stage_from_offer(&host, None)
                .unwrap()
                .planned_mask,
            stage.planned_mask,
        );
        assert_eq!(host, original, "actual Host identity/profile remains exact");
        assert!(verify_plan(&stage.planned_mask.plan));
        let other = match profile {
            NativeMaskStorageProfile::Standard => NativeMaskStorageProfile::Collection,
            NativeMaskStorageProfile::Collection => NativeMaskStorageProfile::Standard,
        };
        assert!(prepare_native_stage_with_profile(&host, other).is_err());
        let mut wrong = host.clone();
        wrong
            .capabilities
            .iter_mut()
            .find(|offer| offer.capability_id.as_str() == "renderer-conduitos-tee")
            .unwrap()
            .implementation
            .execution_profile_id = "incorrect/execution-profile".into();
        assert!(prepare_native_stage_with_profile(&wrong, profile).is_err());
        assert!(prepare_advertised_native_stage_from_offer(&wrong, None).is_err());
        let mut small = host.clone();
        small
            .capabilities
            .iter_mut()
            .find(|offer| offer.capability_id.as_str() == "renderer-conduitos")
            .unwrap()
            .limits
            .max_queue_bytes = profile.bytes() - 1;
        assert!(prepare_native_stage_with_profile(&small, profile).is_err());
        assert!(prepare_advertised_native_stage_from_offer(&small, None).is_err());
    }
}
