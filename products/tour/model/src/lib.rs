#![no_std]

extern crate alloc;

use alloc::{format, string::String, vec, vec::Vec};
use conduit_presentation::{
    ActionAvailability, AdmittedNavigationDestination, ApplicationEventKind, PresentationMechanism,
    SemanticAction, SemanticApplicationView, SemanticPresentationNode, SemanticPresentationRefusal,
    StatusKind,
};

mod controller;
mod layout;
mod pointer;
pub use controller::*;
pub use layout::*;
pub use pointer::*;

pub const CANONICAL_SPECIMEN_ID: &str = "canonical-form:meet-one-gear";
pub const CANONICAL_LITERAL: &str = "hello";
pub const CANONICAL_RESULT: &str = "HELLO";
pub const CANONICAL_SOURCE: &str = concat!(
    "form meet-one-gear {\n",
    "    words: text/literal(\"hello\")\n",
    "    change: text/upper\n",
    "    result: presentation/text\n\n",
    "    words > change > result\n",
    "}"
);
pub const RUN_ACTION_ID: &str = "tour.run";
pub const OPEN_PATCHBAY_ACTION_ID: &str = "tour.open-patchbay";
pub const CANONICAL_PATCHBAY_GEARS: [&str; 3] = [
    "meet-one-gear/words",
    "meet-one-gear/change",
    "meet-one-gear/result",
];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourWorkspacePhase {
    LessonReady,
    ResultVisible,
    PatchbayOpen,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TourWorkspaceState {
    pub revision: u32,
    pub phase: TourWorkspacePhase,
    pub focused_key: String,
    pub specimen_id: String,
    pub source: String,
    pub result: Option<String>,
    pub run_pending: bool,
    pub hovered_patchbay_subject: Option<String>,
    pub selected_patchbay_subject: Option<String>,
}

impl TourWorkspaceState {
    pub fn canonical(revision: u32, phase: TourWorkspacePhase) -> Self {
        Self {
            revision,
            phase,
            focused_key: match phase {
                TourWorkspacePhase::LessonReady => "source".into(),
                TourWorkspacePhase::ResultVisible => "result".into(),
                TourWorkspacePhase::PatchbayOpen => "patchbay".into(),
            },
            specimen_id: CANONICAL_SPECIMEN_ID.into(),
            source: CANONICAL_SOURCE.into(),
            result: (phase == TourWorkspacePhase::ResultVisible).then(|| CANONICAL_RESULT.into()),
            run_pending: false,
            hovered_patchbay_subject: None,
            selected_patchbay_subject: None,
        }
    }

    pub fn presentation(&self) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
        let status = match self.phase {
            TourWorkspacePhase::LessonReady => (StatusKind::Ordinary, "Lesson ready"),
            TourWorkspacePhase::ResultVisible => (StatusKind::Success, "Result visible"),
            TourWorkspacePhase::PatchbayOpen => (StatusKind::Ordinary, "Patchbay open"),
        };
        let view = SemanticApplicationView {
            revision: self.revision,
            root: node(
                "tour",
                PresentationMechanism::Shell,
                vec![
                    node(
                        "navigation",
                        PresentationMechanism::Navigation {
                            label: "Conduit products".into(),
                            current: "tour-link".into(),
                        },
                        vec![
                            node(
                                "tour-link",
                                PresentationMechanism::NavigationLink {
                                    label: "Tour".into(),
                                    destination: AdmittedNavigationDestination::Tour,
                                },
                                vec![],
                            ),
                            node(
                                "patchbay-link",
                                PresentationMechanism::NavigationLink {
                                    label: "Patchbay".into(),
                                    destination: AdmittedNavigationDestination::Patchbay,
                                },
                                vec![],
                            ),
                        ],
                    ),
                    node(
                        "workspace",
                        PresentationMechanism::Workbench,
                        vec![
                            node(
                                "lesson",
                                PresentationMechanism::Panel {
                                    title: "A first Form".into(),
                                },
                                vec![node(
                                    "lesson-status",
                                    PresentationMechanism::Status {
                                        kind: status.0,
                                        title: status.1.into(),
                                        detail: self.specimen_id.clone(),
                                    },
                                    vec![],
                                )],
                            ),
                            node(
                                "patchbay-panel",
                                PresentationMechanism::Panel {
                                    title: "Patchbay".into(),
                                },
                                vec![node(
                                    "patchbay",
                                    PresentationMechanism::PatchbayCanvas {
                                        label: patchbay_label(self),
                                    },
                                    vec![],
                                )],
                            ),
                            node(
                                "source-panel",
                                PresentationMechanism::Panel {
                                    title: "Source".into(),
                                },
                                vec![node(
                                    "source",
                                    PresentationMechanism::CodeBlock {
                                        language: "conduit".into(),
                                        code: self.source.clone(),
                                    },
                                    vec![],
                                )],
                            ),
                            node(
                                "result-panel",
                                PresentationMechanism::Panel {
                                    title: "Output".into(),
                                },
                                vec![node(
                                    "result",
                                    PresentationMechanism::Status {
                                        kind: status.0,
                                        title: status.1.into(),
                                        detail: self
                                            .result
                                            .clone()
                                            .unwrap_or_else(|| "Not run".into()),
                                    },
                                    vec![],
                                )],
                            ),
                            node(
                                "actions",
                                PresentationMechanism::ActionGroup {
                                    label: "Tour actions".into(),
                                },
                                vec![
                                    action(
                                        "run",
                                        RUN_ACTION_ID,
                                        "Run Form",
                                        if self.run_pending {
                                            ActionAvailability::Busy {
                                                detail: "Canonical Play is active".into(),
                                            }
                                        } else {
                                            ActionAvailability::Available
                                        },
                                    ),
                                    action(
                                        "open-patchbay",
                                        OPEN_PATCHBAY_ACTION_ID,
                                        "Open Patchbay",
                                        ActionAvailability::Available,
                                    ),
                                ],
                            ),
                        ],
                    ),
                ],
            ),
        };
        view.lower()?;
        Ok(view)
    }
}

fn format_phase(phase: TourWorkspacePhase) -> &'static str {
    match phase {
        TourWorkspacePhase::LessonReady => "meet-one-gear graph ready",
        TourWorkspacePhase::ResultVisible => "meet-one-gear graph played",
        TourWorkspacePhase::PatchbayOpen => "meet-one-gear graph inspection",
    }
}

fn patchbay_label(state: &TourWorkspaceState) -> String {
    match (
        state.selected_patchbay_subject.as_deref(),
        state.hovered_patchbay_subject.as_deref(),
    ) {
        (Some(selected), _) => format!("{}; selected {selected}", format_phase(state.phase)),
        (None, Some(hovered)) => format!("{}; hover {hovered}", format_phase(state.phase)),
        (None, None) => format_phase(state.phase).into(),
    }
}

fn action(
    key: &str,
    identity: &str,
    label: &str,
    availability: ActionAvailability,
) -> SemanticPresentationNode {
    node(
        key,
        PresentationMechanism::Action(SemanticAction {
            identity: identity.into(),
            event: ApplicationEventKind::Activate,
            label: label.into(),
            availability,
        }),
        vec![],
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_presentation::{ApplicationComponent, ApplicationView};

    #[test]
    fn canonical_workspace_is_one_bounded_portable_view() {
        for phase in [
            TourWorkspacePhase::LessonReady,
            TourWorkspacePhase::ResultVisible,
            TourWorkspacePhase::PatchbayOpen,
        ] {
            let state = TourWorkspaceState::canonical(7, phase);
            let lowered = state.presentation().unwrap().lower().unwrap();
            assert_eq!(
                ApplicationView::decode(&lowered.encode().unwrap()),
                Ok(lowered.clone())
            );
            assert!(
                lowered
                    .nodes
                    .iter()
                    .any(|node| node.component == ApplicationComponent::PatchbayCanvas)
            );
            assert!(
                lowered
                    .nodes
                    .iter()
                    .any(|node| node.key == state.focused_key)
            );
            assert_eq!(lowered.actions.len(), 2);
        }
    }

    #[test]
    fn specimen_identity_survives_every_surface() {
        let identities: Vec<_> = [
            TourWorkspacePhase::LessonReady,
            TourWorkspacePhase::ResultVisible,
            TourWorkspacePhase::PatchbayOpen,
        ]
        .into_iter()
        .map(|phase| TourWorkspaceState::canonical(1, phase).specimen_id)
        .collect();
        assert!(
            identities
                .iter()
                .all(|identity| identity == CANONICAL_SPECIMEN_ID)
        );
    }
}
