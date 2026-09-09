use alloc::{format, string::String, vec, vec::Vec};

use conduit_presentation::{
    Presentation, PresentationBasis, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject, PresentationText,
};

use crate::{CANONICAL_PATCHBAY_GEARS, TourWorkspacePhase, TourWorkspaceState};

pub const TOUR_WORKSPACE_SUBJECT: &str = "tour/workspace";

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
    /// Primary Patchbay workspace meaning, independent of shell geometry.
    pub fn workspace_presentation(&self) -> Result<Presentation, &'static str> {
        let mut subjects = vec![subject(
            TOUR_WORKSPACE_SUBJECT,
            PresentationRole::Form,
            "Patchbay workspace",
        )];
        let mut relationships = Vec::new();
        let mut text = vec![PresentationText {
            subject: TOUR_WORKSPACE_SUBJECT.into(),
            text: workspace_summary(self),
        }];
        for gear in CANONICAL_PATCHBAY_GEARS {
            subjects.push(subject(gear, PresentationRole::Gear, gear));
            relationships.push(PresentationRelationship {
                source: TOUR_WORKSPACE_SUBJECT.into(),
                target: gear.into(),
                kind: PresentationRelationshipKind::Contains,
            });
            text.push(PresentationText {
                subject: gear.into(),
                text: if self.selected_patchbay_subject.as_deref() == Some(gear) {
                    "selected".into()
                } else {
                    "available".into()
                },
            });
        }
        Presentation::new(
            u64::from(self.revision),
            empty_basis(),
            subjects,
            relationships,
            vec![],
            text,
        )
        .map_err(|_| "tour-workspace-presentation-refused")
    }

    /// Auxiliary inspection meaning for the selected Gear. Its relationship
    /// to the workspace lives in Presentation rather than compositor state.
    pub fn inspector_presentation(&self) -> Result<Option<Presentation>, &'static str> {
        let Some(gear) = self.selected_patchbay_subject.as_deref() else {
            return Ok(None);
        };
        if !CANONICAL_PATCHBAY_GEARS.contains(&gear) {
            return Err("tour-inspector-subject-refused");
        }
        let inspection = format!("{gear}/inspection");
        let mut subjects = vec![
            subject(
                TOUR_WORKSPACE_SUBJECT,
                PresentationRole::Form,
                "Patchbay workspace",
            ),
            subject(gear, PresentationRole::Gear, gear),
            subject(&inspection, PresentationRole::Form, "Gear inspection"),
        ];
        let text = crate::inspection::fields(self, gear, &mut subjects);
        Presentation::new(
            u64::from(self.revision),
            empty_basis(),
            subjects,
            vec![
                PresentationRelationship {
                    source: TOUR_WORKSPACE_SUBJECT.into(),
                    target: gear.into(),
                    kind: PresentationRelationshipKind::Contains,
                },
                PresentationRelationship {
                    source: inspection.clone(),
                    target: gear.into(),
                    kind: PresentationRelationshipKind::Describes,
                },
            ],
            vec![],
            text,
        )
        .map(Some)
        .map_err(|_| "tour-inspector-presentation-refused")
    }

    /// Auxiliary lifecycle status meaning, independently revisable from the
    /// workspace and inspector surfaces.
    pub fn status_presentation(&self) -> Result<Presentation, &'static str> {
        self.status_presentation_with_basis(u64::from(self.revision), empty_basis())
    }

    /// Lifecycle status backed by the exact identities owned by Conduit. The
    /// shell may render this Presentation, but it does not synthesize lifecycle
    /// truth from pixels or Patchbay selection state.
    pub fn status_presentation_with_basis(
        &self,
        revision: u64,
        basis: PresentationBasis,
    ) -> Result<Presentation, &'static str> {
        let status = "tour/status";
        let mut subjects = vec![
            subject(
                TOUR_WORKSPACE_SUBJECT,
                PresentationRole::Form,
                "Patchbay workspace",
            ),
            subject(
                status,
                PresentationRole::Status,
                "Body / Wake / Plan / Play",
            ),
        ];
        let mut relationships = vec![PresentationRelationship {
            source: status.into(),
            target: TOUR_WORKSPACE_SUBJECT.into(),
            kind: PresentationRelationshipKind::Observes,
        }];
        let mut observed = Vec::new();
        if let Some(body_id) = &basis.body_id {
            push_lifecycle_subject(
                &mut subjects,
                &mut relationships,
                &mut observed,
                status,
                body_id.as_str(),
                PresentationRole::Body,
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
                PresentationRole::Status,
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
                PresentationRole::Plan,
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
                PresentationRole::Play,
                "Play",
            );
        }
        let lifecycle = if observed.is_empty() {
            "Body absent; Wake absent; Plan absent; Play inactive".into()
        } else {
            observed.join("; ")
        };
        let mut text = vec![PresentationText {
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
            subjects.push(subject(&identity, PresentationRole::Status, label));
            relationships.push(PresentationRelationship {
                source: status.into(),
                target: identity.clone(),
                kind: PresentationRelationshipKind::Contains,
            });
            text.push(PresentationText {
                subject: identity,
                text: value.into(),
            });
        }
        Presentation::new(revision, basis, subjects, relationships, vec![], text)
            .map_err(|_| "tour-status-presentation-refused")
    }

    pub fn transient_presentation(
        &self,
        kind: TourTransientKind,
        detail: &str,
    ) -> Result<Presentation, &'static str> {
        if detail.is_empty() {
            return Err("tour-transient-detail-refused");
        }
        let transient = kind.subject_identity();
        Presentation::new(
            u64::from(self.revision),
            empty_basis(),
            vec![
                subject(
                    TOUR_WORKSPACE_SUBJECT,
                    PresentationRole::Form,
                    "Patchbay workspace",
                ),
                subject(transient, PresentationRole::Diagnostic, kind.label()),
            ],
            vec![PresentationRelationship {
                source: transient.into(),
                target: TOUR_WORKSPACE_SUBJECT.into(),
                kind: PresentationRelationshipKind::Describes,
            }],
            vec![],
            vec![PresentationText {
                subject: transient.into(),
                text: detail.into(),
            }],
        )
        .map_err(|_| "tour-transient-presentation-refused")
    }
}

fn push_lifecycle_subject(
    subjects: &mut Vec<PresentationSubject>,
    relationships: &mut Vec<PresentationRelationship>,
    observed: &mut Vec<String>,
    status: &str,
    identity: &str,
    role: PresentationRole,
    label: &str,
) {
    subjects.push(subject(identity, role, label));
    relationships.push(PresentationRelationship {
        source: status.into(),
        target: identity.into(),
        kind: PresentationRelationshipKind::Observes,
    });
    observed.push(format!("{label} present"));
}

fn subject(identity: &str, role: PresentationRole, label: &str) -> PresentationSubject {
    PresentationSubject {
        identity: identity.into(),
        role,
        label: label.into(),
        accessibility_name: label.into(),
    }
}

fn workspace_summary(state: &TourWorkspaceState) -> String {
    match state.phase {
        TourWorkspacePhase::LessonReady => "Form ready; no Play active".into(),
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

fn empty_basis() -> PresentationBasis {
    PresentationBasis {
        body_id: None,
        wake_id: None,
        source_document_id: None,
        checked_form_id: None,
        expanded_form_id: None,
        plan_id: None,
        active_play_id: None,
        sign_ids: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_gear_has_a_distinct_related_inspection_presentation() {
        let mut state = TourWorkspaceState::canonical(7, TourWorkspacePhase::PatchbayOpen);
        state.selected_patchbay_subject = Some("meet-one-gear/change".into());
        let workspace = state.workspace_presentation().unwrap();
        let inspector = state.inspector_presentation().unwrap().unwrap();

        assert_ne!(workspace.identity, inspector.identity);
        assert!(inspector.subjects.iter().any(|subject| {
            subject.identity == "meet-one-gear/change" && subject.role == PresentationRole::Gear
        }));
        assert!(inspector.relationships.iter().any(|relationship| {
            relationship.source == "meet-one-gear/change/inspection"
                && relationship.target == "meet-one-gear/change"
                && relationship.kind == PresentationRelationshipKind::Describes
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
        let status = state.status_presentation().unwrap();
        let transient = state
            .transient_presentation(TourTransientKind::Refusal, "Action refused")
            .unwrap();
        assert!(status.relationships.iter().any(|relationship| {
            relationship.target == TOUR_WORKSPACE_SUBJECT
                && relationship.kind == PresentationRelationshipKind::Observes
        }));
        assert!(transient.relationships.iter().any(|relationship| {
            relationship.target == TOUR_WORKSPACE_SUBJECT
                && relationship.kind == PresentationRelationshipKind::Describes
        }));
    }
}
