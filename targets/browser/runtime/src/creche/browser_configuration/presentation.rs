//! Maps authoritative browser Host configuration truth into presentation semantics.

use std::collections::BTreeSet;

use conduit_host_browser_make::BROWSER_IMPLEMENTATIONS;
use conduit_presentation::{
    ActionAvailability, ApplicationEventKind, ChoiceMultiplicity, ChoiceOption,
    EvidenceDisposition, EvidencePresentation, PresentationMechanism, SemanticAction,
    SemanticApplicationView, SemanticPresentationNode, SemanticPresentationRefusal, StatusKind,
};
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
struct BrowserConfigurationReviewPresentation {
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
    fn presentation(&self) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
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

impl BrowserConfigurationGroup {
    fn presentation(&self) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
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

impl BrowserConfigurationReviewPresentation {
    fn presentation(&self) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
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
                    provenance: "checked browser make configuration".into(),
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

fn presentation_view(request: BrowserConfigurationViewRequest) -> Result<Vec<u8>, String> {
    let semantic = match request.mode.as_str() {
        "actions" => BrowserConfigurationActions {
            revision: request.revision,
            diagnostic: request.diagnostic,
        }
        .presentation(),
        "group" => {
            if request.catalog_generation != CATALOG_GENERATION {
                return Err("StaleCatalogGeneration: configuration presentation is stale".into());
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
            .presentation()
        }
        "review" => {
            let (reviewed, _, _) = review(BrowserConfigurationSelection {
                catalog_generation: request.catalog_generation,
                implementations: request.implementations,
            })?;
            BrowserConfigurationReviewPresentation {
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
            .presentation()
        }
        mode => return Err(format!("UnknownConfigurationPresentation: {mode}")),
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
        .map_err(|error| format!("InvalidConfigurationPresentation: {error}"))
        .and_then(presentation_view)
        .and_then(|encoded| {
            super::super::abi::write_output_bytes(&encoded)
                .map_err(|_| "ConfigurationPresentationOutputBound".to_string())
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
        let encoded = presentation_view(BrowserConfigurationViewRequest {
            revision: 1,
            mode: "group".into(),
            catalog_generation: CATALOG_GENERATION,
            implementations: DEFAULT_IMPLEMENTATIONS
                .iter()
                .map(|value| (*value).into())
                .collect(),
            group: Some("Presentation".into()),
            diagnostic: None,
        })
        .unwrap();
        let view = conduit_presentation::ApplicationView::decode(&encoded).unwrap();
        assert!(view
            .nodes
            .iter()
            .any(|node| node.key == "configuration-group-options"));
        assert!(view
            .actions
            .iter()
            .all(|action| action.id.starts_with("implementation.change-")));

        let unknown = presentation_view(BrowserConfigurationViewRequest {
            revision: 2,
            mode: "group".into(),
            catalog_generation: CATALOG_GENERATION,
            implementations: vec!["browser/unknown@1".into()],
            group: Some("Presentation".into()),
            diagnostic: None,
        })
        .unwrap_err();
        assert!(unknown.contains("StaleImplementation"));
    }
}
