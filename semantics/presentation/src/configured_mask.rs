//! Source and selected-placement binding for configured ordinary Mask renderers.
//! This gate supplies configuration to a Back; schema and recipe realization
//! remain separate checks, and legacy unconfigured Mask admission is unchanged.
use crate::{MaskPlot, PlannedMaskPlot};
use alloc::{collections::BTreeMap, string::String, vec::Vec};
use conduit_core::{
    BaseImplementationId, CheckedPlotId, ConfigurationValue, HostAdvertisement, KindId,
    PlannedGear, SourceDocumentId, StructuredConfigurationValue,
};
use conduit_planner::{
    ConnectionQueueLimits, ForeBoundaryKey, PlacementChoices, PlanningOptions,
    plan_expanded_authoring_with_options,
};
use conduit_plot::{
    CanonicalBackCatalog, CanonicalStartupValue, CheckedSyntaxDocument, ExpandedAuthoringPlot,
    ExpandedGearProvenance, ProfileCatalog, expand_canonical_plot_for_authoring_with_backs,
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfiguredMaskRefusal {
    SourceExpansion,
    MaskPlan,
    BackPreparation,
    MissingRenderer,
    AmbiguousRenderer,
    RendererPlacement,
    RendererProvenance,
    RecipeNotSourceLiteral,
    Configuration,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConfiguredComponentSource {
    pub source_document_id: SourceDocumentId,
    pub checked_plot_id: CheckedPlotId,
    pub fore_id: String,
}

/// Planner-admitted selected Back plus canonical startup values from that placement.
/// This is not a resource reservation or successful renderer acknowledgement.
/// No consumer may substitute the public Show boundary's configuration.
pub struct PreparedConfiguredMask<'a> {
    pub component: ConfiguredComponentSource,
    pub component_invocation_path: Vec<String>,
    pub renderer: &'a PlannedGear,
    pub recipe: &'a StructuredConfigurationValue,
    pub expression: &'a StructuredConfigurationValue,
}

/// The same retained ordinary planning inputs used by the production Mask offer.
/// Hosts and choices come from the Host-owned offer, never the Look request.
pub struct ConfiguredMaskPlanningInputs<'a> {
    pub hosts: &'a [HostAdvertisement],
    pub placements: &'a PlacementChoices,
    pub bases: &'a [BaseImplementationId],
    pub options: PlanningOptions<'a>,
    pub boundary_limits: &'a BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>,
}

/// Independently reproduce expansion from the checked source before following
/// provenance. A copied source/Fore digest or self-consistent forged expansion
/// cannot stand in for this actual ordinary compiler operation.
pub fn prepare_configured_mask<'a>(
    document: &CheckedSyntaxDocument,
    expanded: &ExpandedAuthoringPlot,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    planned: &'a PlannedMaskPlot,
    selection: ConfiguredMaskPlanningInputs<'_>,
    renderer_kind: &KindId,
) -> Result<PreparedConfiguredMask<'a>, ConfiguredMaskRefusal> {
    let reproduced = expand_canonical_plot_for_authoring_with_backs(
        document,
        &expanded.expanded.name,
        catalog,
        backs,
    )
    .map_err(|_| ConfiguredMaskRefusal::SourceExpansion)?;
    if reproduced != *expanded {
        return Err(ConfiguredMaskRefusal::SourceExpansion);
    }
    let mask = MaskPlot::admit(expanded).map_err(|_| ConfiguredMaskRefusal::MaskPlan)?;
    if mask != planned.mask || PlannedMaskPlot::admit(&mask, &planned.plan).is_err() {
        return Err(ConfiguredMaskRefusal::MaskPlan);
    }
    let reproduced_plan = plan_expanded_authoring_with_options(
        &reproduced,
        selection.hosts,
        selection.placements,
        selection.bases,
        selection.options,
        selection.boundary_limits,
    )
    .map_err(|_| ConfiguredMaskRefusal::BackPreparation)?;
    if reproduced_plan != planned.plan {
        return Err(ConfiguredMaskRefusal::BackPreparation);
    }
    let mut renderers = expanded
        .expanded
        .gears
        .iter()
        .filter(|gear| &gear.kind_id == renderer_kind);
    let renderer = renderers
        .next()
        .ok_or(ConfiguredMaskRefusal::MissingRenderer)?;
    if renderers.next().is_some() {
        return Err(ConfiguredMaskRefusal::AmbiguousRenderer);
    }
    let mut placements = planned
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .filter(|placement| placement.gear_id == renderer.gear_id);
    let placement = placements
        .next()
        .ok_or(ConfiguredMaskRefusal::RendererPlacement)?;
    if placements.next().is_some()
        || placement.kind_id != renderer.kind_id
        || placement.kind_contract_revision != renderer.kind_contract_revision
        || placement.configuration != renderer.configuration
        || placement.inputs != renderer.inputs
        || placement.outputs != renderer.outputs
        || placement.semantic_contract != renderer.semantic_contract
    {
        return Err(ConfiguredMaskRefusal::RendererPlacement);
    }
    let recipe = structured_field(placement, "recipe")?;
    let expression = structured_field(placement, "expression")?;
    let provenance = expanded
        .expanded
        .provenance
        .iter()
        .find(|source| source.gear_id == renderer.gear_id.as_str())
        .ok_or(ConfiguredMaskRefusal::RendererProvenance)?;
    let (component, component_invocation_path) =
        bind_source_recipe(document, backs, &expanded.expanded.name, provenance, recipe)?;
    Ok(PreparedConfiguredMask {
        component,
        component_invocation_path,
        renderer: placement,
        recipe,
        expression,
    })
}

fn structured_field<'a>(
    placement: &'a PlannedGear,
    name: &str,
) -> Result<&'a StructuredConfigurationValue, ConfiguredMaskRefusal> {
    let mut fields = placement
        .configuration
        .iter()
        .filter(|field| field.key == name);
    let value = fields.next().ok_or(ConfiguredMaskRefusal::Configuration)?;
    if fields.next().is_some() {
        return Err(ConfiguredMaskRefusal::Configuration);
    }
    match &value.value {
        ConfigurationValue::Structured(value) => Ok(value),
        _ => Err(ConfiguredMaskRefusal::Configuration),
    }
}

/// Follow only the exact expanded ancestor invocation, never a sibling literal.
/// Re-expansion already authenticated this path and every parameter substitution.
fn bind_source_recipe(
    document: &CheckedSyntaxDocument,
    backs: &CanonicalBackCatalog,
    root_name: &str,
    provenance: &ExpandedGearProvenance,
    recipe: &StructuredConfigurationValue,
) -> Result<(ConfiguredComponentSource, Vec<String>), ConfiguredMaskRefusal> {
    // Plot names themselves may contain '/'; use the authenticated root name.
    let root = document
        .plots
        .iter()
        .find(|plot| plot.name == root_name)
        .ok_or(ConfiguredMaskRefusal::RendererProvenance)?;
    if provenance.plot_path.first().map(|name| name.as_str()) != Some(root_name) {
        return Err(ConfiguredMaskRefusal::RendererProvenance);
    }
    // Keep compiler-owned path segments: a legal Gear name can contain '/'.
    let path = provenance
        .plot_path
        .iter()
        .skip(1)
        .chain(core::iter::once(&provenance.source_gear));
    let mut current = root;
    let mut literal_seen = false;
    let mut component = None;
    let mut source_document_id = &document.source_document_id;
    let mut invocation_path = alloc::vec![root_name.into()];
    for (depth, name) in path.enumerate() {
        if depth >= 64 {
            return Err(ConfiguredMaskRefusal::RendererProvenance);
        }
        let gear = current
            .gears
            .iter()
            .find(|gear| gear.name.as_deref() == Some(name.as_str()))
            .ok_or(ConfiguredMaskRefusal::RendererProvenance)?;
        let recipe_binding = gear
            .startup_bindings
            .iter()
            .find(|binding| binding.name == "recipe");
        if literal_seen && recipe_binding.is_none() {
            return Err(ConfiguredMaskRefusal::RecipeNotSourceLiteral);
        }
        if let Some(binding) = gear
            .startup_bindings
            .iter()
            .find(|binding| binding.name == "recipe")
        {
            let CanonicalStartupValue::Structured(value) = &binding.value else {
                return Err(ConfiguredMaskRefusal::RecipeNotSourceLiteral);
            };
            if let Some(value) = value.try_concrete() {
                if literal_seen
                    || value
                        .canonical_bytes()
                        .map_err(|_| ConfiguredMaskRefusal::Configuration)?
                        != recipe.canonical_value()
                {
                    return Err(ConfiguredMaskRefusal::RecipeNotSourceLiteral);
                }
                if current.startup_parameters.len() != 1
                    || current.startup_parameters[0].name != "expression"
                {
                    return Err(ConfiguredMaskRefusal::RecipeNotSourceLiteral);
                }
                let expression_binding = gear
                    .startup_bindings
                    .iter()
                    .find(|binding| binding.name == "expression")
                    .ok_or(ConfiguredMaskRefusal::Configuration)?;
                let CanonicalStartupValue::Structured(expression_value) = &expression_binding.value
                else {
                    return Err(ConfiguredMaskRefusal::Configuration);
                };
                if expression_value.parameter_reference() != Some("expression") {
                    return Err(ConfiguredMaskRefusal::Configuration);
                }
                component = Some((
                    ConfiguredComponentSource {
                        source_document_id: source_document_id.clone(),
                        checked_plot_id: current.checked_plot_id.clone(),
                        fore_id: conduit_core::compute_checked_front_fingerprint(
                            &current.runtime_front,
                        ),
                    },
                    invocation_path.clone(),
                ));
                literal_seen = true;
            } else if !literal_seen || value.parameter_reference() != Some("recipe") {
                return Err(ConfiguredMaskRefusal::RecipeNotSourceLiteral);
            }
        }
        if literal_seen {
            let expression_binding = gear
                .startup_bindings
                .iter()
                .find(|binding| binding.name == "expression")
                .ok_or(ConfiguredMaskRefusal::Configuration)?;
            let CanonicalStartupValue::Structured(value) = &expression_binding.value else {
                return Err(ConfiguredMaskRefusal::Configuration);
            };
            if value.parameter_reference() != Some("expression") {
                return Err(ConfiguredMaskRefusal::Configuration);
            }
        }
        if let Some(back) = backs.checked_definition(&conduit_core::kind_id(&gear.kind)) {
            current = &back.plot;
            source_document_id = &back.realization.source_document_id;
        } else if let Some(child) = document.plots.iter().find(|plot| plot.name == gear.kind) {
            current = child;
            source_document_id = &document.source_document_id;
        }
        invocation_path.push(name.clone());
    }
    if !literal_seen {
        return Err(ConfiguredMaskRefusal::RecipeNotSourceLiteral);
    }
    component.ok_or(ConfiguredMaskRefusal::RecipeNotSourceLiteral)
}

/// Owned source/placement evidence retained by the renderer preparation join.
/// Canonical values belong to the actual selected gear, not the Show boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConfiguredMaskConfiguration {
    pub component: ConfiguredComponentSource,
    pub invocation_path: Vec<String>,
    pub placement_id: conduit_core::PlacementId,
    pub host_id: conduit_core::HostId,
    pub boot_id: conduit_core::BootId,
    pub execution_profile_id: conduit_core::ExecutionProfileId,
    pub implementation_id: conduit_core::ImplementationId,
    pub recipe: StructuredConfigurationValue,
    pub expression: StructuredConfigurationValue,
}

impl PreparedConfiguredMask<'_> {
    pub fn configuration(&self) -> ConfiguredMaskConfiguration {
        ConfiguredMaskConfiguration {
            component: self.component.clone(),
            invocation_path: self.component_invocation_path.clone(),
            placement_id: self.renderer.placement_id.clone(),
            host_id: self.renderer.host_id.clone(),
            boot_id: self.renderer.boot_id.clone(),
            execution_profile_id: self.renderer.execution_profile_id.clone(),
            implementation_id: self.renderer.implementation_id.clone(),
            recipe: self.recipe.clone(),
            expression: self.expression.clone(),
        }
    }
}
