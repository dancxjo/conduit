use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_presentation::{
    ActionAvailability, ApplicationEventKind, EvidenceDisposition, EvidencePresentation,
    PresentationMechanism, SemanticAction, SemanticApplicationView, SemanticPresentationNode,
    SemanticPresentationRefusal, StatusKind,
};

pub const HOST_PATCHBAY_ACTION: &str = "graduate.host-patchbay";
pub const WITHOUT_PATCHBAY_ACTION: &str = "graduate.without-patchbay";
pub const END_CRECHE_ACTION: &str = "graduate.end";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraduationControls {
    pub revision: u32,
    pub durable_identity: bool,
    pub birth_evidence: bool,
    pub current_admitted_part: bool,
    pub ready: bool,
    pub graduated: bool,
    pub status: String,
    pub status_kind: StatusKind,
}

impl GraduationControls {
    pub fn presentation(&self) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
        let choices_available = self.ready && !self.graduated;
        let mut actions = Vec::new();
        if choices_available {
            actions.push(action(HOST_PATCHBAY_ACTION, "Host Patchbay on this Body"));
            actions.push(action(
                WITHOUT_PATCHBAY_ACTION,
                "Finish without hosted Patchbay",
            ));
        }
        if self.graduated {
            actions.push(action(END_CRECHE_ACTION, "End the Crèche"));
        }
        semantic_view(
            self.revision,
            node(
                "graduation",
                PresentationMechanism::Shell,
                vec![
                    node(
                        "graduation-criteria",
                        PresentationMechanism::Grid,
                        vec![
                            criterion(
                                "durable-identity",
                                "Durable Body identity",
                                self.durable_identity,
                            ),
                            criterion(
                                "birth-evidence",
                                "Bound BIRTH evidence",
                                self.birth_evidence,
                            ),
                            criterion(
                                "current-part",
                                "Current admitted Part",
                                self.current_admitted_part,
                            ),
                        ],
                    ),
                    node(
                        "graduation-status",
                        PresentationMechanism::Status {
                            kind: self.status_kind,
                            title: self.status.clone(),
                            detail: String::new(),
                        },
                        vec![],
                    ),
                    node(
                        "graduation-actions",
                        PresentationMechanism::ActionGroup {
                            label: "Graduation actions".into(),
                        },
                        actions,
                    ),
                ],
            ),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraduationEvidenceView {
    pub revision: u32,
    pub body_id: String,
    pub choice: String,
    pub sign_id: String,
    pub patchbay_plan_id: Option<String>,
    pub patchbay_implementation_id: Option<String>,
    pub creche_required: bool,
    pub canonical_json: String,
}

impl GraduationEvidenceView {
    pub fn presentation(&self) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
        let values = [
            ("Body", self.body_id.clone()),
            ("Choice", self.choice.clone()),
            ("Graduation Sign", self.sign_id.clone()),
            (
                "Patchbay Plan",
                self.patchbay_plan_id
                    .clone()
                    .unwrap_or_else(|| "not hosted".into()),
            ),
            (
                "Patchbay implementation",
                self.patchbay_implementation_id
                    .clone()
                    .unwrap_or_else(|| "not hosted".into()),
            ),
            ("Crèche required", self.creche_required.to_string()),
        ];
        let definitions = values
            .into_iter()
            .enumerate()
            .map(|(index, (term, value))| {
                node(
                    &alloc::format!("graduation-{index}"),
                    PresentationMechanism::Definition {
                        term: term.into(),
                        value,
                    },
                    vec![],
                )
            })
            .collect();
        semantic_view(
            self.revision,
            node(
                "graduation-evidence",
                PresentationMechanism::Evidence(EvidencePresentation {
                    title: "Graduation evidence".into(),
                    disposition: EvidenceDisposition::Succeeded,
                    identity: self.sign_id.clone(),
                    provenance: "exact Crèche graduation receipt".into(),
                }),
                vec![
                    node(
                        "graduation-identities",
                        PresentationMechanism::DefinitionTable {
                            title: "Exact graduation identities".into(),
                        },
                        definitions,
                    ),
                    node(
                        "graduation-raw",
                        PresentationMechanism::Disclosure {
                            summary: "Raw graduation evidence".into(),
                        },
                        vec![node(
                            "graduation-raw-json",
                            PresentationMechanism::CodeBlock {
                                language: "json".into(),
                                code: self.canonical_json.clone(),
                            },
                            vec![],
                        )],
                    ),
                ],
            ),
        )
    }
}

fn criterion(key: &str, title: &str, ready: bool) -> SemanticPresentationNode {
    node(
        key,
        PresentationMechanism::Panel {
            title: alloc::format!("{title} · {}", if ready { "ready" } else { "waiting" }),
        },
        vec![],
    )
}

fn action(identity: &str, label: &str) -> SemanticPresentationNode {
    node(
        identity,
        PresentationMechanism::Action(SemanticAction {
            identity: identity.into(),
            event: ApplicationEventKind::Activate,
            label: label.into(),
            availability: ActionAvailability::Available,
        }),
        vec![],
    )
}

fn semantic_view(
    revision: u32,
    root: SemanticPresentationNode,
) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
    let view = SemanticApplicationView { revision, root };
    view.lower()?;
    Ok(view)
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

    #[test]
    fn graduation_controls_admit_only_current_actions() {
        let waiting = GraduationControls {
            revision: 1,
            durable_identity: true,
            birth_evidence: false,
            current_admitted_part: false,
            ready: false,
            graduated: false,
            status: "Not ready".into(),
            status_kind: StatusKind::Ordinary,
        }
        .presentation()
        .unwrap()
        .lower()
        .unwrap();
        assert!(waiting.actions.is_empty());

        let ready = GraduationControls {
            ready: true,
            ..GraduationControls {
                revision: 2,
                durable_identity: true,
                birth_evidence: true,
                current_admitted_part: true,
                ready: false,
                graduated: false,
                status: "Ready".into(),
                status_kind: StatusKind::Ordinary,
            }
        }
        .presentation()
        .unwrap()
        .lower()
        .unwrap();
        assert_eq!(
            ready
                .actions
                .iter()
                .map(|action| action.id.as_str())
                .collect::<Vec<_>>(),
            vec![HOST_PATCHBAY_ACTION, WITHOUT_PATCHBAY_ACTION]
        );
    }

    #[test]
    fn graduation_evidence_is_finite_semantic_evidence() {
        let view = GraduationEvidenceView {
            revision: 3,
            body_id: "body/1".into(),
            choice: "host-patchbay".into(),
            sign_id: "sign/1".into(),
            patchbay_plan_id: Some("plan/1".into()),
            patchbay_implementation_id: Some("browser/patchbay-surface@1".into()),
            creche_required: false,
            canonical_json: "{}".into(),
        }
        .presentation()
        .unwrap()
        .lower()
        .unwrap();
        assert!(
            view.nodes
                .iter()
                .any(|node| node.key == "graduation-identities")
        );
        assert!(view.encode().unwrap().len() < 65_536);
    }
}
