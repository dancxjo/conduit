//! Finite contextual projection of canonical Form–Body–Wake–Plan–Play truth.

use crate::{
    gui::{GuiAction, HitTarget, LifecycleContext},
    gui_hit::HitShape,
    gui_primitives::{frame_rect, icon_label, text, PixelRect},
    icon::Icon,
    PatchbayApplication,
};
use embedded_graphics::{
    draw_target::DrawTargetExt,
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point, Size},
    primitives::Rectangle,
};
use patchbay_model::{PatchbayAction, PatchbayMode, PatchbayTheme, WakeLifecycle};

pub const MAX_LIFECYCLE_ACTIONS: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleFlow {
    pub state_code: &'static str,
    pub state_text: String,
    pub detail: String,
    pub exact_basis: String,
    pub actions: Vec<LifecycleFlowAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleFlowAction {
    pub action: PatchbayAction,
    pub label: &'static str,
    pub accelerator: &'static str,
}

impl Default for LifecycleFlow {
    fn default() -> Self {
        Self {
            state_code: "FORM_UNAVAILABLE",
            state_text: "FORM unavailable".into(),
            detail: "Open a checked Form to begin".into(),
            exact_basis: "body=none wake=none plan=none play=none".into(),
            actions: Vec::with_capacity(MAX_LIFECYCLE_ACTIONS),
        }
    }
}

impl PatchbayApplication {
    pub(super) fn lifecycle_flow(&self) -> LifecycleFlow {
        let Some(editor) = &self.form_editor else {
            return LifecycleFlow::default();
        };
        let Ok(document) = self.build_birth.document(editor) else {
            return LifecycleFlow::default();
        };
        if document.revisions.checked_revision != Some(document.revisions.current_revision) {
            return flow(
                "FORM_UNCHECKED",
                "FORM unchecked",
                "Fix the Form before Birth",
                [],
            );
        }
        let retained_failed_wake = document
            .wake
            .as_ref()
            .is_some_and(|wake| wake.lifecycle == WakeLifecycle::Failed);
        let mut result = if retained_failed_wake {
            flow(
                "WAKE_FAILED",
                "WAKE failed",
                "Failure is terminal; Wake creates a new exact identity",
                [action(PatchbayAction::Wake, "WAKE", "F5")],
            )
        } else {
            match document.mode {
                PatchbayMode::Build => flow(
                    "FORM_CHECKED",
                    "FORM checked",
                    "No Body exists",
                    [action(PatchbayAction::Birth, "BIRTH BODY", "F4")],
                ),
                PatchbayMode::BornLulled => flow(
                    "BODY_LULLED",
                    "BODY born · LULLED",
                    "Wake creates a new exact Wake identity",
                    [action(PatchbayAction::Wake, "WAKE", "F5")],
                ),
                PatchbayMode::Awake(lifecycle) => self.awake_flow(lifecycle),
            }
        };
        result.exact_basis = format!(
            "body={} wake={} plan={} play={}",
            document
                .body
                .as_ref()
                .map_or("none", |body| body.body_id.as_str()),
            document
                .wake
                .as_ref()
                .map_or("none", |wake| wake.wake_id.as_str()),
            self.control
                .plan()
                .map_or("none", |plan| plan.plan_id.as_str()),
            document
                .wake
                .as_ref()
                .and_then(|wake| wake.plans.last())
                .and_then(|plan| plan.active_play_id.as_ref())
                .map_or("none", |play| play.as_str()),
        );
        result
    }

    fn awake_flow(&self, lifecycle: WakeLifecycle) -> LifecycleFlow {
        match lifecycle {
            WakeLifecycle::AwaitingPlan => flow(
                "WAKE_AWAITING_PLAN",
                "WAKE awaiting Plan",
                "No Plan has been admitted",
                [
                    action(PatchbayAction::Plan, "PLAN", "F6"),
                    action(PatchbayAction::Lull, "LULL", "Shift+F6"),
                ],
            ),
            WakeLifecycle::AwaitingPlay => flow(
                "PLAN_READY",
                "PLAN ready",
                "Exact Plan admitted; no Play active",
                [
                    action(PatchbayAction::Play, "PLAY", "F7"),
                    action(PatchbayAction::Lull, "LULL", "Shift+F6"),
                ],
            ),
            WakeLifecycle::Playing if self.control.is_running() => flow(
                "PLAY_ACTIVE",
                "PLAY active",
                "Stop requests bounded cancellation",
                [action(PatchbayAction::Stop, "STOP", "Esc")],
            ),
            WakeLifecycle::Playing => self.terminal_flow(),
            WakeLifecycle::Unsatisfied | WakeLifecycle::AwaitingReplacement
                if self.control.is_running() =>
            {
                flow(
                    "PLAY_UNSATISFIED",
                    "PLAY unsatisfied",
                    "Stop the active Play before replacement planning",
                    [action(PatchbayAction::Stop, "STOP", "Esc")],
                )
            }
            WakeLifecycle::Unsatisfied | WakeLifecycle::AwaitingReplacement => flow(
                "PLAY_UNSATISFIED",
                "PLAY unsatisfied",
                "The current Plan cannot continue",
                [
                    action(PatchbayAction::Plan, "REPLAN", "F6"),
                    action(PatchbayAction::Lull, "LULL", "Shift+F6"),
                ],
            ),
            WakeLifecycle::Held => flow(
                "PLAN_HELD",
                "PLAN held",
                "Held Plan has no automatic release policy",
                [action(PatchbayAction::Lull, "LULL", "Shift+F6")],
            ),
            WakeLifecycle::Failed => flow(
                "WAKE_FAILED",
                "WAKE failed",
                "Failure is terminal for this Wake",
                [action(PatchbayAction::Wake, "WAKE", "F5")],
            ),
            WakeLifecycle::Lulled => flow(
                "WAKE_LULLED",
                "WAKE lulled",
                "Wake is terminal",
                [action(PatchbayAction::Wake, "WAKE", "F5")],
            ),
        }
    }

    fn terminal_flow(&self) -> LifecycleFlow {
        if self.control.play_failure().is_some() {
            return flow(
                "PLAY_FAILED",
                "PLAY failed",
                "Platform or application failure retained",
                [action(PatchbayAction::Lull, "LULL", "Shift+F6")],
            );
        }
        match self.control.play_terminal() {
            Some(conduit_core::TerminalDisposition::Completed) => flow(
                "PLAY_COMPLETED",
                "PLAY completed",
                "Completion evidence retained",
                [action(PatchbayAction::Lull, "LULL", "Shift+F6")],
            ),
            Some(conduit_core::TerminalDisposition::Cancelled { .. }) => flow(
                "PLAY_CANCELLED",
                "PLAY cancelled",
                "Cancellation evidence retained",
                [action(PatchbayAction::Lull, "LULL", "Shift+F6")],
            ),
            Some(conduit_core::TerminalDisposition::Failed { .. }) => flow(
                "PLAY_FAILED",
                "PLAY failed",
                "Terminal failure evidence retained",
                [action(PatchbayAction::Lull, "LULL", "Shift+F6")],
            ),
            None => flow(
                "PLAY_TERMINAL_MISSING",
                "PLAY terminal unavailable",
                "No terminal evidence was retained",
                [action(PatchbayAction::Lull, "LULL", "Shift+F6")],
            ),
        }
    }

    pub(super) fn lifecycle_unavailable_reason(&self, requested: PatchbayAction) -> Option<String> {
        let flow = self.lifecycle_flow();
        if requested == PatchbayAction::Hold && flow.state_code == "PLAY_ACTIVE" {
            return None;
        }
        (!flow
            .actions
            .iter()
            .any(|candidate| candidate.action == requested))
        .then(|| {
            format!(
                "{} unavailable while {}: {}",
                action_label(requested),
                flow.state_code,
                flow.detail
            )
        })
    }
}

fn flow<const N: usize>(
    state_code: &'static str,
    state_text: &str,
    detail: &str,
    actions: [LifecycleFlowAction; N],
) -> LifecycleFlow {
    debug_assert!(N <= MAX_LIFECYCLE_ACTIONS);
    LifecycleFlow {
        state_code,
        state_text: state_text.into(),
        detail: detail.into(),
        exact_basis: String::new(),
        actions: actions.into_iter().collect(),
    }
}

const fn action(
    action: PatchbayAction,
    label: &'static str,
    accelerator: &'static str,
) -> LifecycleFlowAction {
    LifecycleFlowAction {
        action,
        label,
        accelerator,
    }
}

pub(super) const fn is_lifecycle_action(action: PatchbayAction) -> bool {
    matches!(
        action,
        PatchbayAction::Birth
            | PatchbayAction::Wake
            | PatchbayAction::Lull
            | PatchbayAction::Plan
            | PatchbayAction::Play
            | PatchbayAction::Stop
            | PatchbayAction::Hold
    )
}

fn action_label(action: PatchbayAction) -> &'static str {
    match action {
        PatchbayAction::Birth => "Birth",
        PatchbayAction::Wake => "Wake",
        PatchbayAction::Lull => "Lull",
        PatchbayAction::Plan => "Plan",
        PatchbayAction::Play => "Play",
        PatchbayAction::Stop => "Stop",
        PatchbayAction::Hold => "Unsatisfied transition",
        _ => "Action",
    }
}

pub(super) fn draw_lifecycle_flow<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    lifecycle: &LifecycleContext,
    width: i32,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let flow = &lifecycle.flow;
    let state_icon = if lifecycle.play_id.is_some() {
        Icon::Play
    } else if lifecycle.plan_id.is_some() {
        Icon::Plan
    } else if lifecycle.wake_id.is_some() {
        Icon::Wake
    } else {
        Icon::Body
    };
    let state_color = match flow.state_code {
        "PLAY_ACTIVE" => theme.success,
        "PLAY_UNSATISFIED" | "WAKE_FAILED" | "FORM_UNAVAILABLE" | "FORM_UNCHECKED" => theme.failure,
        _ => theme.text_secondary,
    };
    let count = i32::try_from(flow.actions.len()).unwrap_or(0);
    let first_x = width.saturating_sub(count * 142 + 12);
    {
        let clip = Rectangle::new(
            Point::new(242, 0),
            Size::new(
                u32::try_from(first_x.saturating_sub(250)).unwrap_or_default(),
                52,
            ),
        );
        let mut state = target.clipped(&clip);
        icon_label(
            &mut state,
            state_icon,
            Point::new(250, 5),
            &flow.state_text,
            state_color,
        );
        text(
            &mut state,
            Point::new(250, 28),
            &format!(
                "{} · {} · {}",
                flow.state_code, flow.detail, flow.exact_basis
            ),
            state_color,
        );
    }
    for (index, candidate) in flow.actions.iter().enumerate() {
        let bounds = PixelRect {
            x: first_x + index as i32 * 142,
            y: 7,
            width: 132,
            height: 36,
        };
        frame_rect(target, bounds, theme.focus, 2);
        text(
            target,
            Point::new(bounds.x + 8, bounds.y + 7),
            candidate.label,
            theme.focus,
        );
        text(
            target,
            Point::new(bounds.x + 8, bounds.y + 23),
            candidate.accelerator,
            theme.text_secondary,
        );
        targets.push(HitTarget {
            action: GuiAction::Lifecycle(candidate.action),
            shape: HitShape::Rect(bounds),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_bound_is_explicit() {
        assert_eq!(MAX_LIFECYCLE_ACTIONS, 2);
        assert!(
            flow(
                "TEST",
                "test",
                "test",
                [action(PatchbayAction::Plan, "PLAN", "F6")]
            )
            .actions
            .len()
                <= MAX_LIFECYCLE_ACTIONS
        );
    }
}
