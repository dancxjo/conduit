//! Native projection of the Tour specimen's Gear contracts.
use super::*;
use conduit_tour_model::{CANONICAL_PATCHBAY_GEARS, canonical_gear_contract};

pub(super) fn card_bounds(bounds: LayoutRect, index: usize) -> LayoutRect {
    let column = bounds.width / 3;
    LayoutRect {
        x: bounds
            .x
            .saturating_add(index as i16 * column as i16)
            .saturating_add(8),
        y: bounds.y.saturating_add(64),
        width: column.saturating_sub(16).max(1),
        height: bounds.height.saturating_sub(80).clamp(1, 240),
    }
}

pub(super) fn append(
    scene: &mut GraphicsScene,
    bounds: LayoutRect,
    state: &TourWorkspaceState,
    observations: Option<&crate::text_composition::TextObservations>,
) -> Result<(), TourWorkspaceSceneRefusal> {
    for (index, gear) in CANONICAL_PATCHBAY_GEARS.into_iter().enumerate() {
        let contract =
            canonical_gear_contract(gear).ok_or(TourWorkspaceSceneRefusal::MissingRegion)?;
        let card = card_bounds(bounds, index);
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
        text.push_str(if observations.is_some() {
            "Last run\n"
        } else {
            "Unobserved\n"
        });
        for (direction, ports) in [("in", &contract.inputs), ("out", &contract.outputs)] {
            for port in ports {
                text.push_str(&alloc::format!(
                    "\n{direction} {}\n{}",
                    port.port_id.as_str(),
                    port.value_kind.as_str()
                ));
                let value = observations.and_then(|observed| {
                    match (gear, direction, port.port_id.as_str()) {
                        ("meet-one-gear/change", "in", "text") => observed.upper_input.text(),
                        ("meet-one-gear/change", "out", "text") => observed.upper_output.text(),
                        ("meet-one-gear/result", "in", "text") => {
                            observed.presentation_input.text()
                        }
                        _ => None,
                    }
                });
                text.push_str("\n= ");
                text.push_str(&preview(value));
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

// A card is a bounded preview, not a replacement for the exact observation.
fn preview(value: Option<&str>) -> alloc::string::String {
    let Some(value) = value else {
        return "unobserved".into();
    };
    let escaped = alloc::format!("{value:?}");
    if escaped.len() <= 26 {
        return escaped;
    }
    let mut end = 23;
    while !escaped.is_char_boundary(end) {
        end -= 1;
    }
    alloc::format!("{}…", &escaped[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_distinguish_absence_empty_and_bounded_unicode() {
        assert_eq!(preview(None), "unobserved");
        assert_eq!(preview(Some("")), "\"\"");
        assert_eq!(preview(Some("hello")), "\"hello\"");
        let long = preview(Some("中文中文中文中文中文中文中文中文"));
        assert!(long.len() <= 26);
        assert!(long.ends_with('…'));
    }
}
