//! Native composition of one validated attached Body workbench.
//!
//! This state owns no Body lifecycle truth. It retains one immutable accepted
//! attachment and derives native cursor state over Program and Body. “History”
//! remains the `Body / Signs` location supplied by the shared model.

use std::io::Read;

use conduit_presentation::{PresentationAspect, PresentationDepth, PresentationPlace};
use patchbay_model::{
    CurrentBodyFrame, PatchbayBodyApplicationEntrance, PatchbayBodyAttachment, ReadableBodyHistory,
    MAX_PATCHBAY_BODY_EVIDENCE_BYTES,
};

use crate::arguments::Arguments;

#[derive(Debug, Clone)]
pub(super) struct NativeBodyWorkbench {
    attachment: PatchbayBodyAttachment,
    frame: CurrentBodyFrame,
    history: ReadableBodyHistory,
    place: PresentationPlace,
    aspect: PresentationAspect,
    depth: PresentationDepth,
    history_focus: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum NativeWorkbenchError {
    Read(String),
    Attachment(patchbay_model::PatchbayBodyEntranceError),
    History(patchbay_model::ReadableBodyHistoryError),
    InvalidRevision,
    StaleRevision { current: u64, offered: u64 },
    InvalidDestination,
    LifecycleAuthorityUnavailable,
}

#[derive(Debug, Default)]
pub(super) struct NativeBodyWorkbenchSlot {
    last_revision: Option<u64>,
    current: Option<NativeBodyWorkbench>,
}

impl NativeBodyWorkbenchSlot {
    pub(super) fn open_arguments(arguments: &Arguments) -> Result<Self, String> {
        let Some(path) = &arguments.body_biography_path else {
            return Ok(Self::default());
        };
        let entrance = match (
            arguments.hosted_patchbay_plan_id.as_deref(),
            arguments.hosted_patchbay_implementation_id.as_deref(),
        ) {
            (Some(plan), Some(implementation)) => PatchbayBodyApplicationEntrance::Hosted {
                plan_id: conduit_core::PlanId::from(plan),
                implementation_id: conduit_core::ImplementationId::from(implementation),
            },
            (None, None) => PatchbayBodyApplicationEntrance::ExternalReader,
            _ => return Err("hosted Patchbay placement arguments are incomplete".into()),
        };
        let encoded = read_bounded(path).map_err(|error| format!("Body biography: {error:?}"))?;
        let mut slot = Self::default();
        slot.replace_serialized(1, &encoded, entrance)
            .map_err(|error| format!("Body biography: {error:?}"))?;
        Ok(slot)
    }

    pub(super) fn current(&self) -> Option<&NativeBodyWorkbench> {
        self.current.as_ref()
    }

    pub(super) fn current_mut(&mut self) -> Option<&mut NativeBodyWorkbench> {
        self.current.as_mut()
    }

    pub(super) fn replace_serialized(
        &mut self,
        revision: u64,
        encoded: &[u8],
        entrance: PatchbayBodyApplicationEntrance,
    ) -> Result<&NativeBodyWorkbench, NativeWorkbenchError> {
        if revision == 0 {
            self.current = None;
            return Err(NativeWorkbenchError::InvalidRevision);
        }
        if let Some(current) = self.last_revision {
            if revision <= current {
                self.current = None;
                return Err(NativeWorkbenchError::StaleRevision {
                    current,
                    offered: revision,
                });
            }
        }
        self.last_revision = Some(revision);
        self.current = None;
        self.current = Some(NativeBodyWorkbench::from_serialized(
            revision, encoded, entrance,
        )?);
        Ok(self.current.as_ref().expect("native workbench installed"))
    }

    pub(super) fn detach(&mut self) {
        self.current = None;
    }
}

impl NativeBodyWorkbench {
    pub(super) fn from_serialized(
        evidence_revision: u64,
        encoded: &[u8],
        entrance: PatchbayBodyApplicationEntrance,
    ) -> Result<Self, NativeWorkbenchError> {
        let attachment = PatchbayBodyAttachment::open_serialized(encoded, entrance)
            .map_err(NativeWorkbenchError::Attachment)?;
        Self::from_attachment(evidence_revision, attachment)
    }

    pub(super) fn from_attachment(
        evidence_revision: u64,
        attachment: PatchbayBodyAttachment,
    ) -> Result<Self, NativeWorkbenchError> {
        let frame = CurrentBodyFrame::from_attachment(evidence_revision, &attachment);
        let history = ReadableBodyHistory::from_attachment(evidence_revision, &attachment)
            .map_err(NativeWorkbenchError::History)?;
        if frame.body_id != history.body_id
            || frame.evidence_revision != history.evidence_revision
            || history.place != PresentationPlace::Body
            || history.aspect != PresentationAspect::Signs
        {
            return Err(NativeWorkbenchError::InvalidDestination);
        }
        Ok(Self {
            attachment,
            frame,
            history,
            place: PresentationPlace::Body,
            aspect: PresentationAspect::Structure,
            depth: PresentationDepth::Primary,
            history_focus: 0,
        })
    }

    pub(super) fn validate_form(
        &self,
        editor: Option<&patchbay_model::FormEditor>,
    ) -> Result<(), String> {
        let Some(editor) = editor else {
            return Ok(());
        };
        let view = editor.view();
        let source_matches = view.checked.source_document_id.as_ref()
            == Some(&self.frame.program.source_document_id);
        let checked_matches = view
            .checked
            .forms
            .iter()
            .find(|form| form.name == view.open_form)
            .is_some_and(|form| form.checked_form_id == self.frame.program.checked_form_id);
        if source_matches && checked_matches {
            Ok(())
        } else {
            Err("opened Form does not match the attached Body program identities".into())
        }
    }

    pub(super) fn frame(&self) -> &CurrentBodyFrame {
        &self.frame
    }

    pub(super) fn history(&self) -> &ReadableBodyHistory {
        &self.history
    }

    pub(super) fn place(&self) -> PresentationPlace {
        self.place
    }

    pub(super) fn aspect(&self) -> PresentationAspect {
        self.aspect
    }

    pub(super) fn depth(&self) -> PresentationDepth {
        self.depth
    }

    pub(super) fn is_program(&self) -> bool {
        self.place == PresentationPlace::Program
    }

    pub(super) fn is_history(&self) -> bool {
        self.place == PresentationPlace::Body && self.aspect == PresentationAspect::Signs
    }

    pub(super) fn show(
        &mut self,
        place: PresentationPlace,
        aspect: PresentationAspect,
    ) -> Result<(), NativeWorkbenchError> {
        if !matches!(
            (place, aspect),
            (PresentationPlace::Program, PresentationAspect::Structure)
                | (PresentationPlace::Body, PresentationAspect::Structure)
                | (PresentationPlace::Body, PresentationAspect::Signs)
        ) {
            return Err(NativeWorkbenchError::InvalidDestination);
        }
        self.place = place;
        self.aspect = aspect;
        self.depth = PresentationDepth::Primary;
        Ok(())
    }

    pub(super) fn cycle_destination(&mut self) {
        let destination = match (self.place, self.aspect) {
            (PresentationPlace::Program, _) => {
                (PresentationPlace::Body, PresentationAspect::Structure)
            }
            (PresentationPlace::Body, PresentationAspect::Structure) => {
                (PresentationPlace::Body, PresentationAspect::Signs)
            }
            _ => (PresentationPlace::Program, PresentationAspect::Structure),
        };
        self.show(destination.0, destination.1)
            .expect("native destinations are finite and valid");
    }

    pub(super) fn toggle_exact(&mut self) {
        self.depth = if self.depth == PresentationDepth::Exact {
            PresentationDepth::Primary
        } else {
            PresentationDepth::Exact
        };
    }

    pub(super) fn inspect_focused_history(&mut self) {
        if self.is_history() {
            self.depth = PresentationDepth::Detail;
        }
    }

    pub(super) fn move_history_focus(&mut self, forward: bool) {
        let count = self.history.entries.len();
        if count == 0 {
            self.history_focus = 0;
        } else if forward {
            self.history_focus = (self.history_focus + 1) % count;
        } else {
            self.history_focus = (self.history_focus + count - 1) % count;
        }
    }

    pub(super) fn history_focus(&self) -> usize {
        self.history_focus
    }

    pub(super) fn request_lifecycle_action(&self) -> Result<(), NativeWorkbenchError> {
        Err(NativeWorkbenchError::LifecycleAuthorityUnavailable)
    }

    pub(super) fn evidence(&self) -> &conduit_body::BodyBiographyEvidence {
        self.attachment.evidence()
    }
}

fn read_bounded(path: &std::path::Path) -> Result<Vec<u8>, NativeWorkbenchError> {
    let file =
        std::fs::File::open(path).map_err(|error| NativeWorkbenchError::Read(error.to_string()))?;
    let limit = u64::try_from(MAX_PATCHBAY_BODY_EVIDENCE_BYTES)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut encoded = Vec::with_capacity(MAX_PATCHBAY_BODY_EVIDENCE_BYTES.min(4 * 1_024));
    file.take(limit)
        .read_to_end(&mut encoded)
        .map_err(|error| NativeWorkbenchError::Read(error.to_string()))?;
    if encoded.len() > MAX_PATCHBAY_BODY_EVIDENCE_BYTES {
        return Err(NativeWorkbenchError::Attachment(
            patchbay_model::PatchbayBodyEntranceError::EvidenceTooLarge,
        ));
    }
    Ok(encoded)
}
