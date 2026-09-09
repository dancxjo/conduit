//! Native projection of the Tour specimen's Gear contracts.
use super::*;
use conduit_presentation::{GraphicsPath, GraphicsPoint};
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
    let mut previous_output = None;
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
        let heading_len = text.len();
        text.push_str(if observations.is_some() {
            "Last run\n"
        } else {
            "Unobserved\n"
        });
        let (mut input, mut output) = (None, None);
        for (direction, ports) in [("in", &contract.inputs), ("out", &contract.outputs)] {
            for port in ports {
                text.push('\n');
                if port.port_id.as_str() == "text" {
                    let anchor = port_anchor(card, &text, direction == "out");
                    if direction == "out" {
                        output = anchor;
                    } else {
                        input = anchor;
                    }
                }
                text.push_str(&alloc::format!(
                    "{direction} {}\n{}",
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
        let text_bounds = super::inset(card);
        // Sub-glyph-width cards remain valid clipped surfaces. Their ASCII
        // heading can be measured at one glyph even though no glyph fits.
        let heading_height =
            crate::display::text_height(&text[..heading_len], text_bounds.width.max(8))
                .map_err(|_| TourWorkspaceSceneRefusal::MissingRegion)?
                .saturating_sub(16);
        scene
            .push(
                GraphicsCommand::text(text_bounds, card, paint, &text[..heading_len])
                    .map_err(TourWorkspaceSceneRefusal::Graphics)?,
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?;
        scene
            .push(
                GraphicsCommand::text(
                    LayoutRect {
                        y: text_bounds.y + heading_height as i16,
                        height: text_bounds.height.saturating_sub(heading_height).max(1),
                        ..text_bounds
                    },
                    card,
                    GraphicsPaintRole::Foreground,
                    &text[heading_len..],
                )
                .map_err(TourWorkspaceSceneRefusal::Graphics)?,
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?;
        // The canonical source wires words.text -> change.text -> result.text.
        // Anchors come from those exact typed Port rows, never card centers.
        if let (Some(start), Some(end)) = (previous_output, input) {
            let path = cord_path(start, end).map_err(TourWorkspaceSceneRefusal::Graphics)?;
            scene
                .push(
                    GraphicsCommand::path(path, bounds, GraphicsPaintRole::Status)
                        .map_err(TourWorkspaceSceneRefusal::Graphics)?,
                )
                .map_err(TourWorkspaceSceneRefusal::Graphics)?;
        }
        previous_output = output;
    }
    Ok(())
}

fn port_anchor(card: LayoutRect, preceding_text: &str, output: bool) -> Option<GraphicsPoint> {
    let text_bounds = super::inset(card);
    let height = crate::display::text_height(preceding_text, text_bounds.width).ok()?;
    // Measurement includes the next empty line; its center is eight pixels
    // above the bottom of that line in the pinned sixteen-pixel font.
    let y = i32::from(text_bounds.y) + i32::from(height) - 8;
    if y >= i32::from(card.y) + i32::from(card.height) {
        return None;
    }
    Some(GraphicsPoint {
        x: if output {
            card.x + card.width as i16 - 1
        } else {
            card.x
        },
        y: i16::try_from(y).ok()?,
    })
}

fn cord_path(start: GraphicsPoint, end: GraphicsPoint) -> Result<GraphicsPath, GraphicsError> {
    if start.y == end.y {
        return GraphicsPath::new(&[start, end]);
    }
    let middle = start.x + (end.x - start.x) / 2;
    GraphicsPath::new(&[
        start,
        GraphicsPoint {
            x: middle,
            y: start.y,
        },
        GraphicsPoint {
            x: middle,
            y: end.y,
        },
        end,
    ])
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
    alloc::format!("{}...", &escaped[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_cords_join_measured_port_rows_with_one_command_each() {
        let state = TourWorkspaceState::canonical(1, TourWorkspacePhase::PatchbayOpen);
        let layout = super::super::layout_for_state(1280, 800, &state).unwrap();
        let graph = graphics_rect(layout.patchbay).unwrap();
        let scene = super::super::scene_for_state(1280, 800, &state).unwrap();
        let paths: alloc::vec::Vec<_> = scene
            .commands()
            .iter()
            .filter_map(GraphicsCommand::path_geometry)
            .collect();
        assert_eq!(paths.len(), 2);
        assert_eq!(scene.commands().len(), 19);
        let words = card_bounds(graph, 0);
        let change = card_bounds(graph, 1);
        let result = card_bounds(graph, 2);
        assert_eq!(
            paths[0].points()[0],
            GraphicsPoint {
                x: words.x + words.width as i16 - 1,
                y: words.y + 80
            }
        );
        assert_eq!(
            paths[0].points().last().unwrap(),
            &GraphicsPoint {
                x: change.x,
                y: change.y + 80
            }
        );
        assert_eq!(
            paths[1].points()[0],
            GraphicsPoint {
                x: change.x + change.width as i16 - 1,
                y: change.y + 128
            }
        );
        assert_eq!(
            paths[1].points().last().unwrap(),
            &GraphicsPoint {
                x: result.x,
                y: result.y + 80
            }
        );
        assert_eq!(paths[1].points().len(), 4);
        assert!(
            port_anchor(
                LayoutRect {
                    height: 32,
                    ..change
                },
                "name\nkind\nstate\n\n",
                true
            )
            .is_none()
        );
    }

    #[test]
    fn hit_regions_match_card_edges_and_exclude_headers_and_gaps() {
        for selected in [false, true] {
            let mut state = TourWorkspaceState::canonical(1, TourWorkspacePhase::PatchbayOpen);
            if selected {
                state.selected_patchbay_subject = Some("meet-one-gear/change".into());
            }
            for (width, height) in [(640, 480), (1280, 800), (1920, 1080)] {
                let layout = super::super::layout_for_state(width, height, &state).unwrap();
                let bounds = graphics_rect(layout.patchbay).unwrap();
                for index in 0..3 {
                    let card = card_bounds(bounds, index);
                    let x = card.x as u16;
                    let y = card.y as u16;
                    let hit = |x, y| super::super::hits_card(&layout, x, y).unwrap();
                    assert!(hit(x, y));
                    assert!(hit(x + card.width - 1, y + card.height - 1));
                    assert!(!hit(x - 1, y));
                    assert!(!hit(x + card.width, y));
                    assert!(!hit(x, y - 1));
                    assert!(!hit(x, y + card.height));
                }
            }
        }
    }

    #[test]
    fn previews_distinguish_absence_empty_and_bounded_unicode() {
        assert_eq!(preview(None), "unobserved");
        assert_eq!(preview(Some("")), "\"\"");
        assert_eq!(preview(Some("hello")), "\"hello\"");
        let long = preview(Some("中文中文中文中文中文中文中文中文"));
        assert!(long.len() <= 26);
        assert!(long.ends_with("..."));
    }
}
