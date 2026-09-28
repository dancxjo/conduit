use alloc::{format, string::String, vec, vec::Vec};

use conduit_presentation::{
    ActionAvailability, ApplicationEventKind, PresentationMechanism, SemanticAction,
    SemanticApplicationView, SemanticPresentationNode, SemanticPresentationRefusal, StatusKind,
};

pub const PREVIOUS_PAGE_ACTION: &str = "tour.previous";
pub const NEXT_PAGE_ACTION: &str = "tour.next";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TourPageNavigation {
    pub revision: u32,
    pub current_page: u32,
    pub page_count: u32,
}

impl TourPageNavigation {
    pub fn presentation(&self) -> Result<SemanticApplicationView, SemanticPresentationRefusal> {
        if self.page_count == 0 || self.current_page >= self.page_count {
            return Err(SemanticPresentationRefusal::InvalidNavigation);
        }
        let view = SemanticApplicationView {
            revision: self.revision,
            root: node(
                "navigation",
                PresentationMechanism::Navigation {
                    label: "Tour pages".into(),
                    current: if self.current_page > 0 {
                        "previous".into()
                    } else {
                        "next".into()
                    },
                },
                vec![
                    node(
                        "progress",
                        PresentationMechanism::Status {
                            kind: StatusKind::Ordinary,
                            title: format!("Page {} of {}", self.current_page + 1, self.page_count),
                            detail: String::new(),
                        },
                        vec![],
                    ),
                    action(
                        "previous",
                        PREVIOUS_PAGE_ACTION,
                        "Previous",
                        if self.current_page > 0 {
                            ActionAvailability::Available
                        } else {
                            ActionAvailability::Unavailable {
                                detail: "Already at the first page".into(),
                            }
                        },
                    ),
                    action(
                        "next",
                        NEXT_PAGE_ACTION,
                        "Next",
                        if self.current_page + 1 < self.page_count {
                            ActionAvailability::Available
                        } else {
                            ActionAvailability::Unavailable {
                                detail: "Already at the last page".into(),
                            }
                        },
                    ),
                ],
            ),
        };
        view.lower()?;
        Ok(view)
    }
}

fn action(
    key: &str,
    identity: &str,
    label: &str,
    availability: ActionAvailability,
) -> SemanticPresentationNode {
    node(
        key,
        PresentationMechanism::Action(SemanticAction {
            identity: identity.into(),
            event: ApplicationEventKind::Activate,
            label: label.into(),
            availability,
        }),
        vec![],
    )
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
    use conduit_presentation::{ApplicationComponent, ApplicationView};

    use super::*;

    #[test]
    fn navigation_is_finite_semantic_presentation_with_boundary_actions() {
        let lowered = TourPageNavigation {
            revision: 7,
            current_page: 0,
            page_count: 3,
        }
        .presentation()
        .unwrap()
        .lower()
        .unwrap();
        assert_eq!(
            ApplicationView::decode(&lowered.encode().unwrap()),
            Ok(lowered.clone())
        );
        assert!(lowered.nodes.iter().any(|node| {
            node.key == "navigation" && node.component == ApplicationComponent::Navigation
        }));
        assert!(
            lowered
                .nodes
                .iter()
                .any(|node| { node.key == "previous" && node.action.is_none() })
        );
        assert!(
            lowered
                .nodes
                .iter()
                .any(|node| node.key == "next" && node.action.is_some())
        );
    }

    #[test]
    fn navigation_refuses_empty_and_out_of_range_pages() {
        for navigation in [
            TourPageNavigation {
                revision: 1,
                current_page: 0,
                page_count: 0,
            },
            TourPageNavigation {
                revision: 1,
                current_page: 2,
                page_count: 2,
            },
        ] {
            assert_eq!(
                navigation.presentation(),
                Err(SemanticPresentationRefusal::InvalidNavigation)
            );
        }
    }
}
