//! Configured native Back of the same ordinary portable component Fore.
use super::*;
use alloc::collections::BTreeMap;
use conduit_core::*;
use conduit_face::*;
use conduit_plot::{CanonicalBackCatalog, expand_canonical_plot_for_authoring_with_backs};

pub const NATIVE_COMPONENT_RENDERER_KIND: &str = "face/native-component-resource-mask";
const BACK_SOURCE: &str = "plot native-component-back (\n label: Text\n recipe: ComponentRecipeInfo\n expression: VisualExpressionInfo\n >> face: Face\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n spread: face/tee(\"Shared Face\")\n surface: face/show-resource-source(\"Output resource\")\n render: face/native-component-resource-mask(label = label, recipe = recipe, expression = expression)\n human: face/interaction(\"User actions\")\n face >> spread.source\n spread.face >> render.face\n surface.show-resource >> render.show-resource\n spread.face >> human.face\n render.show >> human.show\n render.show >> show\n human.interaction >> interaction\n}\n";

pub fn configured_resource_renderer_kind() -> Kind {
    let component = component_mask_kind(MASK_BYTES as u32);
    let mut kind = resource_renderer_kind();
    kind.kind_id = kind_id(NATIVE_COMPONENT_RENDERER_KIND);
    kind.kind_contract_revision =
        KindIdentity::from("conduit.face/native-component-resource-mask@1");
    kind.startup_parameters = component.startup_parameters;
    kind.configuration = component.configuration;
    kind
}

pub fn configured_resource_renderer_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        configured_resource_renderer_kind(),
        Back {
            capability_id: CapabilityId::from("renderer-conduitos-component"),
            execution_profile_id: ExecutionProfileId::from("conduitos/bounded-mask@1"),
            implementation_id: ImplementationId::from("face/renderer-conduitos-component@1"),
            artifact_id: ArtifactId::from("conduitos/native-compositor@1"),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from("conduit.host/present@1"),
                target_kind: Some(kind_id("face/base/conduitos-surface@1")),
                maximum_in_flight: 1,
                maximum_input_bytes: MASK_BYTES as u32,
                maximum_output_bytes: MASK_BYTES as u32,
            }],
            resource_requirements: vec![],
            authority_requirements: vec![present_authority_requirement(kind_id(
                "face/base/conduitos-surface@1",
            ))],
        },
    )
    .build()
}

pub fn prepare_component_stage(
    host_id: &HostId,
    boot_id: &BootId,
    generation: u64,
    surface_provider: Option<&BaseProviderEntry>,
    look: ComponentMaskLook,
    recipe: TodoComponentRecipe,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    let host = renderer_host(
        Adapter::Native,
        host_id,
        boot_id,
        generation,
        surface_provider,
    );
    prepare_component_stage_from_offer(&host, look, recipe)
}

pub fn prepare_component_stage_from_offer(
    host: &HostAdvertisement,
    look: ComponentMaskLook,
    recipe: TodoComponentRecipe,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    if host.protocol_version != PROTOCOL_VERSION
        || host.profile.as_str() != "conduitos/mask-host@1"
        || host.offer_generation.0 == 0
    {
        return Err(MaskOfferError::InvalidPlan);
    }
    let mut catalog = ProfileCatalog::new();
    for projection in [
        face_interaction_kind_projection(),
        face_tee_kind_projection(),
    ] {
        catalog
            .insert(projection)
            .map_err(|_| MaskOfferError::InvalidPlan)?;
    }
    for kind in [
        show_resource_source_kind(),
        configured_resource_renderer_kind(),
        component_mask_kind(MASK_BYTES as u32),
    ] {
        catalog
            .insert_kind(kind)
            .map_err(|_| MaskOfferError::InvalidPlan)?;
    }
    let mut startup = catalog
        .startup_catalog()
        .map_err(|_| MaskOfferError::InvalidPlan)?;
    install_mask_plot_value_aliases(&mut startup).map_err(|_| MaskOfferError::InvalidPlan)?;
    for (alias, schema) in [
        ("ComponentRecipeInfo", component_recipe_type()),
        ("VisualExpressionInfo", visual_expression_type()),
    ] {
        let profile = schema.profile().map_err(|_| MaskOfferError::InvalidPlan)?;
        startup
            .insert_structured_type(alias, schema.clone())
            .map_err(|_| MaskOfferError::InvalidPlan)?;
        startup
            .insert_structured_type(profile.value_kind().as_str(), schema)
            .map_err(|_| MaskOfferError::InvalidPlan)?;
    }
    let back = check_syntax_document(&parse_syntax_document(BACK_SOURCE), &startup)
        .map_err(|_| MaskOfferError::InvalidPlan)?;
    let mut backs = CanonicalBackCatalog::new();
    backs
        .insert(
            &component_mask_kind(MASK_BYTES as u32),
            &back,
            "native-component-back",
        )
        .map_err(|_| MaskOfferError::InvalidPlan)?;
    let document =
        check_syntax_document(&parse_syntax_document(TODO_COMPONENT_MASK_SOURCE), &startup)
            .map_err(|_| MaskOfferError::InvalidPlan)?;
    let authoring = expand_canonical_plot_for_authoring_with_backs(
        &document,
        todo_component_mask_root(look, recipe),
        &catalog,
        &backs,
    )
    .map_err(|_| MaskOfferError::InvalidPlan)?;
    let mask = MaskPlot::admit(&authoring).map_err(|_| MaskOfferError::InvalidPlan)?;
    let placements = default_expanded_placements(&authoring.expanded, core::slice::from_ref(host))
        .map_err(|_| MaskOfferError::InvalidPlan)?;
    let boundaries: BTreeMap<_, _> = [
        (PortDirection::Input, "face"),
        (PortDirection::Output, "show"),
        (PortDirection::Output, "interaction"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: port_id(name),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: if name == "interaction" {
                    MAX_FACE_INTERACTION_BYTES as u32
                } else {
                    MASK_BYTES as u32
                },
            },
        )
    })
    .collect();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let grants = vec![authority_grant(
        "conduitos/component-mask/present",
        &present_authority_requirement(kind_id("face/base/conduitos-surface@1")),
        host.host_id.clone(),
        host.boot_id.clone(),
        CapabilityId::from("renderer-conduitos-component"),
    )];
    let bases = [BaseImplementationId::from("conduit.base/local@1")];
    let options = || PlanningOptions {
        connection_bases: &empty,
        line_candidates: &lines,
        connection_item_capacity: 1,
        connection_byte_capacity: MASK_BYTES as u32,
        authority_grants: &grants,
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let plan = plan_expanded_authoring_with_options(
        &authoring,
        core::slice::from_ref(host),
        &placements,
        &bases,
        options(),
        &boundaries,
    )
    .map_err(|_| MaskOfferError::InvalidPlan)?;
    let planned = PlannedMaskPlot::admit(&mask, &plan).map_err(|_| MaskOfferError::InvalidPlan)?;
    let prepared = prepare_configured_mask(
        &document,
        &authoring,
        &catalog,
        &backs,
        &planned,
        ConfiguredMaskPlanningInputs {
            hosts: core::slice::from_ref(host),
            placements: &placements,
            bases: &bases,
            options: options(),
            boundary_limits: &boundaries,
        },
        &kind_id(NATIVE_COMPONENT_RENDERER_KIND),
    )
    .map_err(|_| MaskOfferError::InvalidPlan)?;
    let configuration = prepared.configuration();
    Ok((
        MaskStage {
            planned_mask: planned,
        },
        configuration,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_checked_component_two_looks_and_recipes() {
        let provider = BaseProviderEntry {
            base_id: "native/component/framebuffer".into(),
            provider_instance_id: "native/component/provider".into(),
            provider_generation: 1,
            implementation_id: "conduitos/framebuffer@1".into(),
            mechanism_family: "conduitos.base/framebuffer@1".into(),
            enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
            lifecycle: BaseLifecycle::Ready,
            capabilities: Vec::new(),
            resources: vec![resource_offer(
                "native/component/surface",
                SHOW_RESOURCE_CLASS,
                1,
            )],
        };
        let host = renderer_host(
            Adapter::Native,
            &HostId::from("native/component"),
            &BootId::from("native/boot"),
            1,
            Some(&provider),
        );
        for recipe in [TodoComponentRecipe::Flat, TodoComponentRecipe::Grouped] {
            let (compact, a) =
                prepare_component_stage_from_offer(&host, ComponentMaskLook::Compact, recipe)
                    .unwrap();
            let (spacious, b) =
                prepare_component_stage_from_offer(&host, ComponentMaskLook::Spacious, recipe)
                    .unwrap();
            assert_eq!(a.component, b.component);
            assert_eq!(a.recipe, b.recipe);
            assert_ne!(a.expression, b.expression);
            assert_ne!(
                compact.planned_mask.plan.plan_id,
                spacious.planned_mask.plan.plan_id
            );
            assert_eq!(
                a.implementation_id.as_str(),
                "face/renderer-conduitos-component@1"
            );
        }
        let mut foreign = host;
        foreign
            .capabilities
            .retain(|offer| offer.capability_id.as_str() != "renderer-conduitos-component");
        assert!(
            prepare_component_stage_from_offer(
                &foreign,
                ComponentMaskLook::Compact,
                TodoComponentRecipe::Flat
            )
            .is_err()
        );
    }
}
