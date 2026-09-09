use super::*;
use alloc::{format, vec};
use conduit_presentation::{
    ActionAvailability, ApplicationEventKind, ChoiceMultiplicity, ChoiceOption, FieldKind,
    FormField, PresentationMechanism, SelectOption, SemanticAction, SemanticApplicationView,
    SemanticPresentationNode, SemanticPresentationRefusal, StatusKind,
};

impl BirthDraft {
    pub fn presentation(&self) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
        let mut birth = action("creche.birth", "Birth Body", ApplicationEventKind::Activate);
        if let Err(error) = self.selection(self.revision) {
            birth.availability = ActionAvailability::Unavailable {
                detail: format!("{error:?}"),
            };
        }
        let query = self.search.to_lowercase();
        let choices: Vec<_> = self
            .choices
            .iter()
            .enumerate()
            .filter(|(_, choice)| {
                let text = format!("{} {}", choice.title, choice.search_text).to_lowercase();
                query.split_whitespace().all(|term| text.contains(term))
            })
            .map(|(index, choice)| {
                let mut change = action(
                    &format!("creche.form.{index}"),
                    "Include Form",
                    ApplicationEventKind::Change,
                );
                if let Some(detail) = &choice.refusal {
                    change.availability = ActionAvailability::Unavailable {
                        detail: detail.clone(),
                    };
                }
                ChoiceOption {
                    identity: choice.form.checked_form_id.as_str().into(),
                    label: choice.title.clone(),
                    selected: choice.selected,
                    change_action: change,
                }
            })
            .collect();
        let form_selection = if choices.is_empty() {
            PresentationMechanism::Status {
                kind: StatusKind::Ordinary,
                title: "No Forms match your search.".into(),
                detail: "Your selected Forms are still included.".into(),
            }
        } else {
            PresentationMechanism::ChoiceGroup {
                label: "Forms to include".into(),
                multiplicity: ChoiceMultiplicity::Independent,
                options: choices,
            }
        };
        let mut view = SemanticApplicationView {
            revision: self.revision,
            root: node(
                "creche",
                PresentationMechanism::Shell,
                vec![
                    node(
                        "creche-heading",
                        PresentationMechanism::Heading {
                            text: "A Body of your own".into(),
                        },
                        vec![],
                    ),
                    node(
                        "body-name",
                        PresentationMechanism::FormField(FormField {
                            label: "Friendly Body name".into(),
                            help: "Give it a name, or use a suggestion.".into(),
                            error: None,
                            value: self.friendly_name.clone(),
                            value_capacity: MAX_FRIENDLY_NAME_BYTES as u32,
                            input_action: action(
                                "creche.name",
                                "Edit Body name",
                                ApplicationEventKind::Input,
                            ),
                            kind: FieldKind::Text,
                        }),
                        vec![],
                    ),
                    node(
                        "name-system",
                        PresentationMechanism::FormField(FormField {
                            label: "Naming tradition".into(),
                            help: "Choose a tradition for the next name suggestion.".into(),
                            error: None,
                            value: self.requested_system.clone(),
                            value_capacity: 64,
                            input_action: action(
                                "creche.naming",
                                "Use naming tradition",
                                ApplicationEventKind::Change,
                            ),
                            kind: FieldKind::NamedSelect {
                                options: self
                                    .naming_systems()
                                    .map(|(id, label)| SelectOption {
                                        identity: id.into(),
                                        label: label.into(),
                                    })
                                    .collect(),
                            },
                        }),
                        vec![],
                    ),
                    node(
                        "suggest-name",
                        PresentationMechanism::Action(action(
                            "creche.suggest",
                            "Suggest another name",
                            ApplicationEventKind::Activate,
                        )),
                        vec![],
                    ),
                    node("initial-forms", form_selection, vec![]),
                    node("birth-body", PresentationMechanism::Action(birth), vec![]),
                ],
            ),
        };
        if self.choices.len() > 1 {
            view.root.children.insert(
                4,
                node(
                    "form-search",
                    PresentationMechanism::FormField(FormField {
                        label: "Search Forms".into(),
                        help: "Find a Form by name or what it uses.".into(),
                        error: None,
                        value: self.search.clone(),
                        value_capacity: 128,
                        input_action: action(
                            "creche.search",
                            "Search Forms",
                            ApplicationEventKind::Input,
                        ),
                        kind: FieldKind::Text,
                    }),
                    vec![],
                ),
            );
            view.root.children.insert(
                6,
                node(
                    "selected-forms",
                    PresentationMechanism::Status {
                        kind: StatusKind::Ordinary,
                        title: format!(
                            "Selected: {}",
                            self.choices.iter().filter(|choice| choice.selected).count()
                        ),
                        detail: String::new(),
                    },
                    vec![],
                ),
            );
        }
        view.lower()?;
        Ok(view)
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
fn action(identity: &str, label: &str, event: ApplicationEventKind) -> SemanticAction {
    SemanticAction {
        identity: identity.into(),
        label: label.into(),
        event,
        availability: ActionAvailability::Available,
    }
}
