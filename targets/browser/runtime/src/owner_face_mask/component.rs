//! Realization configuration from the exact current planned renderer placement.
use super::*;
use conduit_face::{
    admit_visual_look, prepare_component_recipe, AdmittedVisualExpression,
    ConfiguredMaskConfiguration, VisualUserConstraints,
};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct VisualConstraints {
    #[serde(default)]
    pub forced_colors: bool,
    #[serde(default)]
    pub reduced_motion: bool,
    pub minimum_text_scale_percent: u16,
}

#[derive(Clone, Serialize)]
pub(super) struct ComponentView {
    #[serde(serialize_with = "super::component_wire::configuration")]
    pub configuration: ConfiguredMaskConfiguration,
    pub expression: AdmittedVisualExpression,
    pub selected_look: conduit_core::StructuredConfigurationValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipe_identity: Option<String>,
    pub maximum_columns: u16,
    pub reflow_width: u16,
    pub gutter: u16,
    pub subject_regions: Vec<Option<usize>>,
    pub groups: Vec<u8>,
    pub fallback: Option<String>,
}

pub(super) fn prepare(
    basis: &HostBasis,
    planned: &conduit_face::PlannedMaskPlot,
    face: &Face,
    host: &conduit_core::HostAdvertisement,
) -> Result<Option<ComponentView>, String> {
    let Some(configuration) =
        crate::installed_browser::dom_mask::configured_mask_configuration(host, planned)?
    else {
        return Ok(None);
    };
    let constraints = basis.visual_constraints.as_ref();
    let minimum = constraints.map_or(100, |value| value.minimum_text_scale_percent);
    if !(100..=200).contains(&minimum) {
        return Err("invalid browser user text scale".into());
    }
    let requested = configuration
        .requested_look()
        .map_err(|error| format!("browser configured Look: {error:?}"))?;
    let admitted = admit_visual_look(
        &requested,
        VisualUserConstraints {
            forced_colors: constraints.is_some_and(|value| value.forced_colors),
            reduced_motion: constraints.is_some_and(|value| value.reduced_motion),
            minimum_text_scale_percent: minimum,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .map_err(|error| format!("browser visual capacity: {error:?}"))?;
    use conduit_plot::rust_binding::NativeRustBinding;
    let selected = admitted
        .selected
        .into_structured()
        .map_err(|error| format!("browser selected Look: {error:?}"))?;
    let selected_look = conduit_core::StructuredConfigurationValue::new(
        selected
            .value_type()
            .profile()
            .map_err(|error| format!("{error:?}"))?
            .value_kind()
            .clone(),
        selected
            .canonical_bytes()
            .map_err(|error| format!("{error:?}"))?,
    )
    .ok_or_else(|| "browser selected Look encoding".to_owned())?;
    let expression = admitted.visual;
    let prepared = conduit_core::StructuredInfoValue::from_canonical_bytes(
        configuration.recipe.canonical_value(),
    )
    .map_err(|_| "component recipe encoding".to_owned())
    .and_then(|value| prepare_component_recipe(value, face).map_err(|error| format!("{error:?}")));
    let recipe_identity = prepared
        .as_ref()
        .ok()
        .map(|prepared| prepared.recipe.identity().to_owned());
    let (maximum_columns, reflow_width, gutter, subject_regions, groups, fallback) = match prepared
    {
        Ok(prepared) => (
            *prepared.recipe.maximum_columns(),
            *prepared.recipe.reflow_width(),
            *prepared.recipe.gutter(),
            prepared.subject_regions,
            prepared
                .recipe
                .regions()
                .iter()
                .map(|region| *region.group())
                .collect(),
            None,
        ),
        Err(reason) => (
            1,
            720,
            12,
            vec![None; face.subjects.len()],
            vec![],
            Some(reason),
        ),
    };
    Ok(Some(ComponentView {
        configuration,
        expression,
        selected_look,
        recipe_identity,
        maximum_columns,
        reflow_width,
        gutter,
        subject_regions,
        groups,
        fallback,
    }))
}
