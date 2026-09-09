use super::*;
use alloc::{format, vec};
use conduit_presentation::{
    ActionAvailability, ApplicationEventKind, ChoiceMultiplicity, ChoiceOption, FieldKind,
    FormField, PresentationMechanism, SemanticAction, SemanticApplicationView,
    SemanticPresentationNode, SemanticPresentationRefusal,
};

impl BirthDraft {
    pub fn presentation(&self) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
        let mut birth = action("creche.birth", "Birth Body", ApplicationEventKind::Activate);
        if let Err(error) = self.selection(self.revision) {
            birth.availability = ActionAvailability::Unavailable {
                detail: format!("{error:?}"),
            };
        }
        let choices = self
            .choices
            .iter()
            .enumerate()
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
        let view = SemanticApplicationView {
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
                        PresentationMechanism::ChoiceGroup {
                            label: "Naming tradition".into(),
                            multiplicity: ChoiceMultiplicity::Exclusive,
                            options: self
                                .naming_systems()
                                .enumerate()
                                .map(|(index, (id, label))| ChoiceOption {
                                    identity: id.into(),
                                    label: label.into(),
                                    selected: id == self.requested_system,
                                    change_action: action(
                                        &format!("creche.naming.{index}"),
                                        "Use naming tradition",
                                        ApplicationEventKind::Change,
                                    ),
                                })
                                .collect(),
                        },
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
                    node(
                        "initial-forms",
                        PresentationMechanism::ChoiceGroup {
                            label: "Forms to include".into(),
                            multiplicity: ChoiceMultiplicity::Independent,
                            options: choices,
                        },
                        vec![],
                    ),
                    node("birth-body", PresentationMechanism::Action(birth), vec![]),
                ],
            ),
        };
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
