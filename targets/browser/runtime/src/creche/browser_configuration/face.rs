//! Maps authoritative browser Host configuration truth into face semantics.

use std::collections::BTreeSet;

use conduit_face::{
    ActionAvailability, ApplicationEventKind, ChoiceMultiplicity, ChoiceOption,
    EvidenceDisposition, EvidenceFace, FaceMechanism, SemanticAction, SemanticApplicationView,
    SemanticFaceNode, SemanticFaceRefusal, StatusKind,
};
use conduit_host_browser_make::BROWSER_IMPLEMENTATIONS;
use serde::Deserialize;

use super::{review, BrowserConfigurationSelection, CATALOG_GENERATION};

const MINIMAL_PRESET_ACTION: &str = "configuration.preset.minimal";
const INTERACTIVE_PRESET_ACTION: &str = "configuration.preset.interactive";
const CUSTOM_PRESET_ACTION: &str = "configuration.preset.custom";
const REVIEW_ACTION: &str = "configuration.review";
const EDIT_ACTION: &str = "configuration.edit";

struct BrowserConfigurationActions {
    revision: u32,
    diagnostic: Option<String>,
}
struct BrowserConfigurationChoice {
    catalog_index: usize,
    label: String,
    implementation_id: String,
    selected: bool,
    prerequisites: Vec<String>,
}
struct BrowserConfigurationGroup {
    revision: u32,
    label: String,
    choices: Vec<BrowserConfigurationChoice>,
}
struct BrowserConfigurationReviewFace {
    revision: u32,
    target_id: String,
    selected_implementations: Vec<String>,
    configuration_id: String,
    profile_id: String,
    output: String,
    join_mode: String,
    canonical_source: String,
    does_not_create: Vec<String>,
}

impl BrowserConfigurationActions {
    fn face(&self) -> Result<SemanticApplicationView, SemanticFaceRefusal> {
        let mut children = vec![
            node(
                "configuration-heading",
                FaceMechanism::Heading {
                    text: "Browser Host capabilities".into(),
                },
                vec![],
            ),
            node(
                "configuration-presets",
                FaceMechanism::ActionGroup {
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
                FaceMechanism::Status {
                    kind: StatusKind::Failure,
                    title: "Configuration refused".into(),
                    detail: diagnostic.clone(),
                },
                vec![],
            ));
        }
        view(
            self.revision,
            node("browser-configuration", FaceMechanism::Shell, children),
        )
    }
}

impl BrowserConfigurationGroup {
    fn face(&self) -> Result<SemanticApplicationView, SemanticFaceRefusal> {
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
            FaceMechanism::ChoiceGroup {
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
                    FaceMechanism::Status {
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
                FaceMechanism::Panel {
                    title: self.label.clone(),
                },
                children,
            ),
        )
    }
}

impl BrowserConfigurationReviewFace {
    fn face(&self) -> Result<SemanticApplicationView, SemanticFaceRefusal> {
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
                FaceMechanism::Definition {
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
                FaceMechanism::Evidence(EvidenceFace {
                    title: "Reviewed browser Host configuration".into(),
                    disposition: EvidenceDisposition::Succeeded,
                    identity: self.configuration_id.clone(),
                    provenance: "checked browser make configuration".into(),
                }),
                vec![
                    node(
                        "configuration-review-values",
                        FaceMechanism::DefinitionTable {
                            title: "Exact configuration identities".into(),
                        },
                        definitions,
                    ),
                    node(
                        "configuration-source",
                        FaceMechanism::CodeBlock {
                            language: "conduit".into(),
                            code: self.canonical_source.clone(),
                        },
                        vec![],
                    ),
                    node(
                        "configuration-absent",
                        FaceMechanism::Status {
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
    root: SemanticFaceNode,
) -> Result<SemanticApplicationView, SemanticFaceRefusal> {
    let view = SemanticApplicationView { revision, root };
    view.lower()?;
    Ok(view)
}
fn action(key: &str, identity: &str, label: &str) -> SemanticFaceNode {
    node(
        key,
        FaceMechanism::Action(semantic_action(
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
fn node(key: &str, mechanism: FaceMechanism, children: Vec<SemanticFaceNode>) -> SemanticFaceNode {
    SemanticFaceNode {
        key: key.into(),
        mechanism,
        children,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserConfigurationViewRequest {
    revision: u32,
    mode: String,
    catalog_generation: u32,
    implementations: Vec<String>,
    group: Option<String>,
    diagnostic: Option<String>,
}

fn face_view(request: BrowserConfigurationViewRequest) -> Result<Vec<u8>, String> {
    let semantic = match request.mode.as_str() {
        "actions" => BrowserConfigurationActions {
            revision: request.revision,
            diagnostic: request.diagnostic,
        }
        .face(),
        "group" => {
            if request.catalog_generation != CATALOG_GENERATION {
                return Err("StaleCatalogGeneration: configuration face is stale".into());
            }
            let selected = reviewed_selection(&request.implementations)?;
            let group = request.group.ok_or_else(|| {
                "MissingConfigurationGroup: group view omitted its group".to_string()
            })?;
            let choices = BROWSER_IMPLEMENTATIONS
                .iter()
                .enumerate()
                .filter(|(_, descriptor)| descriptor.group == group)
                .map(|(catalog_index, descriptor)| BrowserConfigurationChoice {
                    catalog_index,
                    label: descriptor.label.into(),
                    implementation_id: descriptor.implementation_id.into(),
                    selected: selected.contains(descriptor.implementation_id),
                    prerequisites: descriptor
                        .prerequisites
                        .iter()
                        .map(|prerequisite| prerequisite.detail.into())
                        .collect(),
                })
                .collect::<Vec<_>>();
            if choices.is_empty() {
                return Err(format!("UnknownConfigurationGroup: {group}"));
            }
            BrowserConfigurationGroup {
                revision: request.revision,
                label: group,
                choices,
            }
            .face()
        }
        "review" => {
            let (reviewed, _, _) = review(BrowserConfigurationSelection {
                catalog_generation: request.catalog_generation,
                implementations: request.implementations,
            })?;
            BrowserConfigurationReviewFace {
                revision: request.revision,
                target_id: reviewed.target_id.into(),
                selected_implementations: reviewed.selected_implementations.clone(),
                configuration_id: reviewed.configuration_id.clone(),
                profile_id: reviewed.profile_id.clone(),
                output: reviewed.output.into(),
                join_mode: reviewed.join_mode.into(),
                canonical_source: reviewed.canonical_source.clone(),
                does_not_create: reviewed
                    .does_not_create
                    .iter()
                    .map(|value| (*value).into())
                    .collect(),
            }
            .face()
        }
        mode => return Err(format!("UnknownConfigurationFace: {mode}")),
    }
    .map_err(|error| format!("describe browser configuration: {error:?}"))?;
    semantic
        .lower()
        .map_err(|error| format!("lower browser configuration: {error:?}"))?
        .encode()
        .map_err(|error| format!("encode browser configuration: {error:?}"))
}

fn reviewed_selection(implementations: &[String]) -> Result<BTreeSet<&str>, String> {
    if implementations.len() > BROWSER_IMPLEMENTATIONS.len() {
        return Err("SelectionBound: browser selection exceeds the reviewed catalog bound".into());
    }
    let mut selected = BTreeSet::new();
    for implementation in implementations {
        if !BROWSER_IMPLEMENTATIONS
            .iter()
            .any(|descriptor| descriptor.implementation_id == implementation)
        {
            return Err(format!("StaleImplementation: {implementation}"));
        }
        if !selected.insert(implementation.as_str()) {
            return Err(format!("DuplicateImplementation: {implementation}"));
        }
    }
    Ok(selected)
}

#[no_mangle]
pub extern "C" fn conduit_creche_browser_configuration_view(length: usize) -> i32 {
    super::super::abi::clear_output();
    let bytes = match super::super::abi::take_input(length) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    match serde_json::from_slice::<BrowserConfigurationViewRequest>(&bytes)
        .map_err(|error| format!("InvalidConfigurationFace: {error}"))
        .and_then(face_view)
        .and_then(|encoded| {
            super::super::abi::write_output_bytes(&encoded)
                .map_err(|_| "ConfigurationFaceOutputBound".to_string())
        }) {
        Ok(()) => 0,
        Err(message) => super::super::abi::refuse(message, super::super::abi::ERROR_SPORE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creche::browser_configuration::DEFAULT_IMPLEMENTATIONS;

    #[test]
    fn configuration_views_are_target_semantics_with_bounded_catalog_inputs() {
        let encoded = face_view(BrowserConfigurationViewRequest {
            revision: 1,
            mode: "group".into(),
            catalog_generation: CATALOG_GENERATION,
            implementations: DEFAULT_IMPLEMENTATIONS
                .iter()
                .map(|value| (*value).into())
                .collect(),
            group: Some("Face".into()),
            diagnostic: None,
        })
        .unwrap();
        let view = conduit_face::ApplicationView::decode(&encoded).unwrap();
        assert!(view
            .nodes
            .iter()
            .any(|node| node.key == "configuration-group-options"));
        assert!(view
            .actions
            .iter()
            .all(|action| action.id.starts_with("implementation.change-")));

        let unknown = face_view(BrowserConfigurationViewRequest {
            revision: 2,
            mode: "group".into(),
            catalog_generation: CATALOG_GENERATION,
            implementations: vec!["browser/unknown@1".into()],
            group: Some("Face".into()),
            diagnostic: None,
        })
        .unwrap_err();
        assert!(unknown.contains("StaleImplementation"));
    }
}
