use alloc::{format, string::String, vec, vec::Vec};

use conduit_face::{
    Face, FaceBasis, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject, FaceText,
};

use crate::{CANONICAL_PATCHBAY_GEARS, TourWorkspacePhase, TourWorkspaceState};

pub const TOUR_WORKSPACE_SUBJECT: &str = "tour/workspace";
pub const INSPECTOR_CLOSE_ACTION_ID: &str = "tour.inspector.close";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourTransientKind {
    Chooser,
    Refusal,
    Confirmation,
}

impl TourTransientKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Chooser => "chooser",
            Self::Refusal => "refusal",
            Self::Confirmation => "confirmation",
        }
    }

    pub const fn subject_identity(self) -> &'static str {
        match self {
            Self::Chooser => "tour/transient/chooser",
            Self::Refusal => "tour/transient/refusal",
            Self::Confirmation => "tour/transient/confirmation",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Chooser => "Chooser",
            Self::Refusal => "Refusal detail",
            Self::Confirmation => "Confirmation",
        }
    }
}

impl TourWorkspaceState {
    pub fn run_action_available(&self) -> bool {
        matches!(
            self.run_availability(),
            conduit_face::ActionAvailability::Available
        )
    }
    /// Primary Patchbay workspace meaning, independent of shell geometry.
    pub fn workspace_face(&self) -> Result<Face, &'static str> {
        let mut subjects = vec![subject(
            TOUR_WORKSPACE_SUBJECT,
            FaceRole::Plot,
            "Patchbay workspace",
        )];
        let mut relationships = Vec::new();
        subjects.push(subject(
            crate::RUN_ACTION_ID,
            if self.run_action_available() {
                FaceRole::Action
            } else {
                FaceRole::Info
            },
            "Run Plan",
        ));
        relationships.push(FaceRelationship {
            source: TOUR_WORKSPACE_SUBJECT.into(),
            target: crate::RUN_ACTION_ID.into(),
            kind: FaceRelationshipKind::Contains,
        });
        subjects.push(subject(
            crate::OPEN_CHOOSER_ACTION_ID,
            FaceRole::Action,
            "Gears",
        ));
        relationships.push(FaceRelationship {
            source: TOUR_WORKSPACE_SUBJECT.into(),
            target: crate::OPEN_CHOOSER_ACTION_ID.into(),
            kind: FaceRelationshipKind::Contains,
        });
        let mut text = vec![FaceText {
            subject: TOUR_WORKSPACE_SUBJECT.into(),
            text: workspace_summary(self),
        }];
        crate::lesson::append(
            self.progress.chapter,
            &mut subjects,
            &mut relationships,
            &mut text,
        )?;
        for gear in CANONICAL_PATCHBAY_GEARS {
            subjects.push(subject(gear, FaceRole::Gear, gear));
            relationships.push(FaceRelationship {
                source: TOUR_WORKSPACE_SUBJECT.into(),
                target: gear.into(),
                kind: FaceRelationshipKind::Contains,
            });
            text.push(FaceText {
                subject: gear.into(),
                text: if self.selected_patchbay_subject.as_deref() == Some(gear) {
                    "selected".into()
                } else {
                    "available".into()
                },
            });
        }
        Face::new(
            u64::from(self.revision),
            empty_basis(),
            subjects,
            relationships,
            vec![],
            text,
        )
        .map_err(|_| "tour-workspace-face-refused")
    }

    /// Auxiliary inspection meaning for the selected gear. Its relationship
    /// to the workspace lives in Face rather than compositor state.
    pub fn inspector_face(&self) -> Result<Option<Face>, &'static str> {
        let Some(gear) = self.selected_patchbay_subject.as_deref() else {
            return Ok(None);
        };
        if !CANONICAL_PATCHBAY_GEARS.contains(&gear) {
            return Err("tour-inspector-subject-refused");
        }
        let inspection = format!("{gear}/inspection");
        let mut subjects = vec![
            subject(TOUR_WORKSPACE_SUBJECT, FaceRole::Plot, "Patchbay workspace"),
            subject(gear, FaceRole::Gear, gear),
            subject(&inspection, FaceRole::Plot, "Gear inspection"),
            subject(INSPECTOR_CLOSE_ACTION_ID, FaceRole::Action, "Close"),
        ];
        let text = crate::inspection::fields(self, gear, &mut subjects);
        Face::new(
            u64::from(self.revision),
            empty_basis(),
            subjects,
            vec![
                FaceRelationship {
                    source: TOUR_WORKSPACE_SUBJECT.into(),
                    target: gear.into(),
                    kind: FaceRelationshipKind::Contains,
                },
                FaceRelationship {
                    source: inspection.clone(),
                    target: gear.into(),
                    kind: FaceRelationshipKind::Describes,
                },
            ],
            vec![],
            text,
        )
        .map(Some)
        .map_err(|_| "tour-inspector-face-refused")
    }

    /// Auxiliary lifecycle status meaning, independently revisable from the
    /// workspace and inspector surfaces.
    pub fn status_face(&self) -> Result<Face, &'static str> {
        self.status_face_with_basis(u64::from(self.revision), empty_basis())
    }

    /// Lifecycle status backed by the exact identities owned by Conduit. The
    /// shell may render this Face, but it does not synthesize lifecycle
    /// truth from pixels or Patchbay selection state.
    pub fn status_face_with_basis(
        &self,
        revision: u64,
        basis: FaceBasis,
    ) -> Result<Face, &'static str> {
        let status = "tour/status";
        let mut subjects = vec![
            subject(TOUR_WORKSPACE_SUBJECT, FaceRole::Plot, "Patchbay workspace"),
            subject(status, FaceRole::Status, "Body / Wake / Plan / Play"),
        ];
        let mut relationships = vec![FaceRelationship {
            source: status.into(),
            target: TOUR_WORKSPACE_SUBJECT.into(),
            kind: FaceRelationshipKind::Observes,
        }];
        let mut observed = Vec::new();
        if let Some(body_id) = &basis.body_id {
            push_lifecycle_subject(
                &mut subjects,
                &mut relationships,
                &mut observed,
                status,
                body_id.as_str(),
                FaceRole::Body,
                "Body",
            );
        }
        if let Some(wake_id) = &basis.wake_id {
            push_lifecycle_subject(
                &mut subjects,
                &mut relationships,
                &mut observed,
                status,
                wake_id.as_str(),
                FaceRole::Status,
                "Wake",
            );
        }
        if let Some(plan_id) = &basis.plan_id {
            push_lifecycle_subject(
                &mut subjects,
                &mut relationships,
                &mut observed,
                status,
                plan_id.as_str(),
                FaceRole::Plan,
                "Plan",
            );
        }
        if let Some(play_id) = &basis.active_play_id {
            push_lifecycle_subject(
                &mut subjects,
                &mut relationships,
                &mut observed,
                status,
                play_id.as_str(),
                FaceRole::Play,
                "Play",
            );
        }
        let lifecycle = if observed.is_empty() {
            "Body absent; Wake absent; Plan absent; Play inactive".into()
        } else {
            observed.join("; ")
        };
        let mut text = vec![FaceText {
            subject: status.into(),
            text: lifecycle,
        }];
        for (key, label, value) in [
            (
                "body",
                "Body",
                if basis.body_id.is_some() {
                    "Present"
                } else {
                    "Absent"
                },
            ),
            (
                "wake",
                "Wake",
                if basis.wake_id.is_some() {
                    "Present"
                } else {
                    "Absent"
                },
            ),
            (
                "plan",
                "Plan",
                if basis.plan_id.is_some() {
                    "Present"
                } else {
                    "Absent"
                },
            ),
            (
                "play",
                "Play",
                if basis.active_play_id.is_some() {
                    "Present"
                } else {
                    "Inactive"
                },
            ),
            ("lines", "Lines", "Unobserved"),
            ("host", "Host", "Unobserved"),
        ] {
            let identity = format!("{status}/{key}");
            subjects.push(subject(&identity, FaceRole::Status, label));
            relationships.push(FaceRelationship {
                source: status.into(),
                target: identity.clone(),
                kind: FaceRelationshipKind::Contains,
            });
            text.push(FaceText {
                subject: identity,
                text: value.into(),
            });
        }
        Face::new(revision, basis, subjects, relationships, vec![], text)
            .map_err(|_| "tour-status-face-refused")
    }

    pub fn transient_face(
        &self,
        kind: TourTransientKind,
        detail: &str,
    ) -> Result<Face, &'static str> {
        if detail.is_empty() {
            return Err("tour-transient-detail-refused");
        }
        let transient = kind.subject_identity();
        Face::new(
            u64::from(self.revision),
            empty_basis(),
            vec![
                subject(TOUR_WORKSPACE_SUBJECT, FaceRole::Plot, "Patchbay workspace"),
                subject(transient, FaceRole::Diagnostic, kind.label()),
                subject(crate::TRANSIENT_CLOSE_ACTION_ID, FaceRole::Action, "Close"),
            ],
            vec![
                FaceRelationship {
                    source: transient.into(),
                    target: TOUR_WORKSPACE_SUBJECT.into(),
                    kind: FaceRelationshipKind::Describes,
                },
                FaceRelationship {
                    source: transient.into(),
                    target: crate::TRANSIENT_CLOSE_ACTION_ID.into(),
                    kind: FaceRelationshipKind::Contains,
                },
            ],
            vec![],
            vec![FaceText {
                subject: transient.into(),
                text: detail.into(),
            }],
        )
        .map_err(|_| "tour-transient-face-refused")
    }
}

fn push_lifecycle_subject(
    subjects: &mut Vec<FaceSubject>,
    relationships: &mut Vec<FaceRelationship>,
    observed: &mut Vec<String>,
    status: &str,
    identity: &str,
    role: FaceRole,
    label: &str,
) {
    subjects.push(subject(identity, role, label));
    relationships.push(FaceRelationship {
        source: status.into(),
        target: identity.into(),
        kind: FaceRelationshipKind::Observes,
    });
    observed.push(format!("{label} present"));
}

fn subject(identity: &str, role: FaceRole, label: &str) -> FaceSubject {
    FaceSubject {
        identity: identity.into(),
        role,
        name: label.into(),
    }
}

fn workspace_summary(state: &TourWorkspaceState) -> String {
    match state.phase {
        TourWorkspacePhase::LessonReady => "Plot ready; no Play active".into(),
        TourWorkspacePhase::ResultVisible => format!(
            "Play complete; result {}",
            state.result.as_deref().unwrap_or("unavailable")
        ),
        TourWorkspacePhase::PatchbayOpen => state.selected_patchbay_subject.as_deref().map_or_else(
            || "Patchbay open; no Gear selected".into(),
            |gear| format!("Patchbay open; selected {gear}"),
        ),
    }
}

fn empty_basis() -> FaceBasis {
    FaceBasis {
        body_id: None,
        wake_id: None,
        source_document_id: None,
        checked_plot_id: None,
        expanded_plot_id: None,
        plan_id: None,
        active_play_id: None,
        sign_ids: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_gear_has_a_distinct_related_inspection_face() {
        let mut state = TourWorkspaceState::canonical(7, TourWorkspacePhase::PatchbayOpen);
        state.selected_patchbay_subject = Some("meet-one-gear/change".into());
        let workspace = state.workspace_face().unwrap();
        let inspector = state.inspector_face().unwrap().unwrap();

        assert_ne!(workspace.identity, inspector.identity);
        assert!(inspector.subjects.iter().any(|subject| {
            subject.identity == "meet-one-gear/change" && subject.role == FaceRole::Gear
        }));
        assert!(inspector.relationships.iter().any(|relationship| {
            relationship.source == "meet-one-gear/change/inspection"
                && relationship.target == "meet-one-gear/change"
                && relationship.kind == FaceRelationshipKind::Describes
        }));
        let detail = inspector.text.first().expect("inspection detail");
        assert_eq!(detail.text, "text/upper");
        assert!(
            inspector
                .text
                .iter()
                .any(|item| item.text.contains("value/text"))
        );
        assert!(
            !inspector
                .text
                .iter()
                .any(|item| item.text.contains("HELLO"))
        );
    }

    #[test]
    fn status_and_transient_relationships_are_not_compositor_roles() {
        let state = TourWorkspaceState::canonical(4, TourWorkspacePhase::PatchbayOpen);
        let status = state.status_face().unwrap();
        let transient = state
            .transient_face(TourTransientKind::Refusal, "Action refused")
            .unwrap();
        assert!(status.relationships.iter().any(|relationship| {
            relationship.target == TOUR_WORKSPACE_SUBJECT
                && relationship.kind == FaceRelationshipKind::Observes
        }));
        assert!(transient.relationships.iter().any(|relationship| {
            relationship.target == TOUR_WORKSPACE_SUBJECT
                && relationship.kind == FaceRelationshipKind::Describes
        }));
    }
}
