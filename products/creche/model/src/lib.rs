#![no_std]

extern crate alloc;

use alloc::{format, string::String, vec, vec::Vec};
use conduit_presentation::{
    ActionAvailability, ApplicationEventKind, ChoiceMultiplicity, ChoiceOption,
    EvidenceDisposition, EvidencePresentation, PresentationMechanism, SemanticAction,
    SemanticApplicationView, SemanticPresentationNode, SemanticPresentationRefusal, StatusKind,
};

mod graduation_presentation;
pub use graduation_presentation::*;
pub mod birth;
pub mod names;

pub const MINIMAL_PRESET_ACTION: &str = "configuration.preset.minimal";
pub const INTERACTIVE_PRESET_ACTION: &str = "configuration.preset.interactive";
pub const CUSTOM_PRESET_ACTION: &str = "configuration.preset.custom";
pub const REVIEW_ACTION: &str = "configuration.review";
pub const EDIT_ACTION: &str = "configuration.edit";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserConfigurationActions {
    pub revision: u32,
    pub diagnostic: Option<String>,
}

impl BrowserConfigurationActions {
    pub fn presentation(&self) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
        let mut children = vec![
            node(
                "configuration-heading",
                PresentationMechanism::Heading {
                    text: "Browser Host capabilities".into(),
                },
                vec![],
            ),
            node(
                "configuration-presets",
                PresentationMechanism::ActionGroup {
                    label: "Configuration presets".into(),
                },
                vec![
                    action("preset-minimal", MINIMAL_PRESET_ACTION, "Minimal"),
                    action(
                        "preset-interactive",
                        INTERACTIVE_PRESET_ACTION,
                        "Interactive",
                    ),
                    action("preset-custom", CUSTOM_PRESET_ACTION, "Custom"),
                    action("review-browser-configuration", REVIEW_ACTION, "Review Host"),
                ],
            ),
        ];
        if let Some(diagnostic) = &self.diagnostic {
            children.push(node(
                "configuration-diagnostic",
                PresentationMechanism::Status {
                    kind: StatusKind::Failure,
                    title: "Configuration refused".into(),
                    detail: diagnostic.clone(),
                },
                vec![],
            ));
        }
        view(
            self.revision,
            node(
                "browser-configuration",
                PresentationMechanism::Shell,
                children,
            ),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserConfigurationChoice {
    pub catalog_index: usize,
    pub label: String,
    pub implementation_id: String,
    pub selected: bool,
    pub prerequisites: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserConfigurationGroup {
    pub revision: u32,
    pub label: String,
    pub choices: Vec<BrowserConfigurationChoice>,
}

impl BrowserConfigurationGroup {
    pub fn presentation(&self) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
        let options = self
            .choices
            .iter()
            .map(|choice| ChoiceOption {
                identity: choice.implementation_id.clone(),
                label: format!("{} · {}", choice.label, choice.implementation_id),
                selected: choice.selected,
                change_action: semantic_action(
                    &format!("implementation.change-{}", choice.catalog_index),
                    "Change implementation selection",
                    ApplicationEventKind::Change,
                ),
            })
            .collect();
        let mut children = vec![node(
            "configuration-group-options",
            PresentationMechanism::ChoiceGroup {
                label: self.label.clone(),
                multiplicity: ChoiceMultiplicity::Independent,
                options,
            },
            vec![],
        )];
        for choice in &self.choices {
            for (index, prerequisite) in choice.prerequisites.iter().enumerate() {
                children.push(node(
                    &format!("prerequisite-{}-{index}", choice.catalog_index),
                    PresentationMechanism::Status {
                        kind: StatusKind::Warning,
                        title: "Future runtime condition".into(),
                        detail: format!("{prerequisite}. Not claimed satisfied here."),
                    },
                    vec![],
                ));
            }
        }
        view(
            self.revision,
            node(
                "configuration-group",
                PresentationMechanism::Panel {
                    title: self.label.clone(),
                },
                children,
            ),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserConfigurationReview {
    pub revision: u32,
    pub target_id: String,
    pub selected_implementations: Vec<String>,
    pub configuration_id: String,
    pub profile_id: String,
    pub output: String,
    pub join_mode: String,
    pub canonical_source: String,
    pub does_not_create: Vec<String>,
}

impl BrowserConfigurationReview {
    pub fn presentation(&self) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
        let definitions = [
            ("Target", self.target_id.clone()),
            ("Implementations", self.selected_implementations.join(", ")),
            ("PROFILE", self.profile_id.clone()),
            ("BrowserBundle output", self.output.clone()),
            ("Body/Spore join", self.join_mode.clone()),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (term, value))| {
            node(
                &format!("review-{index}"),
                PresentationMechanism::Definition {
                    term: term.into(),
                    value,
                },
                vec![],
            )
        })
        .collect();
        view(
            self.revision,
            node(
                "configuration-review",
                PresentationMechanism::Evidence(EvidencePresentation {
                    title: "Reviewed browser Host configuration".into(),
                    disposition: EvidenceDisposition::Succeeded,
                    identity: self.configuration_id.clone(),
                    provenance: "checked browser fabrication configuration".into(),
                }),
                vec![
                    node(
                        "configuration-review-values",
                        PresentationMechanism::DefinitionTable {
                            title: "Exact configuration identities".into(),
                        },
                        definitions,
                    ),
                    node(
                        "configuration-source",
                        PresentationMechanism::CodeBlock {
                            language: "conduit".into(),
                            code: self.canonical_source.clone(),
                        },
                        vec![],
                    ),
                    node(
                        "configuration-absent",
                        PresentationMechanism::Status {
                            kind: StatusKind::Ordinary,
                            title: format!(
                                "Configuration creates no {}.",
                                self.does_not_create.join(", ")
                            ),
                            detail: String::new(),
                        },
                        vec![],
                    ),
                    action("edit-browser-configuration", EDIT_ACTION, "Back / Edit"),
                ],
            ),
        )
    }
}

fn view(
    revision: u32,
    root: SemanticPresentationNode,
) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
    let view = SemanticApplicationView { revision, root };
    view.lower()?;
    Ok(view)
}

fn action(key: &str, identity: &str, label: &str) -> SemanticPresentationNode {
    node(
        key,
        PresentationMechanism::Action(semantic_action(
            identity,
            label,
            ApplicationEventKind::Activate,
        )),
        vec![],
    )
}

fn semantic_action(identity: &str, label: &str, event: ApplicationEventKind) -> SemanticAction {
    SemanticAction {
        identity: identity.into(),
        event,
        label: label.into(),
        availability: ActionAvailability::Available,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_groups_and_review_lower_only_through_semantic_mechanisms() {
        let actions = BrowserConfigurationActions {
            revision: 1,
            diagnostic: Some("StaleCatalogGeneration".into()),
        }
        .presentation()
        .unwrap()
        .lower()
        .unwrap();
        assert_eq!(actions.actions.len(), 4);

        let group = BrowserConfigurationGroup {
            revision: 2,
            label: "Devices".into(),
            choices: vec![BrowserConfigurationChoice {
                catalog_index: 7,
                label: "WebUSB".into(),
                implementation_id: "browser/webusb@1".into(),
                selected: true,
                prerequisites: vec!["explicit permission".into()],
            }],
        }
        .presentation()
        .unwrap()
        .lower()
        .unwrap();
        assert_eq!(group.actions[0].id, "implementation.change-7");

        let review = BrowserConfigurationReview {
            revision: 3,
            target_id: "browser/wasm32/page".into(),
            selected_implementations: vec!["browser/dom@1".into()],
            configuration_id: "configuration/1".into(),
            profile_id: "profile/1".into(),
            output: "browser-bundle".into(),
            join_mode: "separate later step".into(),
            canonical_source: "host browser {}".into(),
            does_not_create: vec!["HostId".into(), "BootId".into()],
        }
        .presentation()
        .unwrap()
        .lower()
        .unwrap();
        assert!(
            review
                .nodes
                .iter()
                .any(|node| node.key == "configuration-source")
        );
        assert_eq!(review.actions[0].id, EDIT_ACTION);
    }
}
