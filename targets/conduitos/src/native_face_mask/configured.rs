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
            conduit_face::AdmittedVisualExpression,
            Option<String>,
        ),
        NativeFaceMaskError,
    > {
        let mut fallback = None;
        let requested = conduit_core::StructuredInfoValue::from_canonical_bytes(
            configuration.expression.canonical_value(),
        )
        .map_err(|_| conduit_face::VisualExpressionRefusal::Encoding)
        .and_then(conduit_face::visual_expression_from_info);
        let expression = conduit_face::admit_visual_expression(requested, constraints)
            .map_err(|_| NativeFaceMaskError::Face)?;
        let recipe = conduit_core::StructuredInfoValue::from_canonical_bytes(
            configuration.recipe.canonical_value(),
        )
        .map_err(|_| NativeFaceMaskError::Plan)?;
        let scene = match conduit_face::prepare_component_recipe(recipe, face) {
            Ok(prepared) => {
                let configured = NativeFaceScene::prepare_configured(
                    face.clone(),
                    width,
                    height,
                    interaction_admitted,
                    &prepared,
                    &expression,
                );
                match configured {
                    Err(
                        error @ (FaceSceneError::InvalidExtent
                        | FaceSceneError::DocumentBound
                        | FaceSceneError::SceneBound),
                    ) => {
                        fallback = Some(alloc::format!("ConfiguredLayout({error:?})"));
                        NativeFaceScene::prepare_visual_fallback(
                            face.clone(),
                            width,
                            height,
                            interaction_admitted,
                            &expression,
                        )
                    }
                    result => result,
                }
            }
            Err(error) => {
                fallback = Some(alloc::format!("{error:?}"));
                NativeFaceScene::prepare_visual_fallback(
                    face.clone(),
                    width,
                    height,
                    interaction_admitted,
                    &expression,
                )
            }
        }
        .map_err(NativeFaceMaskError::Scene)?;
        Ok((scene, expression, fallback))
    }
}
