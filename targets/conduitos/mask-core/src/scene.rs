//! Canonical specialized workspace layout, shared by the guest and host reference.
use crate::{
    data::{PresentationStatus, Refusal},
    layout::{self as application_layout, ApplicationViewport, PAGE_ROWS},
};
const SPACE_LG: u16 = 24;
const SPACE_SM: u16 = 8;
const SPACE_XL: u16 = 32;
const TEXT_STEP: i16 = (SPACE_XL + SPACE_SM) as i16;
const TEXT_BOX_HEIGHT: u16 = SPACE_XL + SPACE_SM * 2;
use alloc::format;
use conduit_face::{
    ApplicationComponent, ApplicationView, GraphicsCommand, GraphicsPaintRole, GraphicsScene,
    GraphicsShapeStyle, GraphicsTextRole, LayoutRect,
};

pub fn scene(
    journey: &crate::data::Facts,
    foreground_title: &str,
    application_view: Option<&ApplicationView>,
    viewport: ApplicationViewport,
    patchbay_graph_available: bool,
    refusal: Option<(&str, &str)>,
    width: u16,
    height: u16,
) -> Result<GraphicsScene, Refusal> {
    let screen = LayoutRect {
        x: 0,
        y: 0,
        width,
        height,
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
            .map_err(|_| Refusal::Layout)?,
        )
        .map_err(|_| Refusal::Layout)?;
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
        PresentationStatus::BornLulled => "Body born. Its installed plots are not executing yet.",
        PresentationStatus::Awake => "Body awake. Planning its installed plots...",
        PresentationStatus::Planned => "Installed plots admitted. Starting their play...",
        PresentationStatus::QuiescentAwaitingInput => "Quiescent. Type to continue this play.",
        PresentationStatus::SemanticCompleted => {
            "Play semantically completed. Your result is below."
        }
        PresentationStatus::InputUnavailable => journey
            .loss_recovery
            .as_deref()
            .unwrap_or("Input unavailable. Inspect details before recovery."),
        PresentationStatus::Stopped => "Play stopped.",
        PresentationStatus::Lulled => "Body lulled. Its installed plots remain included.",
        PresentationStatus::Fulfilled => {
            "Body fulfilled. Its closed biography remains inspectable."
        }
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
    if let Some((heading, refusal)) = refusal {
        let notice_y = result_notice_y
            .checked_add(TEXT_BOX_HEIGHT as i16)
            .ok_or(Refusal::Layout)?;
        text(
            &mut scene,
            screen,
            notice_y,
            heading,
            GraphicsPaintRole::Status,
            GraphicsTextRole::Warning,
        )?;
        text(
            &mut scene,
            screen,
            notice_y.checked_add(TEXT_STEP).ok_or(Refusal::Layout)?,
            refusal,
            GraphicsPaintRole::Status,
            GraphicsTextRole::Code,
        )?;
    }
    let footer_y = i16::try_from(screen.height.saturating_sub(TEXT_BOX_HEIGHT))
        .map_err(|_| Refusal::Layout)?;
    let lifecycle_action = match journey.status {
        PresentationStatus::QuiescentAwaitingInput => "  ·  F8 Stop  ·  F7 Lull",
        PresentationStatus::InputUnavailable
        | PresentationStatus::Stopped
        | PresentationStatus::SemanticCompleted => "  ·  F7 Lull",
        PresentationStatus::Lulled => "  ·  End Fulfill",
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
) -> Result<i16, Refusal> {
    view.validate().map_err(|_| Refusal::Layout)?;
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
                            .map_err(|_| Refusal::Layout)?,
                    )
                    .map_err(|_| Refusal::Layout)?;
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
) -> Result<i16, Refusal> {
    if value.is_empty() {
        text(
            scene,
            screen,
            y,
            "Result is empty.",
            GraphicsPaintRole::Muted,
            GraphicsTextRole::Status,
        )?;
        return y.checked_add(TEXT_STEP).ok_or(Refusal::Layout);
    }
    let mut remaining = value;
    while !remaining.is_empty() {
        let mut split = remaining.len().min(conduit_face::MAX_GRAPHICS_TEXT_BYTES);
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
        y = y.checked_add(TEXT_STEP).ok_or(Refusal::Layout)?;
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
) -> Result<(), Refusal> {
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
                .map_err(|_| Refusal::Layout)?,
        )
        .map_err(|_| Refusal::Layout)
}
