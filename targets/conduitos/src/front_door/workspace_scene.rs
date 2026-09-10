//! Quiet foreground surface for the born Body, projected from its current journey.
use super::Error;
use crate::{
    display::PixelTarget,
    product_journey::{JourneyProjection, JourneyStatus},
};
use alloc::format;
use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, GraphicsTextRole,
    LayoutRect,
};

pub(super) fn scene(
    journey: &JourneyProjection,
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
        "Conduit",
        GraphicsPaintRole::Foreground,
        GraphicsTextRole::Muted,
    )?;
    text(
        &mut scene,
        screen,
        64,
        journey.friendly_name.as_deref().unwrap_or("My Body"),
        GraphicsPaintRole::Accent,
        GraphicsTextRole::Title,
    )?;
    text(
        &mut scene,
        screen,
        104,
        "Keyboard canvas",
        GraphicsPaintRole::Foreground,
        GraphicsTextRole::Heading,
    )?;
    let status = match journey.status {
        JourneyStatus::BornLulled => "Body born. Preparing to wake its Form...",
        JourneyStatus::Awake => "Body awake. Planning its Form...",
        JourneyStatus::Planned => "Form ready. Starting its Play...",
        JourneyStatus::Playing => "Listening. Type to send a key through your Form.",
        JourneyStatus::ResultVisible => "Play completed. Your result is below.",
        JourneyStatus::Stopped => "Play stopped.",
        JourneyStatus::Lulled => "Body lulled. Its Form remains included.",
        _ => "Preparing your Body...",
    };
    text(
        &mut scene,
        screen,
        144,
        status,
        GraphicsPaintRole::Foreground,
        GraphicsTextRole::Status,
    )?;
    let mut result_notice_y = 248;
    if let Some(result) = &journey.result {
        result_notice_y = result_text(&mut scene, screen, 208, result)?;
    }
    if journey.result_omitted_bytes > 0 {
        text(
            &mut scene,
            screen,
            result_notice_y,
            "Showing recent output.",
            GraphicsPaintRole::Foreground,
            GraphicsTextRole::Muted,
        )?;
    }
    if let Some(refusal) = refusal {
        text(
            &mut scene,
            screen,
            288,
            "Wake could not finish. Details:",
            GraphicsPaintRole::Status,
            GraphicsTextRole::Warning,
        )?;
        text(
            &mut scene,
            screen,
            320,
            refusal,
            GraphicsPaintRole::Status,
            GraphicsTextRole::Code,
        )?;
    }
    let footer_y = i16::try_from(screen.height.saturating_sub(48)).map_err(|_| Error::Scene)?;
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
        GraphicsTextRole::Body,
    )?;
    Ok(scene)
}

fn result_text(
    scene: &mut GraphicsScene,
    screen: LayoutRect,
    mut y: i16,
    value: &str,
) -> Result<i16, Error> {
    if value.is_empty() {
        text(
            scene,
            screen,
            y,
            "Result is empty.",
            GraphicsPaintRole::Muted,
            GraphicsTextRole::Status,
        )?;
        return y.checked_add(40).ok_or(Error::Scene);
    }
    let mut remaining = value;
    while !remaining.is_empty() {
        let mut split = remaining
            .len()
            .min(conduit_presentation::MAX_GRAPHICS_TEXT_BYTES);
        while !remaining.is_char_boundary(split) {
            split -= 1;
        }
        text(
            scene,
            screen,
            y,
            &remaining[..split],
            GraphicsPaintRole::Accent,
            GraphicsTextRole::Body,
        )?;
        remaining = &remaining[split..];
        y = y.checked_add(40).ok_or(Error::Scene)?;
    }
    Ok(y)
}

fn text(
    scene: &mut GraphicsScene,
    screen: LayoutRect,
    y: i16,
    value: &str,
    role: GraphicsPaintRole,
    typography: GraphicsTextRole,
) -> Result<(), Error> {
    let bounds = LayoutRect {
        x: 32,
        y,
        width: screen.width.saturating_sub(64),
        height: 48,
    };
    scene
        .push(
            GraphicsCommand::text(bounds, screen, role, value)
                .and_then(|command| command.with_text_role(typography))
                .map_err(|_| Error::Scene)?,
        )
        .map_err(|_| Error::Scene)
}
