//! Renderer-local projection of exact lifecycle truth into contextual actions.

use patchbay_model::{PatchbayAction, WakeLifecycle};

use crate::PatchbayApplication;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleActionView {
    pub action: PatchbayAction,
    pub label: &'static str,
    pub accelerator: &'static str,
    pub enabled: bool,
    pub explanation: String,
}

impl PatchbayApplication {
    pub(super) fn lifecycle_actions(&self) -> Vec<LifecycleActionView> {
        let body = self.build_birth.body();
        let wake = self.build_birth.wake_value();
        let running = self.control.is_running();
        let planned = self.control.plan().is_some();
        let lifecycle = wake.map(|wake| wake.lifecycle);
        let current_form_checked = self.form_editor.as_ref().is_some_and(|editor| {
            self.build_birth.document(editor).is_ok_and(|document| {
                document.revisions.checked_revision == Some(document.revisions.current_revision)
            })
        });

        let candidates = [
            (PatchbayAction::Birth, "BIRTH BODY", "F4"),
            (PatchbayAction::Wake, "WAKE BODY", "F5"),
            (PatchbayAction::Plan, "PLAN PLAY", "F6"),
            (PatchbayAction::Play, "PLAY PLAN", "F7"),
            (PatchbayAction::Stop, "STOP PLAY", "ESC"),
            (PatchbayAction::Hold, "MARK UNSATISFIED", "F8"),
            (PatchbayAction::Lull, "LULL BODY", "F9"),
        ];
        candidates
            .into_iter()
            .map(|(action, label, accelerator)| {
                let explanation = availability_reason(
                    action,
                    body.is_some(),
                    lifecycle,
                    planned,
                    running,
                    current_form_checked,
                );
                LifecycleActionView {
                    action,
                    label,
                    accelerator,
                    enabled: explanation.is_none(),
                    explanation: explanation.unwrap_or_else(|| "available now".into()),
                }
            })
            .collect()
    }

    pub(super) fn lifecycle_action_unavailable(&self, action: PatchbayAction) -> Option<String> {
        self.lifecycle_actions()
            .into_iter()
            .find(|candidate| candidate.action == action && !candidate.enabled)
            .map(|candidate| format!("{} unavailable: {}", candidate.label, candidate.explanation))
    }
}

fn availability_reason(
    action: PatchbayAction,
    born: bool,
    lifecycle: Option<WakeLifecycle>,
    planned: bool,
    running: bool,
    current_form_checked: bool,
) -> Option<String> {
    use PatchbayAction::{Birth, Hold, Lull, Plan, Play, Stop, Wake};
    match action {
        Birth if born => Some("a Body is already born".into()),
        Birth if !current_form_checked => Some("the current Form is not checked".into()),
        Birth => None,
        Wake if !born => Some("Birth the Body first".into()),
        Wake if lifecycle.is_some_and(|state| {
            !matches!(state, WakeLifecycle::Lulled | WakeLifecycle::Failed)
        }) =>
        {
            Some("the Body is already awake".into())
        }
        Wake => None,
        Plan if !matches!(
            lifecycle,
            Some(
                WakeLifecycle::AwaitingPlan
                    | WakeLifecycle::AwaitingReplacement
                    | WakeLifecycle::Unsatisfied
            )
        ) =>
        {
            Some("Wake the Body or finish the current lifecycle transition first".into())
        }
        Plan if !current_form_checked => Some("the current Form is not checked".into()),
        Plan => None,
        Play if running => Some("a Play is already active".into()),
        Play if !planned => Some("Plan a Play first".into()),
        Play if !matches!(
            lifecycle,
            Some(WakeLifecycle::AwaitingPlay | WakeLifecycle::Held)
        ) =>
        {
            Some("the Wake is not awaiting this Plan".into())
        }
        Play => None,
        Stop if !running => Some("no Play is active".into()),
        Stop => None,
        Hold if !running => Some("no active Play can become unsatisfied".into()),
        Hold => None,
        Lull if !born => Some("Birth the Body first".into()),
        Lull if lifecycle.is_none() => Some("Wake the Body first".into()),
        Lull if running => Some("Stop the active Play first".into()),
        Lull if matches!(
            lifecycle,
            Some(WakeLifecycle::Lulled | WakeLifecycle::Failed)
        ) =>
        {
            Some("the Wake is already terminal".into())
        }
        Lull => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{arguments::Arguments, interaction_status::InteractionStatusLevel};

    #[test]
    fn availability_preserves_exact_lifecycle_preconditions() {
        assert_eq!(
            availability_reason(PatchbayAction::Birth, false, None, false, false, true),
            None
        );
        assert!(
            availability_reason(PatchbayAction::Wake, false, None, false, false, true).is_some()
        );
        assert_eq!(
            availability_reason(
                PatchbayAction::Plan,
                true,
                Some(WakeLifecycle::AwaitingPlan),
                false,
                false,
                true
            ),
            None
        );
        assert_eq!(
            availability_reason(
                PatchbayAction::Stop,
                true,
                Some(WakeLifecycle::Playing),
                true,
                true,
                true
            ),
            None
        );
        assert!(availability_reason(
            PatchbayAction::Lull,
            true,
            Some(WakeLifecycle::Playing),
            true,
            true,
            true
        )
        .is_some());
    }

    #[test]
    fn exact_state_and_disabled_accelerator_remain_nonfatal() {
        let directory = std::env::temp_dir().join(format!(
            "patchbay-contextual-lifecycle-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("hello.conduit");
        std::fs::write(&path, include_str!("../../../examples/hello.conduit")).unwrap();
        let mut application = PatchbayApplication::new(Arguments {
            form_path: Some(path.clone()),
            ..Arguments::default()
        })
        .unwrap();

        let actions = application.lifecycle_actions();
        assert!(actions
            .iter()
            .any(|candidate| { candidate.action == PatchbayAction::Birth && candidate.enabled }));
        assert!(actions
            .iter()
            .any(|candidate| { candidate.action == PatchbayAction::Wake && !candidate.enabled }));
        application
            .dispatch_invocation(PatchbayAction::Wake)
            .unwrap();
        let refusal = application.interaction_status.current().unwrap();
        assert_eq!(refusal.level, InteractionStatusLevel::Refusal);
        assert!(refusal.text.contains("Birth the Body first"));
        assert!(application.build_birth.body().is_none());

        application
            .dispatch_invocation(PatchbayAction::Birth)
            .unwrap();
        let actions = application.lifecycle_actions();
        assert!(actions
            .iter()
            .any(|candidate| { candidate.action == PatchbayAction::Wake && candidate.enabled }));
        assert!(actions
            .iter()
            .any(|candidate| { candidate.action == PatchbayAction::Birth && !candidate.enabled }));

        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
