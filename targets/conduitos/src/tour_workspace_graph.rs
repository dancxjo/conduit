//! Native projection of the Tour specimen's Gear contracts.
use super::*;
use conduit_tour_model::{CANONICAL_PATCHBAY_GEARS, canonical_gear_contract};

pub(super) fn append(
    scene: &mut GraphicsScene,
    bounds: LayoutRect,
    state: &TourWorkspaceState,
) -> Result<(), TourWorkspaceSceneRefusal> {
    let column = bounds.width / 3;
    for (index, gear) in CANONICAL_PATCHBAY_GEARS.into_iter().enumerate() {
        let contract =
            canonical_gear_contract(gear).ok_or(TourWorkspaceSceneRefusal::MissingRegion)?;
        let card = LayoutRect {
            x: bounds
                .x
                .saturating_add((index as i16) * column as i16)
                .saturating_add(8),
            y: bounds.y.saturating_add(64),
            width: column.saturating_sub(16).max(1),
            height: bounds.height.saturating_sub(80).clamp(1, 180),
        };
        let paint = if state.selected_patchbay_subject.as_deref() == Some(gear) {
            GraphicsPaintRole::Accent
        } else if state.hovered_patchbay_subject.as_deref() == Some(gear) {
            GraphicsPaintRole::Status
        } else {
            GraphicsPaintRole::Foreground
        };
        let mut text = alloc::format!(
            "{}\n{}\n",
            gear.rsplit('/').next().unwrap_or(gear),
            contract.kind_id.as_str()
        );
        for (direction, ports) in [("in", &contract.inputs), ("out", &contract.outputs)] {
            for port in ports {
                text.push_str(&alloc::format!(
                    "\n{direction} {}\n{}",
                    port.port_id.as_str(),
                    port.value_kind.as_str()
                ));
            }
        }
        scene
            .push(
                GraphicsCommand::rect(card, bounds, paint, GraphicsShapeStyle::Stroke)
                    .map_err(TourWorkspaceSceneRefusal::Graphics)?,
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?;
        scene
            .push(
                GraphicsCommand::text(super::inset(card), card, paint, &text)
                    .map_err(TourWorkspaceSceneRefusal::Graphics)?,
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?;
        if index < 2 {
            let cord = LayoutRect {
                x: card.x.saturating_add(card.width as i16),
                y: card.y.saturating_add((card.height / 2) as i16),
                width: 16,
                height: 2,
            };
            scene
                .push(
                    GraphicsCommand::rect(
                        cord,
                        bounds,
                        GraphicsPaintRole::Status,
                        GraphicsShapeStyle::Fill,
                    )
                    .map_err(TourWorkspaceSceneRefusal::Graphics)?,
                )
                .map_err(TourWorkspaceSceneRefusal::Graphics)?;
        }
    }
    Ok(())
}
