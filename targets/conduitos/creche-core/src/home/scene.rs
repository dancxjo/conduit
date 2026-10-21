//! Guest Home layout over its canonical model and bounded Root observations.
use super::{Controller, presentation::Request};
use crate::wire::Refusal;
use alloc::format;
use conduit_face::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, GraphicsTextRole,
    LayoutRect,
};
use conduit_home_model::HomeView;
impl Controller {
    pub(crate) fn scene(&self, input: &Request) -> Result<GraphicsScene, Refusal> {
        let extent = input.extent.as_ref().ok_or(Refusal::Bounds)?;
        let screen = LayoutRect {
            x: 0,
            y: 0,
            width: extent.width,
            height: extent.height,
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
                .map_err(|_| Refusal::Projection)?,
            )
            .map_err(|_| Refusal::Projection)?;
        draw_text(
            &mut scene,
            screen,
            34,
            28,
            "CONDUIT",
            GraphicsTextRole::Title,
            GraphicsPaintRole::Accent,
        )?;
        draw_text(
            &mut scene,
            screen,
            34,
            60,
            input.friendly_name.as_deref().unwrap_or("Current body"),
            GraphicsTextRole::Status,
            GraphicsPaintRole::Muted,
        )?;
        match self.model.view() {
            HomeView::Launcher => {
                for (index, item) in conduit_home_model::HomeDestination::ALL.iter().enumerate() {
                    let column = index % 3;
                    let row = index / 3;
                    let bounds = LayoutRect {
                        x: 120 + column as i16 * 300,
                        y: 130 + row as i16 * 190,
                        width: 220,
                        height: 132,
                    };
                    scene
                        .push(
                            GraphicsCommand::rect(
                                bounds,
                                screen,
                                if index == self.model.selected_index() {
                                    GraphicsPaintRole::Accent
                                } else {
                                    GraphicsPaintRole::Foreground
                                },
                                GraphicsShapeStyle::Stroke,
                            )
                            .map_err(|_| Refusal::Projection)?,
                        )
                        .map_err(|_| Refusal::Projection)?;
                    draw_text(
                        &mut scene,
                        screen,
                        bounds.x + 22,
                        bounds.y + 48,
                        item.label(),
                        GraphicsTextRole::Heading,
                        if index == self.model.selected_index() {
                            GraphicsPaintRole::Accent
                        } else {
                            GraphicsPaintRole::Foreground
                        },
                    )?;
                }
                draw_text(
                    &mut scene,
                    screen,
                    34,
                    548,
                    ">  type anywhere for Prompt",
                    GraphicsTextRole::Code,
                    GraphicsPaintRole::Status,
                )?;
            }
            HomeView::Prompt => {
                draw_text(
                    &mut scene,
                    screen,
                    56,
                    142,
                    "CONDUIT PROMPT",
                    GraphicsTextRole::Heading,
                    GraphicsPaintRole::Accent,
                )?;
                draw_text(
                    &mut scene,
                    screen,
                    56,
                    214,
                    &format!("conduit> {}_", self.model.command()),
                    GraphicsTextRole::Code,
                    GraphicsPaintRole::Foreground,
                )?;
                draw_text(
                    &mut scene,
                    screen,
                    56,
                    282,
                    self.model.output(),
                    GraphicsTextRole::Body,
                    GraphicsPaintRole::Status,
                )?;
            }
            HomeView::Plots => {
                draw_text(
                    &mut scene,
                    screen,
                    56,
                    128,
                    "INSTALLED PLOTS",
                    GraphicsTextRole::Heading,
                    GraphicsPaintRole::Accent,
                )?;
                for (index, plot) in input.plots.iter().enumerate() {
                    draw_text(
                        &mut scene,
                        screen,
                        72,
                        188 + index as i16 * 48,
                        &format!(
                            "{}  {}",
                            if index == self.model.selected_plot_index() {
                                ">"
                            } else {
                                " "
                            },
                            plot.title
                        ),
                        GraphicsTextRole::Body,
                        GraphicsPaintRole::Foreground,
                    )?;
                }
            }
            HomeView::Body => {
                draw_text(
                    &mut scene,
                    screen,
                    56,
                    128,
                    "BODY",
                    GraphicsTextRole::Heading,
                    GraphicsPaintRole::Accent,
                )?;
                for (index, line) in [
                    format!(
                        "Name       {}",
                        input.friendly_name.as_deref().unwrap_or("Current body")
                    ),
                    format!(
                        "Wake       {}",
                        input.wake_id.as_ref().map_or("-", |id| id.as_str())
                    ),
                    format!(
                        "Plan       {}",
                        input.plan_id.as_ref().map_or("idle", |id| id.as_str())
                    ),
                    format!(
                        "Play       {}",
                        input.play_id.as_ref().map_or("-", |id| id.as_str())
                    ),
                    format!("Lines      {}", usize::from(input.line_count)),
                    format!("Host       {}", input.scope.host_id.as_str()),
                ]
                .iter()
                .enumerate()
                {
                    draw_text(
                        &mut scene,
                        screen,
                        72,
                        188 + index as i16 * 44,
                        line,
                        GraphicsTextRole::Code,
                        GraphicsPaintRole::Foreground,
                    )?;
                }
            }
        }
        if let Some(reason) = self.refusal.as_deref() {
            draw_text(
                &mut scene,
                screen,
                34,
                i16::try_from(screen.height.saturating_sub(104))
                    .map_err(|_| Refusal::Projection)?,
                reason,
                GraphicsTextRole::Status,
                GraphicsPaintRole::Status,
            )?;
        }
        let status_y =
            i16::try_from(screen.height.saturating_sub(52)).map_err(|_| Refusal::Projection)?;
        if let Some(refusal) = &input.root_refusal {
            draw_text(
                &mut scene,
                screen,
                24,
                status_y - 38,
                refusal,
                GraphicsTextRole::Warning,
                GraphicsPaintRole::Status,
            )?;
        }
        scene
            .push(
                GraphicsCommand::rect(
                    LayoutRect {
                        x: 0,
                        y: status_y - 12,
                        width: screen.width,
                        height: 64,
                    },
                    screen,
                    GraphicsPaintRole::Status,
                    GraphicsShapeStyle::Stroke,
                )
                .map_err(|_| Refusal::Projection)?,
            )
            .map_err(|_| Refusal::Projection)?;
        draw_text(
            &mut scene,
            screen,
            24,
            status_y,
            &format!(
                "BODY {}  |  WAKE {}  |  PLAN {}  |  PLAY {}  |  LINES {}  |  HOST {}",
                input.friendly_name.as_deref().unwrap_or("CURRENT"),
                if input.wake_id.is_some() { "1" } else { "-" },
                if input.plan_id.is_some() {
                    "current"
                } else {
                    "idle"
                },
                if input.play_id.is_some() {
                    "current"
                } else {
                    "-"
                },
                usize::from(input.line_count),
                input.scope.host_id.as_str()
            ),
            GraphicsTextRole::Status,
            GraphicsPaintRole::Status,
        )?;
        Ok(scene)
    }
}

fn draw_text(
    scene: &mut GraphicsScene,
    screen: LayoutRect,
    x: i16,
    y: i16,
    value: &str,
    role: GraphicsTextRole,
    paint: GraphicsPaintRole,
) -> Result<(), Refusal> {
    scene
        .push(
            GraphicsCommand::text(
                LayoutRect {
                    x,
                    y,
                    width: screen.width.saturating_sub(x.max(0) as u16 + 24),
                    height: 34,
                },
                screen,
                paint,
                value,
            )
            .and_then(|command| command.with_text_role(role))
            .map_err(|_| Refusal::Projection)?,
        )
        .map_err(|_| Refusal::Projection)
}
