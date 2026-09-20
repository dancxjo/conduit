//! Quiet foreground surface for the born Body, projected from its current journey.
use super::Error;
use crate::{
    display::{PixelTarget, SPACE_LG, SPACE_SM, SPACE_XL},
    product_journey::{JourneyProjection, JourneyStatus},
};

const TEXT_STEP: i16 = (SPACE_XL + SPACE_SM) as i16;
const TEXT_BOX_HEIGHT: u16 = SPACE_XL + SPACE_SM * 2;
const FOOTER_RESERVE: u16 = TEXT_BOX_HEIGHT + TEXT_STEP as u16;
use alloc::format;
use conduit_presentation::{
    ApplicationComponent, ApplicationView, GraphicsCommand, GraphicsPaintRole, GraphicsScene,
    GraphicsShapeStyle, GraphicsTextRole, LayoutRect,
};

pub(super) fn scene(
    journey: &JourneyProjection,
    foreground_title: &str,
    application_view: Option<&ApplicationView>,
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
        SPACE_LG as i16,
        "Conduit",
        GraphicsPaintRole::Foreground,
        GraphicsTextRole::Muted,
    )?;
    text(
        &mut scene,
        screen,
        (SPACE_XL * 2) as i16,
        journey.friendly_name.as_deref().unwrap_or("My Body"),
        GraphicsPaintRole::Accent,
        GraphicsTextRole::Title,
    )?;
    text(
        &mut scene,
        screen,
        (SPACE_XL * 3 + SPACE_SM) as i16,
        foreground_title,
        GraphicsPaintRole::Foreground,
        GraphicsTextRole::Heading,
    )?;
    let status = match journey.status {
        JourneyStatus::BornLulled => "Body born. Its installed forms are not executing yet.",
        JourneyStatus::Awake => "Body awake. Planning its installed forms...",
        JourneyStatus::Planned => "Installed forms admitted. Starting their play...",
        JourneyStatus::QuiescentAwaitingInput => "Quiescent. Type to continue this play.",
        JourneyStatus::SemanticCompleted => "Play semantically completed. Your result is below.",
        JourneyStatus::InputUnavailable => journey
            .loss_kind
            .map(|kind| kind.recovery())
            .unwrap_or("Input unavailable. Inspect details before recovery."),
        JourneyStatus::Stopped => "Play stopped.",
        JourneyStatus::Lulled => "Body lulled. Its installed forms remain included.",
        _ => "Preparing your body...",
    };
    text(
        &mut scene,
        screen,
        (SPACE_XL * 4 + SPACE_SM * 2) as i16,
        status,
        GraphicsPaintRole::Foreground,
        GraphicsTextRole::Status,
    )?;
    let mut result_notice_y = 248;
    if let Some(view) = application_view {
        result_notice_y = application_text(&mut scene, screen, 192, view)?;
    } else if let Some(result) = &journey.result {
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
    let footer_y =
        i16::try_from(screen.height.saturating_sub(TEXT_BOX_HEIGHT)).map_err(|_| Error::Scene)?;
    let lifecycle_action = match journey.status {
        JourneyStatus::QuiescentAwaitingInput => "  ·  F8 Stop  ·  F7 Lull",
        JourneyStatus::InputUnavailable
        | JourneyStatus::Stopped
        | JourneyStatus::SemanticCompleted => "  ·  F7 Lull",
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

fn application_text(
    scene: &mut GraphicsScene,
    screen: LayoutRect,
    mut y: i16,
    view: &ApplicationView,
) -> Result<i16, Error> {
    view.validate().map_err(|_| Error::Presentation)?;
    for node in &view.nodes {
        let (role, typography) = match node.component {
            ApplicationComponent::Heading => (GraphicsPaintRole::Accent, GraphicsTextRole::Heading),
            ApplicationComponent::Status => {
                (GraphicsPaintRole::Foreground, GraphicsTextRole::Status)
            }
            ApplicationComponent::Definition | ApplicationComponent::CodeBlock => {
                (GraphicsPaintRole::Foreground, GraphicsTextRole::Code)
            }
            ApplicationComponent::Paragraph => {
                (GraphicsPaintRole::Foreground, GraphicsTextRole::Body)
            }
            ApplicationComponent::Button => (GraphicsPaintRole::Accent, GraphicsTextRole::Body),
            _ => continue,
        };
        let value = if node.value.is_empty() {
            node.text.as_str()
        } else if node.text.is_empty() {
            node.value.as_str()
        } else {
            // Definition values carry the exact inspected identity. Prefer them
            // over their short semantic label on the finite native surface.
            node.value.as_str()
        };
        let limit = i16::try_from(screen.height.saturating_sub(FOOTER_RESERVE))
            .map_err(|_| Error::Scene)?;
        let mut remaining = value;
        while !remaining.is_empty() && y < limit {
            let mut split = remaining
                .len()
                .min(conduit_presentation::MAX_GRAPHICS_TEXT_BYTES);
            while !remaining.is_char_boundary(split) {
                split -= 1;
            }
            text(scene, screen, y, &remaining[..split], role, typography)?;
            remaining = &remaining[split..];
            y = y.checked_add(TEXT_STEP).ok_or(Error::Scene)?;
        }
        if y >= limit {
            break;
        }
    }
    Ok(y)
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
        return y.checked_add(TEXT_STEP).ok_or(Error::Scene);
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
        y = y.checked_add(TEXT_STEP).ok_or(Error::Scene)?;
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
        x: SPACE_XL as i16,
        y,
        width: screen.width.saturating_sub(SPACE_XL * 2),
        height: TEXT_BOX_HEIGHT,
    };
    scene
        .push(
            GraphicsCommand::text(bounds, screen, role, value)
                .and_then(|command| command.with_text_role(typography))
                .map_err(|_| Error::Scene)?,
        )
        .map_err(|_| Error::Scene)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::String, vec};
    use conduit_presentation::{
        ApplicationAction, ApplicationEventKind, ApplicationNodeState, ApplicationViewNode,
    };

    #[test]
    fn application_projection_keeps_exact_identity_and_action_on_native_surface() {
        let exact = "expanded/form/exact";
        let view = ApplicationView {
            revision: 1,
            nodes: vec![
                ApplicationViewNode {
                    parent: None,
                    component: ApplicationComponent::Shell,
                    key: "shell".into(),
                    text: "Patchbay".into(),
                    value: String::new(),
                    value_capacity: 0,
                    action: None,
                    state: ApplicationNodeState::Ready,
                },
                ApplicationViewNode {
                    parent: Some(0),
                    component: ApplicationComponent::Definition,
                    key: "subject".into(),
                    text: "Form".into(),
                    value: exact.into(),
                    value_capacity: 64,
                    action: None,
                    state: ApplicationNodeState::Ready,
                },
                ApplicationViewNode {
                    parent: Some(0),
                    component: ApplicationComponent::Button,
                    key: "inspect".into(),
                    text: "Inspect next".into(),
                    value: String::new(),
                    value_capacity: 0,
                    action: Some(0),
                    state: ApplicationNodeState::Ready,
                },
            ],
            actions: vec![ApplicationAction {
                id: "patchbay.inspect.next".into(),
                event: ApplicationEventKind::Activate,
            }],
        };
        let mut scene = GraphicsScene::empty();
        application_text(
            &mut scene,
            LayoutRect {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            },
            100,
            &view,
        )
        .unwrap();
        assert!(
            scene
                .commands()
                .iter()
                .any(|command| command.payload() == exact)
        );
        assert!(
            scene
                .commands()
                .iter()
                .any(|command| command.payload() == "Inspect next")
        );
    }
}
