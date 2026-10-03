//! Quiet foreground surface for the born Body, projected from its current journey.
use super::{
    Error,
    application_layout::{self, ApplicationViewport, PAGE_ROWS},
};
use crate::{
    display::{PixelTarget, SPACE_LG, SPACE_SM, SPACE_XL},
    product_journey::{JourneyProjection, JourneyStatus},
};

const TEXT_STEP: i16 = (SPACE_XL + SPACE_SM) as i16;
const TEXT_BOX_HEIGHT: u16 = SPACE_XL + SPACE_SM * 2;
use alloc::format;
use conduit_presentation::{
    ApplicationComponent, ApplicationView, GraphicsCommand, GraphicsPaintRole, GraphicsScene,
    GraphicsShapeStyle, GraphicsTextRole, LayoutRect,
};

pub(super) fn scene(
    journey: &JourneyProjection,
    foreground_title: &str,
    application_view: Option<&ApplicationView>,
    viewport: ApplicationViewport,
    patchbay_graph_available: bool,
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
        JourneyStatus::BornLulled => "Body born. Its installed plots are not executing yet.",
        JourneyStatus::Awake => "Body awake. Planning its installed plots...",
        JourneyStatus::Planned => "Installed plots admitted. Starting their play...",
        JourneyStatus::QuiescentAwaitingInput => "Quiescent. Type to continue this play.",
        JourneyStatus::SemanticCompleted => "Play semantically completed. Your result is below.",
        JourneyStatus::InputUnavailable => journey
            .loss_kind
            .map(|kind| kind.recovery())
            .unwrap_or("Input unavailable. Inspect details before recovery."),
        JourneyStatus::Stopped => "Play stopped.",
        JourneyStatus::Lulled => "Body lulled. Its installed plots remain included.",
        JourneyStatus::Fulfilled => "Body fulfilled. Its closed biography remains inspectable.",
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
        result_notice_y = application_text(&mut scene, screen, 192, view, viewport)?;
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
        JourneyStatus::Lulled => "  ·  End Fulfill",
        _ => "",
    };
    text(
        &mut scene,
        screen,
        footer_y,
        &if patchbay_graph_available {
            format!(
                "{}  ·  F2 inspect Face  ·  F3 diagram there{}",
                journey.status.as_str(),
                lifecycle_action
            )
        } else {
            format!(
                "{}  ·  F2 details  ·  F9 Tutorial{}",
                journey.status.as_str(),
                lifecycle_action
            )
        },
        GraphicsPaintRole::Foreground,
        GraphicsTextRole::Body,
    )?;
    Ok(scene)
}

fn application_text(
    scene: &mut GraphicsScene,
    screen: LayoutRect,
    y: i16,
    view: &ApplicationView,
    viewport: ApplicationViewport,
) -> Result<i16, Error> {
    view.validate().map_err(|_| Error::Presentation)?;
    let total = application_layout::rows(view, |_, _| {});
    let page = viewport.page.min(total.saturating_sub(1) / PAGE_ROWS);
    let mut rendered = Ok(());
    application_layout::rows(view, |index, row| {
        if index / PAGE_ROWS != page || rendered.is_err() {
            return;
        }
        let row_y = y + (index % PAGE_ROWS) as i16 * TEXT_STEP;
        let selected = viewport.selected_node == Some(row.node_index);
        let (role, typography) = match row.node.component {
            ApplicationComponent::Heading
            | ApplicationComponent::Panel
            | ApplicationComponent::Masthead => {
                (GraphicsPaintRole::Accent, GraphicsTextRole::Heading)
            }
            ApplicationComponent::Status
            | ApplicationComponent::SuccessStatus
            | ApplicationComponent::SuccessfulEvidence => {
                (GraphicsPaintRole::Foreground, GraphicsTextRole::Status)
            }
            ApplicationComponent::FailureStatus
            | ApplicationComponent::WarningStatus
            | ApplicationComponent::FailedEvidence
            | ApplicationComponent::RefusedEvidence => {
                (GraphicsPaintRole::Status, GraphicsTextRole::Warning)
            }
            ApplicationComponent::Definition
            | ApplicationComponent::CodeBlock
            | ApplicationComponent::Code => (GraphicsPaintRole::Foreground, GraphicsTextRole::Code),
            ApplicationComponent::Button => (
                if selected {
                    GraphicsPaintRole::Status
                } else {
                    GraphicsPaintRole::Accent
                },
                GraphicsTextRole::Action,
            ),
            _ => (GraphicsPaintRole::Foreground, GraphicsTextRole::Body),
        };
        rendered = (|| {
            if row.node.component == ApplicationComponent::Button {
                let bounds = LayoutRect {
                    x: SPACE_XL as i16 - 4,
                    y: row_y - 2,
                    width: screen.width.saturating_sub(SPACE_XL * 2) + 8,
                    height: TEXT_STEP as u16 - 4,
                };
                scene
                    .push(
                        GraphicsCommand::rect(bounds, screen, role, GraphicsShapeStyle::Stroke)
                            .map_err(|_| Error::Scene)?,
                    )
                    .map_err(|_| Error::Scene)?;
            }
            text(scene, screen, row_y, row.text, role, typography)
        })();
    });
    rendered?;
    text(
        scene,
        screen,
        y + PAGE_ROWS as i16 * TEXT_STEP,
        &format!(
            "Page {}/{} · PgUp/PgDn read · Up/Down choose · Enter use",
            page + 1,
            total.div_ceil(PAGE_ROWS).max(1)
        ),
        GraphicsPaintRole::Muted,
        GraphicsTextRole::Muted,
    )?;
    Ok(y + PAGE_ROWS as i16 * TEXT_STEP)
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
        let exact = "expanded/plot/exact";
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
                    text: "Plot".into(),
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
            ApplicationViewport::default(),
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
