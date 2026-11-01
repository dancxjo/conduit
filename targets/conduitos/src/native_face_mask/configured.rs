//! Checked configuration, selected Look admission and generic scene fallback.
use super::*;

impl NativeFaceMask {
    // Private shared preparation boundary: callers retain exact source/Plan custody.
    // Tests use actual checked Host configuration, not a fabricated config record.
    pub(super) fn prepare_component_scene(
        configuration: &conduit_face::ConfiguredMaskConfiguration,
        face: &Face,
        width: u16,
        height: u16,
        interaction_admitted: bool,
        constraints: conduit_face::VisualUserConstraints,
    ) -> Result<
        (
            NativeFaceScene,
            conduit_face::AdmittedVisualLook,
            Option<String>,
        ),
        NativeFaceMaskError,
    > {
        let mut fallback = None;
        let requested = configuration
            .requested_look()
            .map_err(|_| NativeFaceMaskError::Plan)?;
        let admitted = conduit_face::admit_visual_look(&requested, constraints)
            .map_err(|_| NativeFaceMaskError::Face)?;
        let recipe = conduit_core::StructuredInfoValue::from_canonical_bytes(
            configuration.recipe.canonical_value(),
        )
        .map_err(|_| NativeFaceMaskError::Plan)?;
        // Preparation failures and renderer capacity failures retain separate
        // reasons. Neither can turn a partial configured scene into a Show.
        let configured = if let Some(layout) = &configuration.layout {
            let layout =
                conduit_core::StructuredInfoValue::from_canonical_bytes(layout.canonical_value())
                    .map_err(|_| NativeFaceMaskError::Plan)?;
            conduit_face::prepare_component_layout(layout, recipe, face)
                .map(|prepared| {
                    NativeFaceScene::prepare_structured_look(
                        &prepared,
                        width,
                        height,
                        interaction_admitted,
                        &admitted,
                    )
                })
                .map_err(|error| alloc::format!("LayoutPreparation({error:?})"))
        } else {
            conduit_face::prepare_component_recipe(recipe, face)
                .map(|prepared| {
                    NativeFaceScene::prepare_configured_look(
                        face.clone(),
                        width,
                        height,
                        interaction_admitted,
                        &prepared,
                        &admitted,
                    )
                })
                .map_err(|error| alloc::format!("{error:?}"))
        };
        let scene = match configured {
            Ok(Ok(scene)) => Ok(scene),
            Ok(Err(
                error @ (FaceSceneError::InvalidExtent
                | FaceSceneError::DocumentBound
                | FaceSceneError::SceneBound
                | FaceSceneError::UnsupportedStructure),
            )) => {
                fallback = Some(alloc::format!("ConfiguredLayout({error:?})"));
                NativeFaceScene::prepare_visual_look(
                    face.clone(),
                    width,
                    height,
                    interaction_admitted,
                    &admitted,
                )
            }
            Ok(Err(error)) => Err(error),
            Err(reason) => {
                fallback = Some(reason);
                NativeFaceScene::prepare_visual_look(
                    face.clone(),
                    width,
                    height,
                    interaction_admitted,
                    &admitted,
                )
            }
        }
        .map_err(NativeFaceMaskError::Scene)?;
        Ok((scene, admitted, fallback))
    }
}

#[cfg(test)]
#[path = "metric_consumer_tests.rs"]
mod metric_consumer_tests;
