//! Checked configuration, selected Look admission and generic scene fallback.
use super::*;
use conduit_plot::rust_binding::NativeRustBinding;

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
        let expression = &admitted.visual;
        let recipe = conduit_core::StructuredInfoValue::from_canonical_bytes(
            configuration.recipe.canonical_value(),
        )
        .map_err(|_| NativeFaceMaskError::Plan)?;
        // History has a separate graph mode, but its complete inspector always
        // retains the same full Face, including Body/context and owner actions.
        if conduit_face::ComponentRecipeInfo::from_structured(recipe.clone())
            .map_err(|_| NativeFaceMaskError::Plan)?
            .identity()
            == conduit_face::HISTORY_COMPONENT_RECIPE_ID
        {
            let scene = NativeFaceScene::prepare_visual_fallback(
                face.clone(),
                width,
                height,
                interaction_admitted,
                expression,
            )
            .map_err(NativeFaceMaskError::Scene)?;
            return Ok((scene, admitted, None));
        }
        // Preparation failures and renderer capacity failures retain separate
        // reasons. Neither can turn a partial configured scene into a Show.
        let configured = if let Some(layout) = &configuration.layout {
            let layout =
                conduit_core::StructuredInfoValue::from_canonical_bytes(layout.canonical_value())
                    .map_err(|_| NativeFaceMaskError::Plan)?;
            conduit_face::prepare_component_layout(layout, recipe, face)
                .map(|prepared| {
                    NativeFaceScene::prepare_structured(
                        &prepared,
                        width,
                        height,
                        interaction_admitted,
                        expression,
                    )
                })
                .map_err(|error| alloc::format!("LayoutPreparation({error:?})"))
        } else {
            conduit_face::prepare_component_recipe(recipe, face)
                .map(|prepared| {
                    NativeFaceScene::prepare_configured(
                        face.clone(),
                        width,
                        height,
                        interaction_admitted,
                        &prepared,
                        expression,
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
                NativeFaceScene::prepare_visual_fallback(
                    face.clone(),
                    width,
                    height,
                    interaction_admitted,
                    expression,
                )
            }
            Ok(Err(error)) => Err(error),
            Err(reason) => {
                fallback = Some(reason);
                NativeFaceScene::prepare_visual_fallback(
                    face.clone(),
                    width,
                    height,
                    interaction_admitted,
                    expression,
                )
            }
        }
        .map_err(NativeFaceMaskError::Scene)?;
        Ok((scene, admitted, fallback))
    }
}
