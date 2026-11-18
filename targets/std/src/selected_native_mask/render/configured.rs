//! Exact retained checked configuration and authored Look/layout consumption.
//! The caller independently funds Source/decode/Look preparation before calling
//! these helpers; Scene bounds cover neither those owners nor compiler work.
use super::*;
use conduit_face::{
    AdmittedVisualLook, ConfiguredMaskConfiguration, PreparedComponentLayout, VisualUserConstraints,
};
use conduitos::native_face_scene::{FaceSceneError, NativeFaceScene};

pub(super) fn configuration_bytes(
    configuration: Option<&ConfiguredMaskConfiguration>,
) -> Option<usize> {
    let Some(configuration) = configuration else {
        return Some(0);
    };
    conduit_face::configured_mask_configuration_backing_bytes(configuration)?
        .checked_add(core::mem::size_of::<ConfiguredMaskConfiguration>())
}

pub(super) fn validate_binding(
    configuration: &ConfiguredMaskConfiguration,
    plan: &PlannedMaskPlot,
) -> Result<(), String> {
    let renderer = plan.show_placement();
    if configuration.placement_id != renderer.placement_id
        || configuration.host_id != renderer.host_id
        || configuration.boot_id != renderer.boot_id
        || configuration.execution_profile_id != renderer.execution_profile_id
        || configuration.implementation_id != renderer.implementation_id
        || configuration.component.source_document_id != plan.mask.plot_identity.source_document_id
    {
        return Err("native checked component differs from selected renderer".into());
    }
    for (key, expected) in [
        ("recipe", Some(&configuration.recipe)),
        ("look", Some(&configuration.look)),
        ("layout", configuration.layout.as_ref()),
    ] {
        let mut entries = renderer
            .configuration
            .iter()
            .filter(|entry| entry.key == key);
        let actual = entries.next();
        if entries.next().is_some()
            || match (actual, expected) {
                (Some(entry), Some(expected)) => !matches!(&entry.value,
                ConfigurationValue::Structured(value) if value == expected),
                (None, None) => false,
                _ => true,
            }
        {
            return Err("native checked component startup differs from exact Plan".into());
        }
    }
    Ok(())
}

pub(super) struct PreparedConfiguredScene<'a> {
    pub(super) structure: PreparedComponentLayout<'a>,
    pub(super) look: AdmittedVisualLook,
}
impl<'a> PreparedConfiguredScene<'a> {
    /// Requires the independent original preparation reservation. This consumes
    /// immutable checked startup values and borrows the original semantic Face.
    pub(super) fn prepare(
        configuration: &ConfiguredMaskConfiguration,
        face: &'a Face,
    ) -> Result<Self, String> {
        let requested = configuration
            .requested_look()
            .map_err(|error| format!("native checked Look:{error:?}"))?;
        let look = conduit_face::admit_visual_look(
            &requested,
            VisualUserConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: 150,
                maximum_supported_text_scale_percent: 200,
            },
        )
        .map_err(|error| format!("native checked Look admission:{error:?}"))?;
        let layout = configuration
            .layout
            .as_ref()
            .ok_or("native checked layout absent")?;
        let recipe =
            StructuredInfoValue::from_canonical_bytes(configuration.recipe.canonical_value())
                .map_err(|error| format!("native checked recipe decode:{error:?}"))?;
        let layout = StructuredInfoValue::from_canonical_bytes(layout.canonical_value())
            .map_err(|error| format!("native checked layout decode:{error:?}"))?;
        let structure = conduit_face::prepare_component_layout(layout, recipe, face)
            .map_err(|error| format!("native checked semantic layout:{error:?}"))?;
        Ok(Self { structure, look })
    }

    pub(super) fn scene(
        &self,
        width: u16,
        height: u16,
        interactive: bool,
    ) -> Result<NativeFaceScene, FaceSceneError> {
        NativeFaceScene::prepare_structured_look(
            &self.structure,
            width,
            height,
            interactive,
            &self.look,
        )
    }
}

#[path = "configured/retained.rs"]
mod retained;

#[cfg(test)]
#[path = "configured_tests.rs"]
mod tests;
