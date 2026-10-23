//! Planning/custody only. The ordinary advertisement does not select this Back.
use super::*;

const SOURCE: &str = r#"
plot structured-component (
 expression: VisualExpressionInfo
 >> face: Face
 interaction: FaceInteraction...| >>
 show: Show >>
) {
 render: face/component-mask(label = "Structured component", recipe = {gutter: 12, identity: "test/structured-recipe@1", maximum_columns: 1, reflow_width: 720, regions: [{group: 0, identity: "collection", maximum_subjects: 20, selector: role("Collection")}], version: 1}, expression = expression, layout = {candidates: [{nodes: [column({block_alignment: stretch(empty), break_policy: accessible_continuation(empty), children: [{block_span: 1, block_track: 0, inline_span: 1, inline_track: 0, node: 1}, {block_span: 1, block_track: 0, inline_span: 1, inline_track: 0, node: 2}], gap_em_sixteenths: 8, inline_grow: 0, inline_alignment: stretch(empty), padding_em_sixteenths: 8, inline_shrink: 1}), region(0), fallback(empty)], root: 0}], identity: "test/structured-layout@1", recipe_identity: "test/structured-recipe@1", version: 1})
 face >> render.face
 render.show >> show
 render.interaction >> interaction
}
plot structured-root (
 >> face: Face
 interaction: FaceInteraction...| >>
 show: Show >>
) {
 component: structured-component(expression = {density: compact(empty), identity: "test/look@1", motion: none(empty), palette: phosphor(empty), text_scale_percent: 100, typography: editorial(empty), version: 2})
 face >> component.face
 component.show >> show
 component.interaction >> interaction
}
"#;

fn host(profile: NativeMaskStorageProfile) -> HostAdvertisement {
    let provider = BaseProviderEntry {
        base_id: "layout/framebuffer".into(),
        provider_instance_id: "layout/provider".into(),
        provider_generation: 1,
        implementation_id: "conduitos/framebuffer@1".into(),
        mechanism_family: "conduitos.base/framebuffer@1".into(),
        enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: BaseLifecycle::Ready,
        capabilities: Vec::new(),
        resources: vec![resource_offer(
            "layout/surface",
            conduit_face::SHOW_RESOURCE_CLASS,
            1,
        )],
    };
    let mut host = renderer_host_for_profile(
        Adapter::Native,
        &"layout/host".into(),
        &"layout/boot".into(),
        1,
        Some(&provider),
        profile,
    );
    assert!(
        !host
            .capabilities
            .iter()
            .any(|offer| { offer.kind_id.as_str() == NATIVE_LAYOUT_COMPONENT_RENDERER_KIND })
    );
    host.capabilities
        .push(layout_resource_renderer_offer(profile));
    host
}

#[test]
fn explicit_selected_layout_back_retains_source_values_and_existing_storage() {
    for profile in [
        NativeMaskStorageProfile::Standard,
        NativeMaskStorageProfile::Collection,
    ] {
        let host = host(profile);
        let (stage, configuration) =
            prepare_layout_component_document(&host, SOURCE, "structured-root", profile).unwrap();
        assert!(configuration.layout.is_some());
        assert_eq!(
            configuration.component.source_document_id,
            stage.planned_mask.mask.plot_identity.source_document_id
        );
        assert_eq!(
            configuration.implementation_id.as_str(),
            "face/renderer-conduitos-layout-component@1"
        );
        let selected = stage
            .planned_mask
            .plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .find(|p| p.kind_id.as_str() == NATIVE_LAYOUT_COMPONENT_RENDERER_KIND)
            .unwrap();
        assert_eq!(
            selected
                .configuration
                .iter()
                .find(|f| f.key == "layout")
                .unwrap()
                .value,
            ConfigurationValue::Structured(configuration.layout.unwrap())
        );
        assert_eq!(
            profile.bytes(),
            if profile == NativeMaskStorageProfile::Standard {
                48 * 1024
            } else {
                256 * 1024
            }
        );
    }
}

#[test]
fn old_renderer_cannot_be_relabelled_as_layout_support() {
    let profile = NativeMaskStorageProfile::Standard;
    let mut host = host(profile);
    host.capabilities
        .retain(|offer| offer.capability_id.as_str() != NATIVE_LAYOUT_COMPONENT_CAPABILITY);
    assert!(prepare_layout_component_document(&host, SOURCE, "structured-root", profile).is_err());
    let mut old = configured::configured_resource_renderer_offer_for_profile(profile);
    old.capability_id = NATIVE_LAYOUT_COMPONENT_CAPABILITY.into();
    host.capabilities.push(old);
    assert!(prepare_layout_component_document(&host, SOURCE, "structured-root", profile).is_err());
}

#[test]
fn reviewed_creche_and_todo_sources_plan_both_structural_alternatives() {
    let profile = NativeMaskStorageProfile::Standard;
    let host = host(profile);
    for (source, root) in [
        (
            conduit_face::CRECHE_LAYOUT_MASK_SOURCE,
            "creche-mask-single",
        ),
        (
            conduit_face::CRECHE_LAYOUT_MASK_SOURCE,
            "creche-mask-columns",
        ),
        (
            conduit_face::TODO_LAYOUT_MASK_SOURCE,
            "todo-mask-flat-compact",
        ),
        (
            conduit_face::TODO_LAYOUT_MASK_SOURCE,
            "todo-mask-grouped-compact",
        ),
    ] {
        let (stage, configuration) =
            prepare_layout_component_document(&host, source, root, profile).unwrap();
        assert_eq!(stage.planned_mask.mask.plot_name, root);
        assert!(configuration.layout.is_some());
        assert_eq!(configuration.invocation_path[0], root);
        assert_eq!(configuration.host_id, host.host_id);
    }
}
