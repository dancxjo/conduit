//! Exact ordinary Form-Port adapter for the retained Tour model.

use alloc::vec::Vec;
use conduit_presentation::{ApplicationEvent, ApplicationView, ApplicationViewRefusal};

use crate::{TourWorkspaceController, TourWorkspaceRefusal, TourWorkspaceRequest};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourPortRefusal {
    Event(ApplicationViewRefusal),
    Application(TourWorkspaceRefusal),
    Presentation,
}

#[derive(Debug, Eq, PartialEq)]
pub struct TourPortOutput {
    pub view: Vec<u8>,
    /// Authority-bearing requests remain outside retained presentation state.
    pub request: Option<TourWorkspaceRequest>,
}

pub struct TourApplicationPort {
    controller: TourWorkspaceController,
}

impl TourApplicationPort {
    pub fn canonical() -> Self {
        Self {
            controller: TourWorkspaceController::canonical(1),
        }
    }

    pub const fn controller(&self) -> &TourWorkspaceController {
        &self.controller
    }

    /// An empty input emits the admitted initial view. Later inputs are exact
    /// `application/event` encodings validated against the retained revision.
    pub fn apply(&mut self, encoded: &[u8]) -> Result<TourPortOutput, TourPortRefusal> {
        let current = self.view()?;
        let request = if encoded.is_empty() {
            None
        } else {
            let event =
                ApplicationEvent::decode(encoded, &current).map_err(TourPortRefusal::Event)?;
            Some(
                self.controller
                    .request(&event)
                    .map_err(TourPortRefusal::Application)?,
            )
        };
        Ok(TourPortOutput {
            view: self.view()?.encode().map_err(TourPortRefusal::Event)?,
            request,
        })
    }

    fn view(&self) -> Result<ApplicationView, TourPortRefusal> {
        self.controller
            .state()
            .presentation()
            .and_then(|view| view.lower())
            .map_err(|_| TourPortRefusal::Presentation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OPEN_PATCHBAY_ACTION_ID;
    use conduit_presentation::ApplicationEventKind;

    const FORM_SOURCE: &str = include_str!("../../../../forms/tour/main.conduit");

    #[test]
    fn tour_is_a_checked_host_neutral_form_over_the_application_seam() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        conduit_semantic_catalog::install_application_catalogs(&mut startup, &mut profile).unwrap();
        let syntax = conduit_form::parse_syntax_document(FORM_SOURCE);
        let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
        let expanded = conduit_form::expand_canonical_form(&checked, "tour", &profile).unwrap();
        assert_eq!(expanded.gears.len(), 3);
        assert_eq!(expanded.connections.len(), 2);
        assert!(!FORM_SOURCE.contains("Host"));
        assert!(!FORM_SOURCE.contains("implementation"));
    }

    #[test]
    fn initial_view_and_real_event_share_the_typed_port_encoding() {
        let mut port = TourApplicationPort::canonical();
        let initial = port.apply(&[]).unwrap();
        assert!(initial.request.is_none());
        let view = ApplicationView::decode(&initial.view).unwrap();
        let event = ApplicationEvent {
            revision: view.revision,
            action: OPEN_PATCHBAY_ACTION_ID.into(),
            kind: ApplicationEventKind::Activate,
            value: Vec::new(),
        };
        let output = port.apply(&event.encode(&view).unwrap()).unwrap();
        assert_eq!(output.request, Some(TourWorkspaceRequest::OpenPatchbay));
        assert_eq!(
            port.controller().state().phase,
            crate::TourWorkspacePhase::PatchbayOpen
        );
        assert_eq!(
            ApplicationView::decode(&output.view).unwrap().revision,
            view.revision + 1
        );
    }

    #[test]
    fn stale_and_oversized_events_do_not_mutate_retained_state() {
        let mut port = TourApplicationPort::canonical();
        let view = ApplicationView::decode(&port.apply(&[]).unwrap().view).unwrap();
        let stale = ApplicationEvent {
            revision: view.revision + 1,
            action: OPEN_PATCHBAY_ACTION_ID.into(),
            kind: ApplicationEventKind::Activate,
            value: Vec::new(),
        };
        assert_eq!(
            port.apply(
                &stale
                    .encode(&ApplicationView {
                        revision: stale.revision,
                        ..view.clone()
                    })
                    .unwrap()
            ),
            Err(TourPortRefusal::Event(
                ApplicationViewRefusal::StaleRevision
            ))
        );
        assert_eq!(port.controller().state().revision, view.revision);
    }
}
