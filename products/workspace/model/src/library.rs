//! Portable presentation of reviewed Forms beside the authoritative Body workset.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_body::{MAX_BODY_FORMS, ResidentForm};
use conduit_presentation::{
    ActionAvailability, ApplicationEventKind, FieldKind, FormField, PresentationMechanism,
    SemanticAction, SemanticApplicationView, SemanticPresentationNode, StatusKind,
};

use crate::WorkspaceBody;

pub const LIBRARY_SEARCH_BYTES: usize = 128;
pub const MAX_LIBRARY_FORMS: usize = 107;
pub const MAX_LIBRARY_RESULTS: usize = 24;

#[derive(Clone, Debug)]
pub struct LibraryEntry {
    pub form: ResidentForm,
    pub title: String,
    pub search_text: String,
    pub availability: LibraryAvailability,
    pub graceful_fallback: Option<LibraryFallback>,
}

#[derive(Clone, Debug)]
pub struct LibraryFallback {
    pub title: String,
    pub availability: LibraryAvailability,
}

#[derive(Clone, Debug)]
pub enum LibraryAvailability {
    Available,
    NeedsCapability(String),
}

pub struct FormLibrary {
    entries: Vec<LibraryEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibraryRefusal {
    InvalidInventory,
    SearchBound,
    Presentation,
}

impl FormLibrary {
    pub fn new(entries: Vec<LibraryEntry>) -> Result<Self, LibraryRefusal> {
        if entries.len() > MAX_LIBRARY_FORMS
            || entries.iter().enumerate().any(|(index, entry)| {
                entry.title.is_empty()
                    || entry.title.len() > 256
                    || entry.search_text.len() > 2_048
                    || entry.form.source_document_id.as_str().is_empty()
                    || entry.form.checked_form_id.as_str().is_empty()
                    || matches!(
                        &entry.availability,
                        LibraryAvailability::NeedsCapability(reason)
                            if reason.is_empty() || reason.len() > 512
                    )
                    || entry.graceful_fallback.as_ref().is_some_and(|fallback| {
                        fallback.title.is_empty()
                            || fallback.title.len() > 256
                            || matches!(
                                &fallback.availability,
                                LibraryAvailability::NeedsCapability(reason)
                                    if reason.is_empty() || reason.len() > 512
                            )
                    })
                    || entries[..index]
                        .iter()
                        .any(|prior| prior.form == entry.form)
            })
        {
            return Err(LibraryRefusal::InvalidInventory);
        }
        Ok(Self { entries })
    }

    /// `revision` is presentation freshness, distinct from Body workload revision.
    /// Installed state is always projected from this Body, never retained here.
    pub fn presentation(
        &self,
        body: &WorkspaceBody,
        revision: u32,
        query: &str,
    ) -> Result<SemanticApplicationView, LibraryRefusal> {
        if query.len() > LIBRARY_SEARCH_BYTES {
            return Err(LibraryRefusal::SearchBound);
        }
        let normalized = query.to_lowercase();
        let terms: Vec<_> = normalized.split_whitespace().collect();
        let mut cards = Vec::new();
        let mut matches = 0_usize;
        for (index, entry) in self.entries.iter().enumerate() {
            let text = format!("{} {}", entry.title, entry.search_text).to_lowercase();
            let available = matches!(entry.availability, LibraryAvailability::Available);
            if (!terms.is_empty() && !terms.iter().all(|term| text.contains(term)))
                || (terms.is_empty() && !available)
            {
                continue;
            }
            matches += 1;
            if cards.len() == MAX_LIBRARY_RESULTS {
                continue;
            }
            let installed = body.evidence().body.workset.contains(&entry.form);
            let body_at_capacity = body.evidence().body.workset.len() >= MAX_BODY_FORMS;
            let use_available = available && (installed || !body_at_capacity);
            let unavailable_detail = if !available {
                "This Form needs capabilities this Host does not offer."
            } else {
                "This Body is at its resident Form capacity. Remove a Form before adding another."
            };
            let mut actions = vec![node(
                &format!("use-{index}"),
                PresentationMechanism::Action(action(
                    &format!("library.use.{index}"),
                    "Use",
                    ApplicationEventKind::Activate,
                    use_available,
                    unavailable_detail,
                )),
                vec![],
            )];
            if installed {
                actions.push(node(
                    &format!("remove-{index}"),
                    PresentationMechanism::Action(action(
                        &format!("library.remove.{index}"),
                        "Remove",
                        ApplicationEventKind::Activate,
                        true,
                        "",
                    )),
                    vec![],
                ));
            }
            let mut details = vec![node(
                &format!("installed-{index}"),
                PresentationMechanism::Status {
                    kind: StatusKind::Ordinary,
                    title: if installed {
                        "In your Body"
                    } else if !available {
                        "Needs capability"
                    } else if body_at_capacity {
                        "Body at capacity"
                    } else {
                        "Not in your Body"
                    }
                    .into(),
                    detail: match &entry.availability {
                        LibraryAvailability::Available => String::new(),
                        LibraryAvailability::NeedsCapability(reason) => reason.clone(),
                    },
                },
                vec![],
            )];
            if let Some(fallback) = &entry.graceful_fallback {
                details.push(node(
                    &format!("fallback-{index}"),
                    PresentationMechanism::Status {
                        kind: StatusKind::Ordinary,
                        title: format!("Text fallback: {}", fallback.title),
                        detail: match &fallback.availability {
                            LibraryAvailability::Available => {
                                "This fallback has a reviewed realization on the current Host."
                                    .into()
                            }
                            LibraryAvailability::NeedsCapability(reason) => {
                                format!("The fallback still needs capability: {reason}")
                            }
                        },
                    },
                    vec![],
                ));
            }
            details.push(node(
                &format!("actions-{index}"),
                PresentationMechanism::ActionGroup {
                    label: format!("{} actions", entry.title),
                },
                actions,
            ));
            cards.push(node(
                &format!("library-form-{index}"),
                PresentationMechanism::Panel {
                    title: entry.title.clone(),
                },
                details,
            ));
        }
        let shown = cards.len();
        let view = SemanticApplicationView {
            revision,
            root: node(
                "form-library",
                PresentationMechanism::Shell,
                vec![
                    node(
                        "library-search",
                        PresentationMechanism::FormField(FormField {
                            label: "Find a Form".into(),
                            help: "Search the reviewed catalog. Use a Form here, or remove one from your Body.".into(),
                            error: None,
                            value: query.into(),
                            value_capacity: LIBRARY_SEARCH_BYTES as u32,
                            input_action: action(
                                "library.search",
                                "Find a Form",
                                ApplicationEventKind::Input,
                                true,
                                "",
                            ),
                            kind: FieldKind::Text,
                        }),
                        vec![],
                    ),
                    node("library-forms", PresentationMechanism::Grid, cards),
                    node(
                        "library-count",
                        PresentationMechanism::Status {
                            kind: StatusKind::Ordinary,
                            title: if matches == 0 {
                                "No Forms match your search.".into()
                            } else if shown < matches {
                                format!("Showing {shown} of {matches} Forms. Refine your search.")
                            } else {
                                format!("{shown} Forms")
                            },
                            detail: String::new(),
                        },
                        vec![],
                    ),
                ],
            ),
        };
        view.lower().map_err(|_| LibraryRefusal::Presentation)?;
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
fn action(
    id: &str,
    label: &str,
    event: ApplicationEventKind,
    available: bool,
    unavailable_detail: &str,
) -> SemanticAction {
    SemanticAction {
        identity: id.into(),
        label: label.into(),
        event,
        availability: if available {
            ActionAvailability::Available
        } else {
            ActionAvailability::Unavailable {
                detail: unavailable_detail.into(),
            }
        },
    }
}
