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

pub const PRESENTATION_TO_REQUEST_CALL: HostCallId = HostCallId(0);
pub const REGISTER_GENERATED_MANIFESTATION_CALL: HostCallId = HostCallId(0);
pub const ACKNOWLEDGE_ARTIFACT_AND_BUILD_SHOW_CALL: HostCallId = HostCallId(1);

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
