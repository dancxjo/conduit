//! Renderer-neutral guidance projected only from authoritative Body truth.
use crate::WorkspaceBody;
use alloc::{format, vec, vec::Vec};
use conduit_body::{BodyState, WakeLifecycle};
use conduit_presentation::{
    ActionAvailability, ApplicationEventKind, PresentationMechanism, SemanticAction,
    SemanticApplicationView, SemanticPresentationNode, StatusKind,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TutorialPlayback {
    Lulled,
    Preparing,
    Playing,
    Idle,
    Completed,
    Cancelled,
    Failed,
    Refused,
    Stopped,
    Fulfilled,
}

struct Guidance {
    phase: &'static str,
    title: &'static str,
    detail: &'static str,
    action: &'static str,
    label: &'static str,
}

pub fn presentation(
    body: &WorkspaceBody,
    revision: u32,
    playback: TutorialPlayback,
) -> Result<SemanticApplicationView, conduit_presentation::SemanticPresentationRefusal> {
    let guidance = guidance(body, playback);
    let view = SemanticApplicationView {
        revision,
        root: node(
            "body-tutorial",
            PresentationMechanism::Panel {
                title: guidance.title.into(),
            },
            vec![
                node(
                    "tutorial-guidance",
                    PresentationMechanism::Status {
                        kind: if guidance.phase == "repair" {
                            StatusKind::Warning
                        } else {
                            StatusKind::Ordinary
                        },
                        title: format!("Tutorial · {}", guidance.phase),
                        detail: guidance.detail.into(),
                    },
                    vec![],
                ),
                node(
                    "tutorial-next-action",
                    PresentationMechanism::Action(SemanticAction {
                        identity: guidance.action.into(),
                        event: ApplicationEventKind::Activate,
                        label: guidance.label.into(),
                        availability: ActionAvailability::Available,
                    }),
                    vec![],
                ),
            ],
        ),
    };
    view.lower()?;
    Ok(view)
}

fn guidance(body: &WorkspaceBody, playback: TutorialPlayback) -> Guidance {
    let evidence = body.evidence();
    if matches!(evidence.body.state, BodyState::Fulfilled { .. }) {
        return Guidance {
            phase: "fulfilled",
            title: "Its useful life is complete",
            detail: "Fulfilled is terminal, not deletion. Inspect the closed biography.",
            action: "body.inspect-lifecycle",
            label: "Inspect lifecycle evidence",
        };
    }
    if evidence
        .wakes
        .iter()
        .any(|wake| wake.lifecycle == WakeLifecycle::Failed)
    {
        return Guidance {
            phase: "repair",
            title: "Inspect the real fault",
            detail: "This biography contains a failed Wake. Inspect its evidence, then change the actual workset or available Hosts before waking again.",
            action: "body.inspect-lifecycle",
            label: "Inspect lifecycle evidence",
        };
    }
    if matches!(evidence.body.state, BodyState::Lulled) && evidence.wakes.is_empty() {
        return Guidance {
            phase: "wake",
            title: "Wake this Body",
            detail: "Birth made one retained Body. Wake admits its first exact Plan and Play without creating another Body.",
            action: "body.wake",
            label: "Wake the retained Body",
        };
    }
    if matches!(evidence.body.state, BodyState::Lulled) {
        return Guidance {
            phase: "lull",
            title: "Retained rest is not completion",
            detail: "The Body is lulled: its identity, Forms, and biography remain.",
            action: "body.wake",
            label: "Wake the retained Body",
        };
    }
    if evidence.body.workload_revision > 0 {
        return Guidance {
            phase: "revised",
            title: "One Body, a changed workset",
            detail: "The workload revision changed without rebirth. The current Plan realizes revised Forms for this same Body.",
            action: "body.inspect-lifecycle",
            label: "Inspect the current realization",
        };
    }
    if evidence.wakes.len() > 1 {
        return Guidance {
            phase: "continuity",
            title: "The same Body woke again",
            detail: "A fresh Wake, Plan, and Play continue one retained biography.",
            action: "body.open-library",
            label: "Browse this Body's Forms",
        };
    }
    Guidance {
        phase: "living",
        title: "Use it, then leave it useful",
        detail: if playback == TutorialPlayback::Idle {
            "Idle means admitted work awaits future input. Finite means bounded state, queues, authority, and obligations—not a short lifetime."
        } else {
            "Interact more than once. A finite Body may remain awake indefinitely because its instantaneous and retained bounds stay finite."
        },
        action: "body.use-current",
        label: "Use the current Form",
    }
}

fn node(
    key: &str,
    mechanism: PresentationMechanism,
    children: Vec<SemanticPresentationNode>,
) -> SemanticPresentationNode {
    SemanticPresentationNode {
        key: key.into(),
        mechanism,
        children,
    }
}
