//! Root layout consumes only the current admitted read-only Home projection.
use super::*;
use conduit_face::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, GraphicsTextRole,
    LayoutRect,
};
impl FrontDoor {
    pub(in crate::front_door) fn home_scene(
        &self,
        display: &impl crate::display::PixelTarget,
    ) -> Result<GraphicsScene, Error> {
        let home = self.home.as_ref().ok_or(Error::Scene)?;
        if !home.is_live() {
            return Err(Error::Scene);
        }
        let journey = self.journey.as_ref().ok_or(Error::Scene)?;
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
            journey.friendly_name.as_deref().unwrap_or("Current body"),
            GraphicsTextRole::Status,
            GraphicsPaintRole::Muted,
        )?;
        match home.model().view() {
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
                                if index == home.model().selected_index() {
                                    GraphicsPaintRole::Accent
                                } else {
                                    GraphicsPaintRole::Foreground
                                },
                                GraphicsShapeStyle::Stroke,
                            )
                            .map_err(|_| Error::Scene)?,
                        )
                        .map_err(|_| Error::Scene)?;
                    draw_text(
                        &mut scene,
                        screen,
                        bounds.x + 22,
                        bounds.y + 48,
                        item.label(),
                        GraphicsTextRole::Heading,
                        if index == home.model().selected_index() {
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
                    &format!("conduit> {}_", home.model().command()),
                    GraphicsTextRole::Code,
                    GraphicsPaintRole::Foreground,
                )?;
                draw_text(
                    &mut scene,
                    screen,
                    56,
                    282,
                    home.model().output(),
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
                for (index, plot) in self.home_installed_plots().iter().enumerate() {
                    draw_text(
                        &mut scene,
                        screen,
                        72,
                        188 + index as i16 * 48,
                        &format!(
                            "{}  {}",
                            if index == home.model().selected_plot_index() {
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
                        journey.friendly_name.as_deref().unwrap_or("Current body")
                    ),
                    format!(
                        "Wake       {}",
                        journey.wake_id.as_ref().map_or("-", |id| id.as_str())
                    ),
                    format!(
                        "Plan       {}",
                        journey.plan_id.as_ref().map_or("idle", |id| id.as_str())
                    ),
                    format!(
                        "Play       {}",
                        journey
                            .active_play_id
                            .as_ref()
                            .map_or("-", |id| id.as_str())
                    ),
                    format!("Lines      {}", usize::from(self.connectivity.is_some())),
                    format!("Host       {}", journey.host_id.as_str()),
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
        if let Some(reason) = home.refusal() {
            draw_text(
                &mut scene,
                screen,
                34,
                i16::try_from(screen.height.saturating_sub(104)).map_err(|_| Error::Scene)?,
                reason,
                GraphicsTextRole::Status,
                GraphicsPaintRole::Status,
            )?;
        }
        let status_y = i16::try_from(screen.height.saturating_sub(52)).map_err(|_| Error::Scene)?;
        if let Some(refusal) = &self.refusal {
            draw_text(
                &mut scene,
                screen,
                24,
                status_y - 38,
                &format!("{} {}", refusal.heading(), refusal.reason()),
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
                .map_err(|_| Error::Scene)?,
            )
            .map_err(|_| Error::Scene)?;
        draw_text(
            &mut scene,
            screen,
            24,
            status_y,
            &format!(
                "BODY {}  |  WAKE {}  |  PLAN {}  |  PLAY {}  |  LINES {}  |  HOST {}",
                journey.friendly_name.as_deref().unwrap_or("CURRENT"),
                if journey.wake_id.is_some() { "1" } else { "-" },
                if journey.plan_id.is_some() {
                    "current"
                } else {
                    "idle"
                },
                if journey.active_play_id.is_some() {
                    "current"
                } else {
                    "-"
                },
                usize::from(self.connectivity.is_some()),
                journey.host_id.as_str()
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
) -> Result<(), Error> {
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
            .map_err(|_| Error::Scene)?,
        )
        .map_err(|_| Error::Scene)
}
