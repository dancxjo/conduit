use super::*;
#[test]
fn compiled_native_template_is_exact_ordinary_source_plan_without_route_or_show() {
    let provider = BaseProviderEntry {
        base_id: "canonical/native-surface".into(),
        provider_instance_id: "canonical/native-surface/1".into(),
        provider_generation: 1,
        implementation_id: "conduitos/framebuffer@1".into(),
        mechanism_family: "conduitos.base/framebuffer@1".into(),
        enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: BaseLifecycle::Ready,
        capabilities: Vec::new(),
        resources: Vec::from([
            resource_offer(
                "canonical/input",
                "conduit.resource/conduitos-human-input@1",
                1,
            ),
            resource_offer("canonical/surface", conduit_face::SHOW_RESOURCE_CLASS, 1),
        ]),
    };
    let mut ad = crate::native_host_advertisement(
        &"canonical/native-template".into(),
        &"canonical/native-template/boot".into(),
        1,
        &provider,
    );
    ad.bases[0].capability_ids = ad
        .capabilities
        .iter()
        .filter(|c| c.kind_id.as_str() != conduit_face::FACE_TEE_KIND)
        .map(|c| c.capability_id.clone())
        .collect();
    ad.resources = provider.resources;
    let ordinary = crate::prepare_stage_from_offer(crate::Adapter::Native, &ad)
        .unwrap()
        .planned_mask;
    let receipt = canonical_native_graphical_template_preparation();
    let compiled = receipt.construct();
    assert_eq!(compiled.planned_mask(), &ordinary);
    assert!(conduit_core::verify_plan(&compiled.planned_mask().plan));
    assert!(
        conduit_face::PlannedMaskPlot::admit(
            &compiled.planned_mask().mask,
            &compiled.planned_mask().plan
        )
        .is_ok()
    );
    assert_eq!(receipt.preparation_bytes(), receipt.retained_bytes());
}
