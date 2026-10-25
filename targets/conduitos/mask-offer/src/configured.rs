//! Configured native Back of the same ordinary portable component Fore.
use super::*;
use alloc::collections::BTreeMap;
use conduit_core::*;
use conduit_face::*;
use conduit_plot::{CanonicalBackCatalog, expand_canonical_plot_for_authoring_with_backs};

pub const NATIVE_COMPONENT_RENDERER_KIND: &str = "face/native-component-resource-mask";
const BACK_SOURCE: &str = "plot native-component-back (\n label: Text\n recipe: ComponentRecipeInfo\n look: LookInfo\n >> face: Face\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n spread: face/tee(\"Shared Face\")\n surface: face/show-resource-source(\"Output resource\")\n render: face/native-component-resource-mask(label = label, recipe = recipe, look = look)\n human: face/interaction(\"User actions\")\n face >> spread.source\n spread.face >> render.face\n surface.show-resource >> render.show-resource\n spread.face >> human.face\n render.show >> human.show\n render.show >> show\n human.interaction >> interaction\n}\n";

pub fn configured_resource_renderer_kind() -> Kind {
    configured_resource_renderer_kind_for_profile(NativeMaskStorageProfile::Standard)
}

fn configured_resource_renderer_kind_for_profile(profile: NativeMaskStorageProfile) -> Kind {
    let component = component_mask_kind(profile.bytes());
    let mut kind = resource_renderer_kind();
    kind.kind_id = kind_id(NATIVE_COMPONENT_RENDERER_KIND);
    kind.kind_contract_revision =
        KindIdentity::from("conduit.face/native-component-resource-mask@3");
    kind.startup_parameters = component.startup_parameters;
    kind.configuration = component.configuration;
    kind
}

pub fn configured_resource_renderer_offer() -> CapabilityOffer {
    configured_resource_renderer_offer_for_profile(NativeMaskStorageProfile::Standard)
}

pub(super) fn configured_resource_renderer_offer_for_profile(
    profile: NativeMaskStorageProfile,
) -> CapabilityOffer {
    let mut offer = BackOfferBuilder::new(
        configured_resource_renderer_kind_for_profile(profile),
        Back {
            capability_id: CapabilityId::from("renderer-conduitos-component"),
            execution_profile_id: ExecutionProfileId::from(profile.execution_profile()),
            implementation_id: ImplementationId::from("face/renderer-conduitos-component@1"),
            artifact_id: ArtifactId::from("conduitos/native-compositor@1"),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from("conduit.host/present@1"),
                target_kind: Some(kind_id("face/base/conduitos-surface@1")),
                maximum_in_flight: 1,
                maximum_input_bytes: profile.bytes(),
                maximum_output_bytes: profile.bytes(),
            }],
            resource_requirements: vec![],
            authority_requirements: vec![present_authority_requirement(kind_id(
                "face/base/conduitos-surface@1",
            ))],
        },
    )
    .build();
    if profile == NativeMaskStorageProfile::Collection {
        offer.limits.max_queue_bytes = profile.renderer_queue_bytes();
    }
    offer
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
    prepare_selected_component_stage_from_offer(host, look.into(), recipe)
}

pub fn prepare_selected_component_stage_from_offer(
    host: &HostAdvertisement,
    selection: ComponentVisualSelection,
    recipe: TodoComponentRecipe,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    layout_configured::prepare_offered_component_document(
        host,
        (TODO_COMPONENT_MASK_SOURCE, TODO_LAYOUT_MASK_SOURCE),
        selection.todo_root(recipe),
        NativeMaskStorageProfile::Standard,
    )
}

pub fn prepare_creche_stage_from_offer(
    host: &HostAdvertisement,
    recipe: CrecheComponentRecipe,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    prepare_selected_creche_stage_from_offer(host, recipe, ComponentVisualSelection::default())
}

pub fn prepare_selected_creche_stage_from_offer(
    host: &HostAdvertisement,
    recipe: CrecheComponentRecipe,
    selection: ComponentVisualSelection,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    prepare_typographic_creche_stage_from_offer(
        host,
        recipe,
        selection,
        VisualTypography::Editorial,
    )
}

/// Admit a complete canonical Look before publishing new typography/hit regions.
pub fn prepare_typographic_creche_stage_from_offer(
    host: &HostAdvertisement,
    recipe: CrecheComponentRecipe,
    selection: ComponentVisualSelection,
    typography: VisualTypography,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    layout_configured::prepare_offered_component_document(
        host,
        (CRECHE_COMPONENT_MASK_SOURCE, CRECHE_LAYOUT_MASK_SOURCE),
        selection.creche_root_with_typography(recipe, typography),
        NativeMaskStorageProfile::Standard,
    )
}

/// Select an exact complete navy Look through the ordinary reviewed Source path.
pub fn prepare_navy_creche_stage_from_offer(
    host: &HostAdvertisement,
    recipe: CrecheComponentRecipe,
    selection: ComponentVisualSelection,
    typography: VisualTypography,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    layout_configured::prepare_offered_component_document(
        host,
        (CRECHE_COMPONENT_MASK_SOURCE, CRECHE_LAYOUT_MASK_SOURCE),
        selection.creche_navy_root(recipe, typography),
        NativeMaskStorageProfile::Standard,
    )
}

pub(super) fn prepare_component_document(
    host: &HostAdvertisement,
    source: &str,
    root: &str,
    profile: NativeMaskStorageProfile,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    prepare_component_document_with_startup(host, source, root, profile, None)
}

pub(super) fn prepare_component_document_with_startup(
    host: &HostAdvertisement,
    source: &str,
    root: &str,
    profile: NativeMaskStorageProfile,
    inputs: Option<&[conduit_plot::CheckedStartupBinding]>,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    prepare_renderer_document_with_startup(
        host,
        source,
        root,
        profile,
        RendererContract {
            component: component_mask_kind(profile.bytes()),
            renderer: configured_resource_renderer_kind_for_profile(profile),
            kind_name: NATIVE_COMPONENT_RENDERER_KIND,
            back_source: BACK_SOURCE,
            back_root: "native-component-back",
            capability: "renderer-conduitos-component",
        },
        inputs,
    )
}

pub(super) struct RendererContract {
    pub component: Kind,
    pub renderer: Kind,
    pub kind_name: &'static str,
    pub back_source: &'static str,
    pub back_root: &'static str,
    pub capability: &'static str,
}

pub(super) fn prepare_renderer_document(
    host: &HostAdvertisement,
    source: &str,
    root: &str,
    profile: NativeMaskStorageProfile,
    contract: RendererContract,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    prepare_renderer_document_with_startup(host, source, root, profile, contract, None)
}

pub(super) fn prepare_renderer_document_with_startup(
    host: &HostAdvertisement,
    source: &str,
    root: &str,
    profile: NativeMaskStorageProfile,
    contract: RendererContract,
    inputs: Option<&[conduit_plot::CheckedStartupBinding]>,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    profile.admit_host(host, contract.capability)?;
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
        contract.renderer,
        contract.component.clone(),
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
        ("LookInfo", look_type()),
    ] {
        let profile = schema.profile().map_err(|_| MaskOfferError::InvalidPlan)?;
        startup
            .insert_structured_type(alias, schema.clone())
            .map_err(|_| MaskOfferError::InvalidPlan)?;
        startup
            .ensure_structured_type(profile.value_kind().as_str(), schema)
            .map_err(|_| MaskOfferError::InvalidPlan)?;
    }
    if contract
        .component
        .configuration
        .iter()
        .any(|field| field.key == "layout")
    {
        let schema = component_layout_type();
        let value_kind = schema
            .profile()
            .map_err(|_| MaskOfferError::InvalidPlan)?
            .value_kind()
            .clone();
        startup
            .insert_structured_type("ComponentLayoutInfo", schema.clone())
            .map_err(|_| MaskOfferError::InvalidPlan)?;
        startup
            .ensure_structured_type(value_kind.as_str(), schema)
            .map_err(|_| MaskOfferError::InvalidPlan)?;
    }
    let back = check_syntax_document(&parse_syntax_document(contract.back_source), &startup)
        .map_err(|_| MaskOfferError::InvalidPlan)?;
    let mut backs = CanonicalBackCatalog::new();
    backs
        .insert(&contract.component, &back, contract.back_root)
        .map_err(|_| MaskOfferError::InvalidPlan)?;
    let document = check_syntax_document(&parse_syntax_document(source), &startup)
        .map_err(|_| MaskOfferError::InvalidPlan)?;
    // Retain independent checked Root-input custody across both expansion and
    // source/Plan reproduction. Expanded configuration is never an input source.
    let prepared_inputs = inputs
        .map(|inputs| {
            conduit_plot::PreparedRootStartupInputs::prepare(&document, root, &catalog, inputs)
                .map_err(|_| MaskOfferError::InvalidPlan)
        })
        .transpose()?;
    let authoring = match &prepared_inputs {
        Some(inputs) => conduit_plot::expand_canonical_plot_for_authoring_with_startup(
            &document, root, &catalog, &backs, inputs,
        ),
        None => expand_canonical_plot_for_authoring_with_backs(&document, root, &catalog, &backs),
    }
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
                    profile.bytes()
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
        CapabilityId::from(contract.capability),
    )];
    let bases = [BaseImplementationId::from("conduit.base/local@1")];
    let options = || PlanningOptions {
        connection_bases: &empty,
        line_candidates: &lines,
        connection_item_capacity: 1,
        connection_byte_capacity: profile.bytes(),
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
    let selection = ConfiguredMaskPlanningInputs {
        hosts: core::slice::from_ref(host),
        placements: &placements,
        bases: &bases,
        options: options(),
        boundary_limits: &boundaries,
    };
    let renderer_kind = kind_id(contract.kind_name);
    let prepared = match &prepared_inputs {
        Some(inputs) => prepare_configured_mask_with_startup(
            &document,
            &authoring,
            &catalog,
            &backs,
            inputs,
            &planned,
            selection,
            &renderer_kind,
        ),
        None => prepare_configured_mask(
            &document,
            &authoring,
            &catalog,
            &backs,
            &planned,
            selection,
            &renderer_kind,
        ),
    }
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
mod tests;
