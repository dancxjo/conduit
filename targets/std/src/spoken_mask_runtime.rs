//! Kernel Steps for the semantic edges unique to a generative spoken Mask.
//!
//! Large structured transformations cross admitted Host Calls. The Steps only
//! retain bounded value references and never turn an attempted effect into a
//! Show. A Show is emitted only after the artifact receipt has been accepted by
//! the exact host-side session.

use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};
use conduit_presentation::{
    ArtifactAcknowledgedSpokenShow, GeneratedManifestation, GenerativePresenterRequest,
    ManifestationLifecycle, MaskShow, PlannedMaskForm, Presentation, SpokenMaskArtifactReceipt,
};

pub const PRESENTATION_TO_REQUEST_CALL: HostCallId = HostCallId(0);
pub const GENERATED_MANIFESTATION_TO_SPEECH_CALL: HostCallId = HostCallId(0);
pub const REGISTER_GENERATED_MANIFESTATION_CALL: HostCallId = HostCallId(0);
pub const ACKNOWLEDGE_ARTIFACT_AND_BUILD_SHOW_CALL: HostCallId = HostCallId(1);

/// Prepared host-side semantic adapter for one exact spoken Mask Play.
///
/// Provider, TTS, and artifact effects remain outside this adapter. It accepts
/// only their bounded semantic results and refuses to mint an Available Show
/// until the exact artifact receipt has been validated.
pub struct SpokenMaskSemanticSession {
    request: GenerativePresenterRequest,
    presentation: Presentation,
    planned_mask: PlannedMaskForm,
    active_play: conduit_core::ActivePlayIdentity,
    front_subject: String,
    target_subject: String,
    prepared_sign: conduit_core::SignId,
    available_sign: conduit_core::SignId,
    generated: Option<GeneratedManifestation>,
}

#[derive(Clone)]
pub struct SpokenMaskPreparation {
    pub request: GenerativePresenterRequest,
    pub presentation: Presentation,
    pub planned_mask: PlannedMaskForm,
    pub front_subject: String,
    pub target_subject: String,
    pub prepared_sign: conduit_core::SignId,
    pub available_sign: conduit_core::SignId,
}

impl SpokenMaskPreparation {
    pub fn prepare_session(
        &self,
        active_play: conduit_core::ActivePlayIdentity,
    ) -> Result<SpokenMaskSemanticSession, String> {
        SpokenMaskSemanticSession::prepare(
            self.request.clone(),
            self.presentation.clone(),
            self.planned_mask.clone(),
            active_play,
            self.front_subject.clone(),
            self.target_subject.clone(),
            self.prepared_sign.clone(),
            self.available_sign.clone(),
        )
    }
}

impl SpokenMaskSemanticSession {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        request: GenerativePresenterRequest,
        presentation: Presentation,
        planned_mask: PlannedMaskForm,
        active_play: conduit_core::ActivePlayIdentity,
        front_subject: String,
        target_subject: String,
        prepared_sign: conduit_core::SignId,
        available_sign: conduit_core::SignId,
    ) -> Result<Self, String> {
        request
            .validate()
            .map_err(|error| format!("invalid spoken Mask request: {error:?}"))?;
        if request.semantic_data.presentation != presentation
            || active_play.plan_id != planned_mask.plan.plan_id
            || prepared_sign == available_sign
        {
            return Err("spoken Mask preparation identities do not correlate".into());
        }
        Ok(Self {
            request,
            presentation,
            planned_mask,
            active_play,
            front_subject,
            target_subject,
            prepared_sign,
            available_sign,
            generated: None,
        })
    }

    pub fn adapt_presentation(&self, encoded: &[u8]) -> Result<Vec<u8>, String> {
        if encoded.len() > self.request.bounds.maximum_input_bytes as usize {
            return Err("spoken Mask Presentation exceeds its admitted bound".into());
        }
        let presentation: Presentation = serde_json::from_slice(encoded)
            .map_err(|error| format!("decode spoken Mask Presentation: {error}"))?;
        if presentation != self.presentation {
            return Err("spoken Mask received a stale Presentation".into());
        }
        let encoded = serde_json::to_vec(&self.request)
            .map_err(|error| format!("encode generative Presenter request: {error}"))?;
        if encoded.len() > conduit_presentation::MAX_GENERATIVE_PRESENTER_INPUT_BYTES {
            return Err("generative Presenter request exceeds its portable bound".into());
        }
        Ok(encoded)
    }

    pub fn register_generated_manifestation(&mut self, encoded: &[u8]) -> Result<(), String> {
        if encoded.len() > self.request.bounds.maximum_output_bytes as usize {
            return Err("generated manifestation exceeds its admitted bound".into());
        }
        let generated: GeneratedManifestation = serde_json::from_slice(encoded)
            .map_err(|error| format!("decode generated manifestation: {error}"))?;
        self.request
            .validate_manifestation(&generated)
            .map_err(|error| format!("invalid generated manifestation: {error:?}"))?;
        if self.generated.is_some() {
            return Err("spoken Mask already registered one generated manifestation".into());
        }
        self.generated = Some(generated);
        Ok(())
    }

    pub fn validate_and_extract_speech(&self, encoded: &[u8]) -> Result<Vec<u8>, String> {
        if encoded.len() > self.request.bounds.maximum_output_bytes as usize {
            return Err("generated manifestation exceeds its admitted bound".into());
        }
        let generated: GeneratedManifestation = serde_json::from_slice(encoded)
            .map_err(|error| format!("decode generated manifestation: {error}"))?;
        self.request
            .validate_manifestation(&generated)
            .map_err(|error| format!("invalid generated manifestation: {error:?}"))?;
        let mut speech_segments = generated
            .content
            .iter()
            .filter(|segment| segment.role == conduit_presentation::GeneratedContentRole::Speech);
        let speech = speech_segments
            .next()
            .ok_or_else(|| "generated manifestation has no outward Speech".to_string())?;
        if speech_segments.next().is_some() {
            return Err("generated manifestation has more than one outward Speech".into());
        }
        core::str::from_utf8(&speech.bytes)
            .map_err(|_| "generated outward Speech is not UTF-8".to_string())?;
        if speech.bytes.len() > conduit_tongues::MAXIMUM_TEXT_BYTES as usize {
            return Err("generated outward Speech exceeds the TTS Fore bound".into());
        }
        Ok(speech.bytes.clone())
    }

    pub fn acknowledge_artifact_and_build_show(&self, encoded: &[u8]) -> Result<Vec<u8>, String> {
        if encoded.len() > 4_096 {
            return Err("spoken Mask artifact receipt exceeds its admitted bound".into());
        }
        let artifact: SpokenMaskArtifactReceipt = serde_json::from_slice(encoded)
            .map_err(|error| format!("decode spoken Mask artifact receipt: {error}"))?;
        let generated = self
            .generated
            .as_ref()
            .ok_or_else(|| "spoken Mask artifact preceded generated manifestation".to_string())?;
        let prepared = MaskShow::prepared(
            &self.planned_mask,
            &self.presentation,
            self.active_play.clone(),
            self.front_subject.clone(),
            self.target_subject.clone(),
            self.prepared_sign.clone(),
        )
        .map_err(|error| format!("prepare spoken Show: {error:?}"))?;
        let available = prepared
            .transition(
                ManifestationLifecycle::Available,
                self.available_sign.clone(),
            )
            .map_err(|error| format!("make spoken Show available: {error:?}"))?;
        let result = ArtifactAcknowledgedSpokenShow {
            show: available,
            generated_manifestation_identity: generated.manifestation_identity.clone(),
            artifact,
        };
        result
            .validate(&self.presentation, generated)
            .map_err(|error| format!("invalid artifact-acknowledged spoken Show: {error:?}"))?;
        serde_json::to_vec(&result).map_err(|error| format!("encode spoken Show: {error}"))
    }
}

pub struct PresentationToGenerativeRequestBack {
    pending_input: Option<ValueRef>,
    pending: bool,
    complete: bool,
    maximum_presentation_bytes: u32,
}

impl PresentationToGenerativeRequestBack {
    pub fn new(maximum_presentation_bytes: u32) -> Result<Self, &'static str> {
        if maximum_presentation_bytes == 0
            || maximum_presentation_bytes
                > conduit_presentation::MAX_GENERATIVE_PRESENTER_INPUT_BYTES as u32
        {
            return Err("invalid Presentation adapter bound");
        }
        Ok(Self {
            pending_input: None,
            pending: false,
            complete: false,
            maximum_presentation_bytes,
        })
    }
}

impl<const PORTS: usize> StepBack<PORTS> for PresentationToGenerativeRequestBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(0) {
                return fail(1);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed request adapter");
                    io.send(PortId(0), output.value)
                        .expect("ready request adapter output");
                    if let Some(input) = self.pending_input.take() {
                        io.discard(input)
                            .expect("release Presentation adapter input");
                    }
                    self.pending = false;
                    self.complete = true;
                    StepOutcome::Complete
                }
                (_, _, Some(failure)) => StepOutcome::Fail(failure),
                _ => fail(2),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending {
                return fail(3);
            }
            let Ok(input) = BoundedValueRef::new(value, self.maximum_presentation_bytes) else {
                return fail(4);
            };
            self.pending_input = Some(io.take_input(PortId(0)).expect("present Presentation"));
            io.request_host_call(RequestId(0), PRESENTATION_TO_REQUEST_CALL, input)
                .expect("Presentation adapter Host Call");
            self.pending = true;
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }

    fn retains_host_call_input(&self, request: RequestId, value: ValueRef) -> bool {
        request == RequestId(0) && self.pending_input == Some(value)
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.complete = true;
    }
}

pub struct GeneratedManifestationToSpeechBack {
    inner: PresentationToGenerativeRequestBack,
}

impl GeneratedManifestationToSpeechBack {
    pub fn new(maximum_manifestation_bytes: u32) -> Result<Self, &'static str> {
        if maximum_manifestation_bytes == 0
            || maximum_manifestation_bytes
                > conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32
        {
            return Err("invalid generated manifestation adapter bound");
        }
        Ok(Self {
            inner: PresentationToGenerativeRequestBack::new(maximum_manifestation_bytes)?,
        })
    }
}

impl<const PORTS: usize> StepBack<PORTS> for GeneratedManifestationToSpeechBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.inner.step(io, inputs)
    }

    fn retains_host_call_input(&self, request: RequestId, value: ValueRef) -> bool {
        <PresentationToGenerativeRequestBack as StepBack<PORTS>>::retains_host_call_input(
            &self.inner,
            request,
            value,
        )
    }

    fn cancel(&mut self) {
        <PresentationToGenerativeRequestBack as StepBack<PORTS>>::cancel(&mut self.inner);
    }
}

enum ShowPhase {
    Manifestation,
    Artifact,
    Complete,
}

pub struct ArtifactAcknowledgedShowBack {
    phase: ShowPhase,
    pending_input: Option<ValueRef>,
    pending_request: Option<RequestId>,
    maximum_manifestation_bytes: u32,
    maximum_receipt_bytes: u32,
}

impl ArtifactAcknowledgedShowBack {
    pub fn new(
        maximum_manifestation_bytes: u32,
        maximum_receipt_bytes: u32,
    ) -> Result<Self, &'static str> {
        if maximum_manifestation_bytes == 0
            || maximum_manifestation_bytes
                > conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32
            || maximum_receipt_bytes == 0
            || maximum_receipt_bytes > 4_096
        {
            return Err("invalid spoken Show adapter bound");
        }
        Ok(Self {
            phase: ShowPhase::Manifestation,
            pending_input: None,
            pending_request: None,
            maximum_manifestation_bytes,
            maximum_receipt_bytes,
        })
    }

    fn begin<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        port: PortId,
        request: RequestId,
        call: HostCallId,
        maximum_bytes: u32,
    ) -> StepOutcome {
        let Some(value) = io.input(port) else {
            return StepOutcome::Await;
        };
        let Ok(input) = BoundedValueRef::new(value, maximum_bytes) else {
            return fail(10);
        };
        self.pending_input = Some(io.take_input(port).expect("present spoken Mask input"));
        io.request_host_call(request, call, input)
            .expect("spoken Mask semantic Host Call");
        self.pending_request = Some(request);
        StepOutcome::Progress
    }
}

impl<const PORTS: usize> StepBack<PORTS> for ArtifactAcknowledgedShowBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if matches!(self.phase, ShowPhase::Complete) {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending_request != Some(request) {
                return fail(11);
            }
            if let Some(failure) = outcome.failure {
                return StepOutcome::Fail(failure);
            }
            match (&self.phase, outcome.disposition, outcome.output) {
                (ShowPhase::Manifestation, HostCallDisposition::Completed, None) => {
                    io.consume_host_completion()
                        .expect("registered manifestation");
                    if let Some(input) = self.pending_input.take() {
                        io.discard(input).expect("release registered manifestation");
                    }
                    self.pending_request = None;
                    self.phase = ShowPhase::Artifact;
                    StepOutcome::Progress
                }
                (ShowPhase::Artifact, HostCallDisposition::Completed, Some(output)) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("acknowledged artifact Show");
                    io.send(PortId(0), output.value)
                        .expect("ready artifact-acknowledged Show");
                    if let Some(input) = self.pending_input.take() {
                        io.discard(input).expect("release artifact receipt");
                    }
                    self.pending_request = None;
                    self.phase = ShowPhase::Complete;
                    StepOutcome::Complete
                }
                _ => fail(12),
            }
        } else {
            match self.phase {
                ShowPhase::Manifestation => self.begin(
                    io,
                    PortId(0),
                    RequestId(0),
                    REGISTER_GENERATED_MANIFESTATION_CALL,
                    self.maximum_manifestation_bytes,
                ),
                ShowPhase::Artifact => self.begin(
                    io,
                    PortId(1),
                    RequestId(1),
                    ACKNOWLEDGE_ARTIFACT_AND_BUILD_SHOW_CALL,
                    self.maximum_receipt_bytes,
                ),
                ShowPhase::Complete => StepOutcome::Complete,
            }
        }
    }

    fn retains_host_call_input(&self, request: RequestId, value: ValueRef) -> bool {
        self.pending_request == Some(request) && self.pending_input == Some(value)
    }

    fn cancel(&mut self) {
        self.pending_request = None;
        self.phase = ShowPhase::Complete;
    }
}

/// A spoken Mask with no admitted input mechanism closes interaction honestly.
/// Completion closes the ordinary `FaceInteraction...|` output; it does not
/// fabricate an empty interaction value.
#[derive(Default)]
pub struct ClosingNoInteractionBack;

impl<const PORTS: usize> StepBack<PORTS> for ClosingNoInteractionBack {
    fn step(&mut self, _: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        StepOutcome::Complete
    }
}

const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}
