//! Portable presentation of reviewed Forms beside the authoritative Body workset.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_body::{MAX_BODY_FORMS, ResidentForm};
use conduit_presentation::{
    ActionAvailability, ApplicationEventKind, FieldKind, FormField, PresentationMechanism,
    SemanticAction, SemanticApplicationView, SemanticPresentationNode, StatusKind,
};

use crate::WorkspaceBody;

pub const LIBRARY_SEARCH_BYTES: usize = 128;

#[derive(Clone, Debug)]
pub struct LibraryEntry {
    pub form: ResidentForm,
    pub title: String,
    pub search_text: String,
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
        if entries.len() > MAX_BODY_FORMS
            || entries.iter().enumerate().any(|(index, entry)| {
                entry.title.is_empty()
                    || entry.title.len() > 256
                    || entry.search_text.len() > 2_048
                    || entry.form.source_document_id.as_str().is_empty()
                    || entry.form.checked_form_id.as_str().is_empty()
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
        for (index, entry) in self.entries.iter().enumerate() {
            let text = format!("{} {}", entry.title, entry.search_text).to_lowercase();
            if !terms.iter().all(|term| text.contains(term)) {
                continue;
            }
            let installed = body.evidence().body.workset.contains(&entry.form);
            let mut actions = vec![node(
                &format!("use-{index}"),
                PresentationMechanism::Action(action(
                    &format!("library.use.{index}"),
                    "Use",
                    ApplicationEventKind::Activate,
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
                    )),
                    vec![],
                ));
            }
            cards.push(node(
                &format!("library-form-{index}"),
                PresentationMechanism::Panel {
                    title: entry.title.clone(),
                },
                vec![
                    node(
                        &format!("installed-{index}"),
                        PresentationMechanism::Status {
                            kind: StatusKind::Ordinary,
                            title: if installed {
                                "In your Body"
                            } else {
                                "Not in your Body"
                            }
                            .into(),
                            detail: String::new(),
                        },
                        vec![],
                    ),
                    node(
                        &format!("actions-{index}"),
                        PresentationMechanism::ActionGroup {
                            label: format!("{} actions", entry.title),
                        },
                        actions,
                    ),
                ],
            ));
        }
        let count = cards.len();
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
                            help: "Use a Form here, or remove one from your Body.".into(),
                            error: None,
                            value: query.into(),
                            value_capacity: LIBRARY_SEARCH_BYTES as u32,
                            input_action: action(
                                "library.search",
                                "Find a Form",
                                ApplicationEventKind::Input,
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
                            title: if count == 0 {
                                "No Forms match your search.".into()
                            } else {
                                format!("{count} Forms")
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
fn action(id: &str, label: &str, event: ApplicationEventKind) -> SemanticAction {
    SemanticAction {
        identity: id.into(),
        label: label.into(),
        event,
        availability: ActionAvailability::Available,
    }
}
