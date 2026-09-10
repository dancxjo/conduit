use alloc::{format, string::String, vec, vec::Vec};
use conduit_presentation::{
    ActionAvailability, ApplicationEventKind, FieldKind, FormField, PresentationMechanism,
    SemanticAction, SemanticApplicationView, SemanticPresentationNode, SemanticPresentationRefusal,
    StatusKind,
};

use super::node;

pub const GALLERY_SEARCH_BYTES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TourGalleryEntry {
    pub title: String,
    pub category: String,
    pub description: String,
    pub checked_form_id: String,
    pub realization: String,
    pub search_terms: String,
    pub runnable: bool,
    pub handoff: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TourGalleryState {
    pub revision: u32,
    pub query: String,
    pub selected_checked_form_id: Option<String>,
    pub entries: Vec<TourGalleryEntry>,
}

impl TourGalleryState {
    pub fn presentation(&self) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
        let over_capacity = self.query.len() > GALLERY_SEARCH_BYTES;
        let normalized = self.query.trim().to_ascii_lowercase();
        let terms = normalized.split_whitespace().collect::<Vec<_>>();
        let visible = self
            .entries
            .iter()
            .filter(|entry| {
                !over_capacity
                    && terms.iter().all(|term| {
                        format!(
                            "{} {} {} {} {}",
                            entry.title,
                            entry.checked_form_id,
                            entry.search_terms,
                            entry.category,
                            entry.description
                        )
                        .to_ascii_lowercase()
                        .contains(term)
                    })
            })
            .collect::<Vec<_>>();

        let mut cards = Vec::with_capacity(visible.len());
        for entry in visible {
            let index = self
                .entries
                .iter()
                .position(|candidate| candidate.checked_form_id == entry.checked_form_id)
                .expect("visible Gallery entry belongs to its bounded source");
            let selected =
                self.selected_checked_form_id.as_deref() == Some(entry.checked_form_id.as_str());
            cards.push(node(
                &format!("form-{index}"),
                PresentationMechanism::Panel {
                    title: String::new(),
                },
                vec![
                    node(
                        &format!("form-intro-{index}"),
                        PresentationMechanism::Status {
                            kind: StatusKind::Ordinary,
                            title: format!("{:02} / {}", index + 1, entry.category),
                            detail: entry.description.clone(),
                        },
                        vec![],
                    ),
                    node(
                        &format!("form-heading-{index}"),
                        PresentationMechanism::Heading {
                            text: entry.title.clone(),
                        },
                        vec![],
                    ),
                    node(
                        &format!("form-state-{index}"),
                        PresentationMechanism::Status {
                            kind: if selected {
                                StatusKind::Success
                            } else {
                                StatusKind::Ordinary
                            },
                            title: if selected {
                                "Selected".into()
                            } else if entry.runnable {
                                "Runnable here".into()
                            } else {
                                "Not runnable here".into()
                            },
                            detail: if entry.runnable {
                                "Ready to explore in your browser".into()
                            } else {
                                "Needs another Host implementation".into()
                            },
                        },
                        vec![],
                    ),
                    node(
                        &format!("form-details-{index}"),
                        PresentationMechanism::Disclosure {
                            summary: "Form details".into(),
                        },
                        vec![
                            node(
                                &format!("form-requirements-{index}"),
                                PresentationMechanism::Status {
                                    kind: StatusKind::Ordinary,
                                    title: "Current capabilities".into(),
                                    detail: entry.realization.clone(),
                                },
                                vec![],
                            ),
                            node(
                                &format!("form-id-{index}"),
                                PresentationMechanism::CodeBlock {
                                    language: "checked-form-id".into(),
                                    code: entry.checked_form_id.clone(),
                                },
                                vec![],
                            ),
                        ],
                    ),
                    node(
                        &format!("form-actions-{index}"),
                        PresentationMechanism::ActionGroup {
                            label: format!("{} actions", entry.title),
                        },
                        vec![
                            gallery_action(index, "open", "Open in laboratory"),
                            gallery_action(index, "inspect", "Inspect Patchbay"),
                            node(
                                &format!("form-add-{index}"),
                                PresentationMechanism::Link {
                                    label: "Use in your Body".into(),
                                    destination: entry.handoff.clone(),
                                },
                                vec![],
                            ),
                        ],
                    ),
                ],
            ));
        }

        let status = if over_capacity {
            "Search is outside the admitted 128-byte bound.".into()
        } else {
            format!(
                "{} reviewed {}",
                cards.len(),
                if cards.len() == 1 { "Form" } else { "Forms" }
            )
        };
        Ok(SemanticApplicationView {
            revision: self.revision,
            root: node(
                "gallery",
                PresentationMechanism::Workbench,
                vec![
                    node(
                        "gallery-heading",
                        PresentationMechanism::Heading {
                            text: "Form Gallery".into(),
                        },
                        vec![],
                    ),
                    node(
                        "gallery-introduction",
                        PresentationMechanism::Status {
                            kind: StatusKind::Ordinary,
                            title: "Small programs. Real possibilities.".into(),
                            detail:
                                "Eight little compositions to run, understand, and make your own."
                                    .into(),
                        },
                        vec![],
                    ),
                    node(
                        "gallery-search",
                        PresentationMechanism::FormField(FormField {
                            label: "Search reviewed Forms".into(),
                            help: "Find a title, an idea, or a kind of Gear.".into(),
                            error: over_capacity
                                .then(|| "Search is outside the admitted bound.".into()),
                            value: self.query.clone(),
                            value_capacity: 512,
                            input_action: SemanticAction {
                                identity: "gallery.search".into(),
                                event: ApplicationEventKind::Input,
                                label: "Search reviewed Forms".into(),
                                availability: ActionAvailability::Available,
                            },
                            kind: FieldKind::Text,
                        }),
                        vec![],
                    ),
                    node(
                        "gallery-status",
                        PresentationMechanism::Status {
                            kind: if over_capacity {
                                StatusKind::Warning
                            } else {
                                StatusKind::Ordinary
                            },
                            title: status,
                            detail: "Choose something that sparks your curiosity.".into(),
                        },
                        vec![],
                    ),
                    node("gallery-cards", PresentationMechanism::Grid, cards),
                ],
            ),
        })
    }
}

fn gallery_action(index: usize, operation: &str, label: &str) -> SemanticPresentationNode {
    node(
        &format!("form-{operation}-{index}"),
        PresentationMechanism::Action(SemanticAction {
            identity: format!("gallery.{operation}.{index}"),
            event: ApplicationEventKind::Activate,
            label: label.into(),
            availability: ActionAvailability::Available,
        }),
        vec![],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_presentation::ApplicationComponent;

    fn state(query: &str) -> TourGalleryState {
        TourGalleryState {
            revision: 7,
            query: query.into(),
            selected_checked_form_id: Some("checked/memory".into()),
            entries: vec![TourGalleryEntry {
                title: "Memory Lantern".into(),
                category: "A first small program".into(),
                description: "Give a thought a place to glow.".into(),
                checked_form_id: "checked/memory".into(),
                realization: "presentation/text=current/local".into(),
                search_terms: "presentation/text".into(),
                runnable: true,
                handoff: "/conduit/creche/?form=memory_lantern&checked_form_id=checked%2Fmemory"
                    .into(),
            }],
        }
    }

    #[test]
    fn gallery_is_lowered_entirely_through_shared_semantics() {
        let lowered = state("").presentation().unwrap().lower().unwrap();
        assert!(
            lowered
                .nodes
                .iter()
                .any(|node| node.component == ApplicationComponent::FormField)
        );
        assert!(
            lowered
                .nodes
                .iter()
                .any(|node| node.component == ApplicationComponent::Link)
        );
        assert!(
            lowered
                .nodes
                .iter()
                .any(|node| node.component == ApplicationComponent::Grid)
        );
        assert_eq!(lowered.actions.len(), 3);
        assert!(lowered.encode().is_ok());
    }

    #[test]
    fn over_bound_search_is_an_explicit_warning_with_no_cards() {
        let lowered = state(&"x".repeat(GALLERY_SEARCH_BYTES + 1))
            .presentation()
            .unwrap()
            .lower()
            .unwrap();
        assert!(
            lowered
                .nodes
                .iter()
                .any(|node| node.component == ApplicationComponent::WarningStatus)
        );
        assert!(!lowered.nodes.iter().any(|node| node.key == "form-0"));
    }
}
