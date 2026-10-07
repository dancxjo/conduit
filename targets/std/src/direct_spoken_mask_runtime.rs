//! Bounded opening speech and artifact Show for one direct Mask.
//!
//! The complete Face is read after this Show by `SpokenFaceSession`, one
//! acknowledged speech batch per Play. The opening artifact never claims that
//! all Face clauses have already been spoken.

use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};
use conduit_presentation::{
    DirectArtifactAcknowledgedSpokenShow, ManifestationLifecycle, MaskShow, PlannedMaskPlot,
    Presentation, SpokenMaskArtifactReceipt,
};

const MAX_OPENING_BYTES: usize = 256;
const MAX_FACE_BYTES: u32 = conduit_presentation::MAX_GENERATIVE_PRESENTER_INPUT_BYTES as u32;

pub struct DirectSpokenMaskPreparation {
    pub presentation: Presentation,
    pub planned_mask: PlannedMaskPlot,
    pub front_subject: String,
    pub target_subject: String,
    pub prepared_sign: conduit_core::SignId,
    pub available_sign: conduit_core::SignId,
    encoded_face: Vec<u8>,
    wording: std::collections::VecDeque<Vec<u8>>,
}

pub struct DirectSpokenMaskSession {
    prepared: DirectSpokenMaskPreparation,
    active_play: conduit_core::ActivePlayIdentity,
    wording: std::collections::VecDeque<Vec<u8>>,
    face_registered: bool,
}

impl DirectSpokenMaskPreparation {
    pub fn encoded_face(&self) -> &[u8] {
        &self.encoded_face
    }

    /// Exact brief wording admitted for this Mask Play. A complete Face
    /// reading is a separate sequence sourced from the acknowledged Show.
    pub fn opening_wording(&self) -> Result<&str, String> {
        let [item] = self.wording.as_slices().0 else {
            return Err("direct Face opening is not one bounded item".into());
        };
        std::str::from_utf8(item).map_err(|_| "direct Face opening is not UTF-8".into())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        presentation: Presentation,
        planned_mask: PlannedMaskPlot,
        front_subject: String,
        target_subject: String,
        prepared_sign: conduit_core::SignId,
        available_sign: conduit_core::SignId,
    ) -> Result<Self, String> {
        let encoded_face = serde_json::to_vec(&presentation)
            .map_err(|error| format!("encode direct Face: {error}"))?;
        if encoded_face.len() > MAX_FACE_BYTES as usize {
            return Err("direct spoken Mask Face exceeds admitted bound".into());
        }
        let wording = prepare_wording_items(&presentation)?;
        Ok(Self {
            presentation,
            planned_mask,
            front_subject,
            target_subject,
            prepared_sign,
            available_sign,
            encoded_face,
            wording,
        })
    }

    pub fn prepare_session(
        mut self,
        active_play: conduit_core::ActivePlayIdentity,
    ) -> Result<DirectSpokenMaskSession, String> {
        if active_play.plan_id != self.planned_mask.plan.plan_id
            || self.prepared_sign == self.available_sign
        {
            return Err("direct spoken Mask preparation identities do not correlate".into());
        }
        let wording = std::mem::take(&mut self.wording);
        Ok(DirectSpokenMaskSession {
            wording,
            prepared: self,
            active_play,
            face_registered: false,
        })
    }
}

pub fn prepare_wording_items(
    presentation: &Presentation,
) -> Result<std::collections::VecDeque<Vec<u8>>, String> {
    // Check that the same complete Face can enter the interactive reader. Its
    // clauses are deliberately not flattened into this one 30-second Play.
    crate::spoken_face_mask::mechanical_face_clauses(presentation)
        .map_err(|error| format!("project direct Face speech: {error:?}"))?;
    let name = presentation
        .subjects
        .iter()
        .find(|subject| subject.role == conduit_presentation::PresentationRole::Body)
        .map(|subject| subject.name.as_str())
        .filter(|name| !name.is_empty() && name.len() <= 128);
    let opening = match name {
        Some(name) => format!("Current view of {name}. This opening is brief; the complete reading needs a selected speaker."),
        None => "Current view. This opening is brief; the complete reading needs a selected speaker.".to_owned(),
    };
    if opening.len() > MAX_OPENING_BYTES {
        return Err("direct Face opening exceeds admitted bound".into());
    }
    Ok(std::collections::VecDeque::from([opening.into_bytes()]))
}

impl DirectSpokenMaskSession {
    fn validate_face(&self, encoded: &[u8]) -> Result<(), String> {
        if encoded.len() > MAX_FACE_BYTES as usize {
            return Err("direct spoken Mask Face exceeds admitted bound".into());
        }
        if encoded != self.prepared.encoded_face {
            return Err("direct spoken Mask received stale Face".into());
        }
        Ok(())
    }

    pub fn next_wording(&mut self, encoded_face: &[u8]) -> Result<Option<Vec<u8>>, String> {
        self.validate_face(encoded_face)?;
        Ok(self.wording.pop_front())
    }

    pub fn register_face(&mut self, encoded_face: &[u8]) -> Result<(), String> {
        self.validate_face(encoded_face)?;
        if self.face_registered {
            return Err("direct spoken Mask registered Face twice".into());
        }
        self.face_registered = true;
        Ok(())
    }

    pub fn acknowledge_artifact_and_build_show(&self, encoded: &[u8]) -> Result<Vec<u8>, String> {
        if !self.face_registered || !self.wording.is_empty() || encoded.len() > 4_096 {
            return Err("direct spoken artifact preceded complete Face wording".into());
        }
        let artifact: SpokenMaskArtifactReceipt = serde_json::from_slice(encoded)
            .map_err(|error| format!("decode direct spoken artifact receipt: {error}"))?;
        let prepared = MaskShow::prepared(
            &self.prepared.planned_mask,
            &self.prepared.presentation,
            self.active_play.clone(),
            self.prepared.front_subject.clone(),
            self.prepared.target_subject.clone(),
            self.prepared.prepared_sign.clone(),
        )
        .map_err(|error| format!("prepare direct spoken Show: {error:?}"))?;
        let available = prepared
            .transition(
                ManifestationLifecycle::Available,
                self.prepared.available_sign.clone(),
            )
            .map_err(|error| format!("make direct spoken Show available: {error:?}"))?;
        let shown = DirectArtifactAcknowledgedSpokenShow {
            show: available,
            artifact,
        };
        shown
            .validate(&self.prepared.presentation)
            .map_err(|error| format!("invalid direct spoken Show: {error:?}"))?;
        serde_json::to_vec(&shown).map_err(|error| format!("encode direct spoken Show: {error}"))
    }
}

/// A retained Face value drives one bounded Host Call per wording item. The
/// output is a closing Flow and obeys downstream pressure before advancing.
pub struct DirectFaceWordingBack {
    face: Option<ValueRef>,
    pending: Option<RequestId>,
    next_request: u32,
    complete: bool,
}

impl DirectFaceWordingBack {
    pub fn new() -> Self {
        Self {
            face: None,
            pending: None,
            next_request: 0,
            complete: false,
        }
    }
}

impl Default for DirectFaceWordingBack {
    fn default() -> Self {
        Self::new()
    }
}

impl<const PORTS: usize> StepBack<PORTS> for DirectFaceWordingBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return invalid(1);
            }
            if let Some(failure) = outcome.failure {
                return StepOutcome::Fail(failure);
            }
            match (outcome.disposition, outcome.output) {
                (HostCallDisposition::Completed, Some(output)) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("direct wording completion");
                    io.send(PortId(0), output.value)
                        .expect("ready direct wording output");
                    self.pending = None;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Completed, None) => {
                    io.consume_host_completion()
                        .expect("direct wording closure");
                    // Clearing the retained Face makes the scheduler release
                    // the pending Host Call's sole owned reference at commit.
                    // Discarding it here would release that same value twice.
                    self.face = None;
                    self.pending = None;
                    self.complete = true;
                    return StepOutcome::Complete;
                }
                _ => return invalid(2),
            }
        }
        if self.face.is_none() {
            let Some(_) = io.input(PortId(0)) else {
                return StepOutcome::Await;
            };
            self.face = Some(io.take_input(PortId(0)).expect("retain direct Face"));
        }
        let Some(next) = self.next_request.checked_add(1) else {
            return invalid(3);
        };
        let input = match BoundedValueRef::new(self.face.expect("retained Face"), MAX_FACE_BYTES) {
            Ok(input) => input,
            Err(_) => return invalid(4),
        };
        let request = RequestId(self.next_request);
        io.request_host_call(request, HostCallId(0), input)
            .expect("direct Face wording Host Call");
        self.next_request = next;
        self.pending = Some(request);
        StepOutcome::Progress
    }

    fn retains_host_call_input(&self, _: RequestId, value: ValueRef) -> bool {
        self.face == Some(value)
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.complete = true;
    }
}

const fn invalid(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::Body;
    use conduit_core::{CheckedPlotId, SignId, SourceDocumentId};
    use conduit_presentation::{
        PresentationBasis, PresentationDisclosure, PresentationDisclosureLevel, PresentationRole,
        PresentationSubject, PresentationText,
    };

    #[test]
    fn pre_show_direct_wording_is_an_explicit_bounded_opening() {
        let body = Body::born(
            SourceDocumentId::from("source/direct-voice"),
            CheckedPlotId::from("checked/direct-voice"),
            1,
            SignId::from("sign/direct-voice/born"),
        )
        .unwrap();
        let face = Presentation::new_with_semantics(
            1,
            PresentationBasis {
                body_id: Some(body.body_id),
                wake_id: None,
                source_document_id: None,
                checked_plot_id: None,
                expanded_plot_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![],
            },
            vec![PresentationSubject {
                identity: "body/current".into(),
                role: PresentationRole::Body,
                name: "Current body".into(),
            }],
            vec![],
            vec![],
            vec![PresentationText {
                subject: "body/current".into(),
                text: "Ready to create a Body.".into(),
            }],
            vec![],
            vec![PresentationDisclosure {
                subject: "body/current".into(),
                level: PresentationDisclosureLevel::Primary,
            }],
        )
        .unwrap();
        let items = prepare_wording_items(&face).unwrap();
        assert_eq!(items.len(), 1);
        assert!(items
            .iter()
            .all(|item| !item.is_empty() && item.len() <= MAX_OPENING_BYTES));
        let spoken = items
            .iter()
            .map(|item| std::str::from_utf8(item).unwrap())
            .collect::<String>();
        assert!(spoken.contains("Current body"));
        assert!(spoken.contains("opening is brief"));
        assert!(!spoken.contains("Ready to create a Body."));
    }
}
