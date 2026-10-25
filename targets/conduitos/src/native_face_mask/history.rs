//! A checked history graph beside a reachable complete same-Face inspector.
use super::*;
use conduit_plot::rust_binding::NativeRustBinding;

pub(super) fn prepare(
    configuration: &conduit_face::ConfiguredMaskConfiguration,
    face: &Face,
    width: u16,
    height: u16,
    local_input: bool,
    look: &conduit_face::AdmittedVisualLook,
) -> Result<Option<conduit_face::GraphicsScene>, String> {
    let recipe = conduit_core::StructuredInfoValue::from_canonical_bytes(
        configuration.recipe.canonical_value(),
    )
    .map_err(|_| String::from("HistoryRecipeEncoding"))?;
    let typed = conduit_face::ComponentRecipeInfo::from_structured(recipe.clone())
        .map_err(|_| String::from("HistoryRecipeEncoding"))?;
    if typed.identity() != conduit_face::HISTORY_COMPONENT_RECIPE_ID {
        return Ok(None);
    }
    if !local_input {
        return Err("HistoryInspectorInputUnavailable".into());
    }
    if width < 640 || height < 400 || look.visual.expression.text_scale_percent != 100 {
        return Err("HistoryPresentationExtent".into());
    }
    conduit_face::prepare_component_recipe(recipe, face)
        .map_err(|e| alloc::format!("HistoryRecipe({e:?})"))?;
    let graph = conduit_environment_history::HistoryGraph::prepare(
        face,
        conduit_environment_history::HISTORY_SUBJECT,
    )
    .map_err(|e| alloc::format!("HistoryGraph({e:?})"))?;
    let graph_scene = graph
        .native_scene_with_look(&look.selected)
        .map_err(|e| alloc::format!("HistoryScene({e:?})"))?;
    let bounds = conduit_face::LayoutRect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut scene = conduit_face::GraphicsScene::empty();
    scene
        .push(
            conduit_face::GraphicsCommand::rect(
                bounds,
                bounds,
                conduit_face::GraphicsPaintRole::Background,
                conduit_face::GraphicsShapeStyle::Fill,
            )
            .map_err(|e| alloc::format!("HistoryBackground({e:?})"))?,
        )
        .map_err(|e| alloc::format!("HistoryBackground({e:?})"))?;
    for command in graph_scene.commands() {
        scene
            .push(*command)
            .map_err(|e| alloc::format!("HistoryScene({e:?})"))?;
    }
    scene
        .push(
            conduit_face::GraphicsCommand::text(
                conduit_face::LayoutRect {
                    x: 16,
                    y: 370,
                    width: 610,
                    height: 20,
                },
                bounds,
                conduit_face::GraphicsPaintRole::Foreground,
                "F2: complete Face and actions; F10: temperature unit",
            )
            .map_err(|e| alloc::format!("HistoryDisclosure({e:?})"))?,
        )
        .map_err(|e| alloc::format!("HistoryDisclosure({e:?})"))?;
    Ok(Some(scene))
}
