//! Native paragraph layout and scrolling; prose remains owned by Tour Presentation.
use super::*;
use conduit_presentation::MAX_GRAPHICS_TEXT_BYTES;
use conduit_tour_model::TOUR_LESSON_SUBJECT;

const TOP: u16 = 40;
const GAP: u16 = 12;

pub(super) fn append(
    scene: &mut GraphicsScene,
    bounds: LayoutRect,
    state: &TourWorkspaceState,
) -> Result<(), TourWorkspaceSceneRefusal> {
    // Tiny synthetic viewports cannot display a glyph; the ordinary shell
    // admits at least 320x240. Retain their existing geometry-only behavior.
    if bounds.width < 32 || bounds.height <= TOP {
        return Ok(());
    }
    let clip = LayoutRect {
        y: bounds.y + TOP as i16,
        height: bounds.height - TOP,
        ..bounds
    };
    // This module owns the prose viewport. Clear it before placing the authored
    // blocks so stale generic lesson-status text or retained pixels cannot sit
    // underneath the measured paragraph layout. The pane title above TOP stays.
    scene
        .push(
            GraphicsCommand::rect(
                clip,
                clip,
                GraphicsPaintRole::Background,
                GraphicsShapeStyle::Fill,
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?,
        )
        .map_err(TourWorkspaceSceneRefusal::Graphics)?;
    let presentation = state
        .workspace_presentation()
        .map_err(|_| TourWorkspaceSceneRefusal::MissingRegion)?;
    let prefix = alloc::format!("{TOUR_LESSON_SUBJECT}/");
    let mut y = TOP;
    for item in presentation
        .text
        .iter()
        .filter(|item| item.subject.starts_with(&prefix))
    {
        let subject = presentation
            .subjects
            .iter()
            .find(|subject| subject.identity == item.subject)
            .ok_or(TourWorkspaceSceneRefusal::MissingRegion)?;
        let paint = if subject.label == "Heading" {
            GraphicsPaintRole::Accent
        } else {
            GraphicsPaintRole::Foreground
        };
        let mut remaining = item.text.as_str();
        while !remaining.is_empty() {
            let end = chunk_end(remaining);
            let chunk = &remaining[..end];
            let width = bounds.width - 16;
            // Give each chunk exactly the vertical extent measured by the same
            // text layout used by raster placement. The next chunk starts below
            // that extent, so wrapped lines cannot overlap their neighbors.
            let height = crate::display::text_height(chunk, width)
                .map_err(|_| TourWorkspaceSceneRefusal::MissingRegion)?;
            let text_bounds = LayoutRect {
                x: bounds.x + 8,
                y: bounds.y
                    + i16::try_from(y).map_err(|_| TourWorkspaceSceneRefusal::MissingRegion)?,
                width,
                height,
            };
            scene
                .push(
                    GraphicsCommand::text(text_bounds, clip, paint, chunk)
                        .map_err(TourWorkspaceSceneRefusal::Graphics)?,
                )
                .map_err(TourWorkspaceSceneRefusal::Graphics)?;
            y = y
                .checked_add(height)
                .ok_or(TourWorkspaceSceneRefusal::MissingRegion)?;
            if y > 4096 {
                return Err(TourWorkspaceSceneRefusal::MissingRegion);
            }
            remaining = remaining[end..].trim_start();
        }
        y = y
            .checked_add(GAP)
            .ok_or(TourWorkspaceSceneRefusal::MissingRegion)?;
    }
    Ok(())
}

fn chunk_end(text: &str) -> usize {
    if text.len() <= MAX_GRAPHICS_TEXT_BYTES {
        return text.len();
    }
    let mut end = MAX_GRAPHICS_TEXT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end]
        .rfind(char::is_whitespace)
        .filter(|&index| index > 0)
        .unwrap_or(end)
}

/// The original scene is retained so moving the lesson never moves the graph,
/// source, result, focus geometry, or status. No new runtime truth is invented.
#[cfg(any(test, all(target_arch = "x86_64", feature = "native-compositor")))]
pub(crate) fn scrolled(scene: &GraphicsScene, offset: u16) -> Result<GraphicsScene, GraphicsError> {
    let mut result = GraphicsScene::empty();
    for command in scene.commands() {
        let command = if is_prose(command) {
            let bounds = LayoutRect {
                y: command
                    .bounds
                    .y
                    .checked_sub(i16::try_from(offset).map_err(|_| GraphicsError::InvalidGeometry)?)
                    .ok_or(GraphicsError::InvalidGeometry)?,
                ..command.bounds
            };
            GraphicsCommand::text(bounds, command.clip, command.paint, command.payload())?
        } else {
            *command
        };
        result.push(command)?;
    }
    Ok(result)
}

#[cfg(any(test, all(target_arch = "x86_64", feature = "native-compositor")))]
pub(crate) fn extent(scene: &GraphicsScene) -> Option<(u16, u16)> {
    let mut commands = scene.commands().iter().filter(|command| is_prose(command));
    let first = commands.next()?;
    let last = commands.next_back().unwrap_or(first);
    let content_height = u16::try_from(
        i32::from(last.bounds.y) - i32::from(first.clip.y)
            + i32::from(last.bounds.height)
            + i32::from(GAP),
    )
    .ok()?;
    Some((first.clip.height, content_height))
}

// Only this module places text below TOP in the leftmost pane. The pane title
// above TOP and the other workspace regions retain their own geometry.
#[cfg(any(test, all(target_arch = "x86_64", feature = "native-compositor")))]
fn is_prose(command: &GraphicsCommand) -> bool {
    command.kind == conduit_presentation::GraphicsCommandKind::Text
        && command.clip.x == 0
        && command.clip.y == TOP as i16
        && command.bounds.x == 8
        && command.bounds.y >= TOP as i16
}
