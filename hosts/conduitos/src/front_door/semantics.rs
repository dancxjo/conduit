//! Portable action and ordinary lifecycle disclosure for the ConduitOS Presenter.

use alloc::{borrow::ToOwned, format, vec, vec::Vec};
use conduit_presentation::{
    PresentationAction, PresentationActionAvailability, PresentationDisclosureLevel,
};

use crate::product_journey::{JourneyProjection, JourneyStatus};

use super::FrontDoor;

impl FrontDoor {
    pub(super) fn semantic_actions(&self, seed_subject: &str) -> Vec<PresentationAction> {
        let status = self
            .journey
            .as_ref()
            .map_or(JourneyStatus::World, |journey| journey.status);
        let lifecycle_target = self
            .journey
            .as_ref()
            .and_then(|journey| journey.body_id.as_ref())
            .map_or_else(
                || seed_subject.to_owned(),
                |body| format!("body/{}", body.as_str()),
            );
        let birth = if !self.lifecycle_authority_admitted {
            unavailable(
                "authority/not-admitted",
                "No admitted authority can create a Body from this entrance.",
            )
        } else if status == JourneyStatus::SeedOpened {
            PresentationActionAvailability::Available
        } else {
            lifecycle_unavailable("Birth", status)
        };
        let mut rows = vec![
            action(
                "open-back",
                "Open",
                seed_subject,
                availability(
                    matches!(status, JourneyStatus::World | JourneyStatus::SeedOpened),
                    "Open",
                    status,
                ),
            ),
            action("birth", "Birth", seed_subject, birth),
        ];
        if self
            .journey
            .as_ref()
            .is_some_and(|journey| journey.body_id.is_some())
        {
            rows.extend([
                action(
                    "wake",
                    "Wake",
                    &lifecycle_target,
                    availability(
                        matches!(status, JourneyStatus::BornLulled | JourneyStatus::Lulled),
                        "Wake",
                        status,
                    ),
                ),
                action(
                    "plan",
                    "Plan",
                    &lifecycle_target,
                    availability(status == JourneyStatus::Awake, "Plan", status),
                ),
                action(
                    "play",
                    "Play",
                    &lifecycle_target,
                    availability(status == JourneyStatus::Planned, "Play", status),
                ),
                action(
                    "stop",
                    "Stop",
                    &lifecycle_target,
                    availability(
                        matches!(
                            status,
                            JourneyStatus::Playing | JourneyStatus::ResultVisible
                        ),
                        "Stop",
                        status,
                    ),
                ),
                action(
                    "lull",
                    "Lull",
                    &lifecycle_target,
                    availability(
                        matches!(
                            status,
                            JourneyStatus::ResultVisible | JourneyStatus::Stopped
                        ),
                        "Lull",
                        status,
                    ),
                ),
            ]);
        }
        rows
    }
}

fn action(
    name: &str,
    label: &str,
    target: &str,
    availability: PresentationActionAvailability,
) -> PresentationAction {
    let semantic = conduit_core::PatchbayAction::from_name(name)
        .expect("the finite ConduitOS action table contains known portable actions");
    PresentationAction {
        identity: format!("action/{name}/{target}"),
        intent: semantic.presentation_intent().into(),
        target: target.into(),
        label: label.into(),
        disclosure: PresentationDisclosureLevel::CurrentAction,
        availability,
    }
}

fn availability(
    current: bool,
    label: &str,
    status: JourneyStatus,
) -> PresentationActionAvailability {
    if current {
        PresentationActionAvailability::Available
    } else {
        lifecycle_unavailable(label, status)
    }
}

fn lifecycle_unavailable(label: &str, status: JourneyStatus) -> PresentationActionAvailability {
    unavailable(
        "lifecycle/not-current",
        &format!(
            "{label} is not available while lifecycle state is {}.",
            status.as_str()
        ),
    )
}

fn unavailable(reason_code: &str, explanation: &str) -> PresentationActionAvailability {
    PresentationActionAvailability::Unavailable {
        reason_code: reason_code.into(),
        explanation: explanation.into(),
    }
}

pub(super) fn lifecycle_summary(journey: &JourneyProjection) -> &'static str {
    match journey.status {
        JourneyStatus::World => "Body none; the entrance Seed is ready for inert inspection.",
        JourneyStatus::SeedOpened => "Seed open; inspection has created no effect.",
        JourneyStatus::BornLulled => "Body born and retained; Wake is available.",
        JourneyStatus::Awake => "Wake active; an exact Plan is required before Play.",
        JourneyStatus::Planned => "Exact immutable Plan ready; Play is available.",
        JourneyStatus::Playing => "Play active; the production kernel awaits keyboard input.",
        JourneyStatus::ResultVisible => "Play result visible; Stop or Lull is available.",
        JourneyStatus::Stopped => "Play stopped; no late value was accepted.",
        JourneyStatus::Lulled => "Body retained; the prior Wake has ended.",
    }
}
