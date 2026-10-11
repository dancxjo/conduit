//! Realization configuration from the exact current planned renderer placement.
use super::*;
use conduit_presentation::{
    admit_visual_expression, prepare_component_recipe, visual_expression_from_info,
    AdmittedVisualExpression, ConfiguredMaskConfiguration, VisualUserConstraints,
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
    pub configuration: ConfiguredMaskConfiguration,
    pub expression: AdmittedVisualExpression,
    pub maximum_columns: u16,
    pub reflow_width: u16,
    pub gutter: u16,
    pub subject_regions: Vec<Option<usize>>,
    pub groups: Vec<u8>,
    pub fallback: Option<String>,
}

pub(super) fn prepare(
    basis: &HostBasis,
    planned: &conduit_presentation::PlannedMaskPlot,
    face: &Presentation,
) -> Result<Option<ComponentView>, String> {
    let host = crate::installed_browser::membership_advertisement(
        basis.host_id.clone(),
        basis.boot_id.clone(),
    );
    let Some(configuration) =
        crate::installed_browser::dom_mask::configured_mask_configuration(&host, planned)?
    else {
        return Ok(None);
    };
    let constraints = basis.visual_constraints.as_ref();
    let minimum = constraints.map_or(100, |value| value.minimum_text_scale_percent);
    if !(100..=200).contains(&minimum) {
        return Err("invalid browser user text scale".into());
    }
    let requested = conduit_core::StructuredInfoValue::from_canonical_bytes(
        configuration.expression.canonical_value(),
    )
    .map_err(|_| conduit_presentation::VisualExpressionRefusal::Encoding)
    .and_then(visual_expression_from_info);
    let expression = admit_visual_expression(
        requested,
        VisualUserConstraints {
            forced_colors: constraints.is_some_and(|value| value.forced_colors),
            reduced_motion: constraints.is_some_and(|value| value.reduced_motion),
            minimum_text_scale_percent: minimum,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .map_err(|error| format!("browser visual capacity: {error:?}"))?;
    let prepared = conduit_core::StructuredInfoValue::from_canonical_bytes(
        configuration.recipe.canonical_value(),
    )
    .map_err(|_| "component recipe encoding".to_owned())
    .and_then(|value| prepare_component_recipe(value, face).map_err(|error| format!("{error:?}")));
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
        maximum_columns,
        reflow_width,
        gutter,
        subject_regions,
        groups,
        fallback,
    }))
}
