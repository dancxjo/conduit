//! The Birth Plot's live Face projection over renderer-neutral draft state.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_face::{
    ActionAvailability, ApplicationEventKind, ChoiceMultiplicity, ChoiceOption, FaceMechanism,
    FieldKind, FormField, SelectOption, SemanticAction, SemanticApplicationView, SemanticFaceNode,
    SemanticFaceRefusal, StatusKind,
};

use crate::{BirthDraft, names::MAX_FRIENDLY_NAME_BYTES};

/// The current browser/native widget view, separate from Birth plot meaning.
pub trait BirthWidgetFace {
    fn widget_face(&self) -> Result<SemanticApplicationView, SemanticFaceRefusal>;
}

impl BirthWidgetFace for BirthDraft {
    fn widget_face(&self) -> Result<SemanticApplicationView, SemanticFaceRefusal> {
        let mut birth = action("creche.birth", "Birth Body", ApplicationEventKind::Activate);
        let readiness = self.selection(self.revision());
        if let Err(error) = &readiness {
            birth.availability = ActionAvailability::Unavailable {
                detail: format!("{error:?}"),
            };
        }
        let selected_count = self
            .choices()
            .iter()
            .filter(|choice| choice.selected)
            .count();
        let review = format!(
            "Name: {}. Starting Plots selected: {} of {}. Birth: {}.",
            if self.friendly_name().is_empty() {
                "(empty)"
            } else {
                self.friendly_name()
            },
            selected_count,
            self.choices().len(),
            match readiness {
                Ok(_) => "available".into(),
                Err(error) => format!("unavailable ({error:?})"),
            }
        );
        // Search narrows the editable inventory, but it must never hide a
        // selected Plot from the review that precedes Birth.
        let review_plots = self
            .choices()
            .iter()
            .enumerate()
            .filter(|(_, choice)| choice.selected)
            .map(|(index, choice)| {
                node(
                    &format!("birth-review-plot-{index}"),
                    FaceMechanism::Status {
                        kind: StatusKind::Ordinary,
                        title: format!("Starting Plot {}", index + 1),
                        detail: choice.title.clone(),
                    },
                    vec![],
                )
            })
            .collect();
        let query = self.search().to_lowercase();
        let choices: Vec<_> = self
            .choices()
            .iter()
            .enumerate()
            .filter(|(_, choice)| {
                let text = format!("{} {}", choice.title, choice.search_text).to_lowercase();
                query.split_whitespace().all(|term| text.contains(term))
            })
            .map(|(index, choice)| {
                let mut change = action(
                    &format!("creche.plot.{index}"),
                    "Include Plot",
                    ApplicationEventKind::Change,
                );
                if let Some(detail) = &choice.refusal {
                    change.availability = ActionAvailability::Unavailable {
                        detail: detail.clone(),
                    };
                }
                ChoiceOption {
                    identity: crate::plot_choice_identity::plot_choice_identity(&choice.plot),
                    label: choice.title.clone(),
                    selected: choice.selected,
                    change_action: change,
                }
            })
            .collect();
        let plot_selection = if choices.is_empty() {
            FaceMechanism::Status {
                kind: StatusKind::Ordinary,
                title: "No Plots match your search.".into(),
                detail: "Your selected plots are still included.".into(),
            }
        } else {
            FaceMechanism::ChoiceGroup {
                label: "Plots to include".into(),
                multiplicity: ChoiceMultiplicity::Independent,
                options: choices,
            }
        };
        let mut view = SemanticApplicationView {
            revision: self.revision(),
            root: node(
                "creche",
                FaceMechanism::Shell,
                vec![
                    node(
                        "creche-heading",
                        FaceMechanism::Heading {
                            text: "A body of your own".into(),
                        },
                        vec![],
                    ),
                    node(
                        "body-name",
                        FaceMechanism::FormField(FormField {
                            label: "Friendly Body name".into(),
                            help: "Give it a name, or use a suggestion.".into(),
                            error: None,
                            value: self.friendly_name().into(),
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
                        FaceMechanism::FormField(FormField {
                            label: "Naming tradition".into(),
                            help: "Choose a tradition for the next name suggestion.".into(),
                            error: None,
                            value: self.requested_system().into(),
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
                        FaceMechanism::Action(action(
                            "creche.suggest",
                            "Suggest another name",
                            ApplicationEventKind::Activate,
                        )),
                        vec![],
                    ),
                    node("initial-plots", plot_selection, vec![]),
                    node(
                        "birth-review",
                        FaceMechanism::Status {
                            kind: StatusKind::Ordinary,
                            title: "Review Birth choices".into(),
                            detail: review,
                        },
                        review_plots,
                    ),
                    node("birth-body", FaceMechanism::Action(birth), vec![]),
                ],
            ),
        };
        if self.choices().len() > 1 {
            view.root.children.insert(
                4,
                node(
                    "plot-search",
                    FaceMechanism::FormField(FormField {
                        label: "Search Plots".into(),
                        help: "Find a plot by name or what it uses.".into(),
                        error: None,
                        value: self.search().into(),
                        value_capacity: 128,
                        input_action: action(
                            "creche.search",
                            "Search Plots",
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
                    "selected-plots",
                    FaceMechanism::Status {
                        kind: StatusKind::Ordinary,
                        title: format!(
                            "Selected: {}",
                            self.choices()
                                .iter()
                                .filter(|choice| choice.selected)
                                .count()
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
fn node(key: &str, mechanism: FaceMechanism, children: Vec<SemanticFaceNode>) -> SemanticFaceNode {
    SemanticFaceNode {
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
