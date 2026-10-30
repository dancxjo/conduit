//! Local presentation controls; this never emits an action to the Body.
use crate::touch_input::Contact;
use conduit_face::{
    GraphicsCommand, GraphicsPaintRole as Paint, GraphicsScene, GraphicsShapeStyle as Style,
    LayoutRect,
    presentation_preferences::{
        PresentationAction as Action, PresentationPreferences as Preferences,
    },
};
pub const HEIGHT: u16 = 56;
const ACTIONS: [Action; 5] = [
    Action::NextTheme,
    Action::ToggleTemperatureUnit,
    Action::ZoomOut,
    Action::ZoomIn,
    Action::ResetHistoryWindow,
];
#[derive(Default)]
pub struct Gestures {
    down: Option<(u16, u16)>,
    pinching: bool,
    distance: Option<u64>,
    dragging: bool,
    moved: bool,
}
fn distance(a: Contact, b: Contact) -> u64 {
    let x = i64::from(a.x) - i64::from(b.x);
    let y = i64::from(a.y) - i64::from(b.y);
    (x * x + y * y) as u64
}
impl Gestures {
    #[cfg(test)]
    pub fn frame(
        &mut self,
        contacts: [Contact; 5],
        width: u16,
        height: u16,
        graph_left: u16,
    ) -> Option<Action> {
        self.frame_with_divider(
            contacts,
            width,
            height,
            graph_left,
            Some(graph_left.saturating_sub(6)),
        )
    }
    pub fn frame_with_divider(
        &mut self,
        contacts: [Contact; 5],
        width: u16,
        height: u16,
        graph_left: u16,
        divider: Option<u16>,
    ) -> Option<Action> {
        let mut active = contacts.into_iter().filter(|p| p.active);
        let first = active.next();
        let second = active.next();
        if let (Some(a), Some(b)) = (first, second) {
            self.down = None;
            self.dragging = false;
            self.pinching = true;
            if a.y >= height.saturating_sub(HEIGHT)
                || b.y >= height.saturating_sub(HEIGHT)
                || a.x < graph_left
                || b.x < graph_left
            {
                self.distance = None;
                return None;
            }
            let current = distance(a, b);
            let previous = self.distance.replace(current)?;
            if previous < 400 {
                return None;
            }
            // 25% distance hysteresis; square comparisons require no floating point.
            if current.saturating_mul(16) > previous.saturating_mul(25) {
                return Some(Action::ZoomIn);
            }
            if current.saturating_mul(25) < previous.saturating_mul(16) {
                return Some(Action::ZoomOut);
            }
            self.distance = Some(previous);
            return None;
        }
        if let Some(point) = first {
            if !self.pinching {
                if self.down.is_none() {
                    self.down = Some((point.x, point.y));
                    self.moved = false;
                    self.dragging = point.y < height.saturating_sub(HEIGHT)
                        && divider.is_some_and(|x| x.abs_diff(point.x) <= 20);
                }
                let (x, y) = self.down.unwrap();
                self.moved |= x.abs_diff(point.x) > 12 || y.abs_diff(point.y) > 12;
                if self.dragging && self.moved {
                    return Some(Action::MoveDivider(
                        (u32::from(point.x) * 1000 / u32::from(width.max(1))) as u16,
                    ));
                }
            }
            return None;
        }
        if self.dragging {
            self.dragging = false;
            self.down = None;
            return None;
        }
        self.distance = None;
        if self.pinching {
            self.pinching = false;
            self.down = None;
            return None;
        }
        let (x, y) = self.down.take()?;
        if self.moved || y >= height || y < height.saturating_sub(HEIGHT) {
            return None;
        }
        ACTIONS.iter().enumerate().find_map(|(index, action)| {
            (u32::from(x) < u32::from(width) * (index as u32 + 1) / 5).then_some(*action)
        })
    }
}
pub fn render(
    width: u16,
    height: u16,
    pixels: &mut [u32],
    preferences: Preferences,
    look: &conduit_face::LookInfo,
) -> Result<(), String> {
    if width < 240 || height < HEIGHT || pixels.len() != usize::from(width) * usize::from(height) {
        return Err("controls viewport".into());
    }
    let clip = LayoutRect {
        x: 0,
        y: 0,
        width,
        height: HEIGHT,
    };
    let mut scene = GraphicsScene::empty();
    let labels = [
        format!("Theme: {}", preferences.theme.label()),
        format!("Units: {}", preferences.temperature.label()),
        "Zoom out -".into(),
        "Zoom in +".into(),
        format!("Range: {}", preferences.history_window.label()),
    ];
    for (index, label) in labels.iter().enumerate() {
        let left = u32::from(width) * index as u32 / 5;
        let right = u32::from(width) * (index + 1) as u32 / 5;
        let bounds = LayoutRect {
            x: left as i16,
            y: 0,
            width: (right - left) as u16,
            height: HEIGHT,
        };
        scene
            .push(
                GraphicsCommand::rect(bounds, clip, Paint::Surface, Style::Fill)
                    .map_err(|e| format!("controls: {e:?}"))?,
            )
            .map_err(|e| format!("controls: {e:?}"))?;
        scene
            .push(
                GraphicsCommand::rect(bounds, clip, Paint::Border, Style::Stroke)
                    .map_err(|e| format!("controls: {e:?}"))?,
            )
            .map_err(|e| format!("controls: {e:?}"))?;
        let bounds = LayoutRect {
            x: bounds.x + 8,
            y: 15,
            width: bounds.width.saturating_sub(16),
            height: 32,
        };
        scene
            .push(
                GraphicsCommand::text(bounds, clip, Paint::Foreground, label)
                    .map_err(|e| format!("controls: {e:?}"))?,
            )
            .map_err(|e| format!("controls: {e:?}"))?;
    }
    let start = usize::from(height - HEIGHT) * usize::from(width);
    crate::raster_target::panel(&scene, width, HEIGHT, &mut pixels[start..], look)
}
pub fn divider_position(preferences: Preferences, width: u16) -> u16 {
    use conduit_face::presentation_preferences::SplitLayout;
    match preferences.split {
        SplitLayout::CollapsedLeft => 0,
        SplitLayout::CollapsedRight => width.saturating_sub(1),
        SplitLayout::Split { left_per_mille } => {
            ((u32::from(width) * u32::from(left_per_mille.min(1000)) / 1000) as u16)
                .clamp(320, width.saturating_sub(332).max(320))
        }
    }
}
pub fn render_divider(
    width: u16,
    height: u16,
    pixels: &mut [u32],
    preferences: Preferences,
    look: &conduit_face::LookInfo,
) -> Result<(), String> {
    if width < 652 {
        return Ok(());
    }
    let x = divider_position(preferences, width)
        .saturating_sub(5)
        .min(width - 12);
    let clip = LayoutRect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut scene = GraphicsScene::empty();
    let bounds = LayoutRect {
        x: x as i16,
        y: 0,
        width: 12,
        height: height.saturating_sub(HEIGHT),
    };
    scene
        .push(
            GraphicsCommand::rect(bounds, clip, Paint::Border, Style::Fill)
                .map_err(|e| format!("divider: {e:?}"))?,
        )
        .map_err(|e| format!("divider: {e:?}"))?;
    let grip = LayoutRect {
        x: x as i16 + 4,
        y: (height.saturating_sub(HEIGHT) / 2).saturating_sub(25) as i16,
        width: 4,
        height: 50,
    };
    scene
        .push(
            GraphicsCommand::rect(grip, clip, Paint::Focus, Style::Fill)
                .map_err(|e| format!("divider: {e:?}"))?,
        )
        .map_err(|e| format!("divider: {e:?}"))?;
    crate::raster_target::panel(&scene, width, height, pixels, look)
}

pub fn chrome(
    width: u16,
    height: u16,
    pixels: &mut [u32],
    preferences: Preferences,
    look: &conduit_face::LookInfo,
) -> Result<(), String> {
    let interaction = crate::display_clock::content_height(height)?;
    let view = &mut pixels[..usize::from(width) * usize::from(interaction)];
    render_divider(width, interaction, view, preferences, look)?;
    render(width, interaction, view, preferences, look)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn contact(x: u16, y: u16) -> Contact {
        Contact { active: true, x, y }
    }
    #[test]
    fn divider_drag_collapses_and_edge_drag_restores_without_a_tap() {
        let mut g = Gestures::default();
        let mut c = [Contact::default(); 5];
        c[0] = contact(368, 250);
        assert_eq!(g.frame_with_divider(c, 1024, 568, 380, Some(368)), None);
        c[0].x = 0;
        assert_eq!(
            g.frame_with_divider(c, 1024, 568, 380, Some(368)),
            Some(Action::MoveDivider(0))
        );
        assert_eq!(
            g.frame_with_divider([Contact::default(); 5], 1024, 568, 0, Some(0)),
            None
        );
        c[0] = contact(0, 250);
        g.frame_with_divider(c, 1024, 568, 0, Some(0));
        c[0].x = 500;
        assert_eq!(
            g.frame_with_divider(c, 1024, 568, 0, Some(0)),
            Some(Action::MoveDivider(488))
        );
    }
    #[test]
    fn pinch_never_turns_into_a_button_tap() {
        let mut g = Gestures::default();
        let mut c = [Contact::default(); 5];
        c[0] = contact(700, 300);
        c[1] = contact(800, 300);
        assert_eq!(g.frame(c, 1024, 568, 372), None);
        c[1].x = 850;
        assert_eq!(g.frame(c, 1024, 568, 372), Some(Action::ZoomIn));
        c[1].active = false;
        c[0].y = 540;
        assert_eq!(g.frame(c, 1024, 568, 372), None);
        assert_eq!(g.frame([Contact::default(); 5], 1024, 568, 372), None);
    }
    #[test]
    fn releasing_in_body_content_does_not_invoke_a_control() {
        let mut g = Gestures::default();
        let mut c = [Contact::default(); 5];
        c[0] = contact(10, 100);
        g.frame(c, 1024, 568, 372);
        assert_eq!(g.frame([Contact::default(); 5], 1024, 568, 372), None);
        c[0].y = 540;
        g.frame(c, 1024, 568, 372);
        assert_eq!(
            g.frame([Contact::default(); 5], 1024, 568, 372),
            Some(Action::NextTheme)
        );
    }
}
