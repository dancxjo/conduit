//! Quiet foreground surface for the born Body, projected from its current journey.
use super::Error;
use crate::{
    display::PixelTarget,
    product_journey::{JourneyProjection, JourneyStatus},
};
use alloc::format;
use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, LayoutRect,
};

pub(super) fn scene(
    journey: &JourneyProjection,
    workspace: Option<&crate::product_journey::WorkspaceProjection>,
    refusal: Option<&str>,
    display: &impl PixelTarget,
) -> Result<GraphicsScene, Error> {
    let format = display.format().validate().map_err(Error::Display)?;
    let screen = LayoutRect {
        x: 0,
        y: 0,
        width: u16::try_from(format.width).map_err(|_| Error::Scene)?,
        height: u16::try_from(format.height).map_err(|_| Error::Scene)?,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::rect(
                screen,
                screen,
                GraphicsPaintRole::Background,
                GraphicsShapeStyle::Fill,
            )
            .map_err(|_| Error::Scene)?,
        )
        .map_err(|_| Error::Scene)?;
    text(
        &mut scene,
        screen,
        24,
        "CONDUIT",
        GraphicsPaintRole::Foreground,
    )?;
    text(
        &mut scene,
        screen,
        64,
        journey.friendly_name.as_deref().unwrap_or("My Body"),
        GraphicsPaintRole::Accent,
    )?;
    text(
        &mut scene,
        screen,
        104,
        workspace
            .and_then(|workspace| workspace.forms.iter().find(|form| form.foreground))
            .map_or("Keyboard canvas", |form| form.title),
        GraphicsPaintRole::Foreground,
    )?;
    let status = match journey.status {
        JourneyStatus::BornLulled => "Body born. Preparing to wake its Forms...",
        JourneyStatus::Awake => "Body awake. Planning its Forms...",
        JourneyStatus::Planned => "Forms ready. Starting their Play...",
        JourneyStatus::Playing => "Listening. Type to send a key through your Form.",
        JourneyStatus::ResultVisible => "Play completed. Your result is below.",
        JourneyStatus::Stopped => "Play stopped.",
        JourneyStatus::Lulled => "Body lulled. Its Forms remain included.",
        _ => "Preparing your Body...",
    };
    text(
        &mut scene,
        screen,
        144,
        status,
        GraphicsPaintRole::Foreground,
    )?;
    if let Some(result) = &journey.result {
        text(&mut scene, screen, 208, result, GraphicsPaintRole::Accent)?;
    }
    if journey.result_omitted_bytes > 0 {
        text(
            &mut scene,
            screen,
            248,
            "Showing recent output.",
            GraphicsPaintRole::Foreground,
        )?;
    }
    if let Some(refusal) = refusal {
        text(
            &mut scene,
            screen,
            352,
            "Wake could not finish. Details:",
            GraphicsPaintRole::Status,
        )?;
        text(&mut scene, screen, 384, refusal, GraphicsPaintRole::Status)?;
    }
    let footer_y = i16::try_from(screen.height.saturating_sub(48)).map_err(|_| Error::Scene)?;
    if let Some(workspace) = workspace {
        let labels = workspace
            .forms
            .iter()
            .map(|form| format!("{}{}", if form.foreground { "> " } else { "" }, form.title))
            .collect::<alloc::vec::Vec<_>>()
            .join("   ·   ");
        text(
            &mut scene,
            screen,
            footer_y - 40,
            &format!("{labels}   [Tab switches]"),
            GraphicsPaintRole::Foreground,
        )?;
    }
    let lifecycle_action = match journey.status {
        JourneyStatus::Playing => "  ·  F8 Stop",
        JourneyStatus::Stopped | JourneyStatus::ResultVisible => "  ·  F7 Lull",
        _ => "",
    };
    text(
        &mut scene,
        screen,
        footer_y,
        &format!(
            "{}  ·  F2 details  ·  F9 Tour{}",
            journey.status.as_str(),
            lifecycle_action
        ),
        GraphicsPaintRole::Foreground,
    )?;
    Ok(scene)
}

fn text(
    scene: &mut GraphicsScene,
    screen: LayoutRect,
    y: i16,
    value: &str,
    role: GraphicsPaintRole,
) -> Result<(), Error> {
    let mut rest = value;
    let mut row = y;
    while !rest.is_empty() {
        let mut end = rest
            .len()
            .min(conduit_presentation::MAX_GRAPHICS_TEXT_BYTES);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        let bounds = LayoutRect {
            x: 32,
            y: row,
            width: screen.width.saturating_sub(64),
            height: 48,
        };
        scene
            .push(
                GraphicsCommand::text(bounds, screen, role, &rest[..end])
                    .map_err(|_| Error::Scene)?,
            )
            .map_err(|_| Error::Scene)?;
        rest = &rest[end..];
        row = row.checked_add(48).ok_or(Error::Scene)?;
    }
    Ok(())
}
