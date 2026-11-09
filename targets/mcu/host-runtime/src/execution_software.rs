//! Standard prepared software Backs shared by MCU Hosts. Source selects these
//! implementations; constructors create no offer, admission, authority or Play.
extern crate alloc;
use alloc::boxed::Box;
use conduit_environment_runtime::{
    observation::{ObservationBack, ObservationContract},
    publication::{PublicationBack, PublicationContract, PublicationSequence, PublicationSnapshot},
    publication_face_custody::{CustodyRefusal, Publisher},
    PreparedCaptureSelector,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId, ValueRef,
};
use core::cell::RefCell;

mod publication_target_sealed {
    pub trait Sealed {}
    impl Sealed for core::cell::RefCell<Option<super::Publisher<'_>>> {}
}
/// A borrowed view of the original affine publisher slot. Sealing prevents a
/// substitute implementation; erasing only its inner borrow lifetime avoids
/// tying the slot's lifetime to the longer-lived Face it owns a loan from.
pub trait PublicationTarget: publication_target_sealed::Sealed {
    fn publish(&self, snapshot: PublicationSnapshot<'_>, now_ms: u64)
        -> Result<(), CustodyRefusal>;
}
impl PublicationTarget for RefCell<Option<Publisher<'_>>> {
    fn publish(
        &self,
        snapshot: PublicationSnapshot<'_>,
        now_ms: u64,
    ) -> Result<(), CustodyRefusal> {
        let mut slot = self.try_borrow_mut().map_err(|_| CustodyRefusal::Busy)?;
        slot.as_mut()
            .ok_or(CustodyRefusal::Unbound)?
            .publish(snapshot, now_ms)
    }
}

/// Physical monotonic reading supplied by the Host, never a browser timestamp.
pub type ReadMonotonicMillis = fn() -> u64;
/// Scope selected from the actual admitted assignment before Back construction,
/// never from an input value or a descriptive candidate projection.
#[derive(Clone, Copy)]
pub struct RecordingLogScope<'a> {
    pub context: conduit_environment_runtime::recording::LogContext<'a>,
    pub first_sequence: u64,
}
/// This enum contains no parser, sensor decoder, packet encoder or scheduler.
/// Resident expressions remain separately compiled, exactly selected owners.
pub enum SoftwareBack<'a> {
    Tick(conduit_time::TickBack),
    Trigger(conduit_time::TickTriggerBack),
    Selector(Box<SelectorBack>),
    Observation(Box<ObservationBack>),
    RecordingLog(Box<conduit_environment_runtime::recording::RecordingLogBack>),
    Publication(Box<PublicationFaceBack<'a>>),
}
impl<'a> SoftwareBack<'a> {
    /// The wait value is already retained in the selected kernel's value store.
    pub fn tick(wait: ValueRef, count: u64) -> Result<Self, &'static str> {
        conduit_time::TickBack::counted(wait, count)
            .map(Self::Tick)
            .ok_or("tick-count")
    }
    pub fn trigger() -> Self {
        Self::Trigger(conduit_time::TickTriggerBack::default())
    }
    pub fn selector(
        selector: conduit_core::StructuredSelector,
        input_bytes: usize,
        output_bytes: usize,
    ) -> Result<Self, &'static str> {
        Ok(Self::Selector(Box::new(SelectorBack::prepare(
            selector,
            input_bytes,
            output_bytes,
        )?)))
    }
    pub fn selector_static(
        canonical: &'static [u8],
        input_bytes: usize,
        output_bytes: usize,
    ) -> Result<Self, &'static str> {
        Ok(Self::Selector(Box::new(SelectorBack::prepare_static(
            canonical,
            input_bytes,
            output_bytes,
        )?)))
    }
    pub fn dht11_observation() -> Result<Self, &'static str> {
        let contract = ObservationContract::prepare().map_err(|_| "observation-contract")?;
        Ok(Self::Observation(Box::new(
            ObservationBack::prepare(&contract).map_err(|_| "observation-codec")?,
        )))
    }
    pub fn recording_log_unbound(
        maximum_samples: u32,
        now: ReadMonotonicMillis,
    ) -> Result<Self, &'static str> {
        let contract = ObservationContract::prepare().map_err(|_| "recording-contract")?;
        Ok(Self::RecordingLog(Box::new(
            conduit_environment_runtime::recording::RecordingLogBack::prepare_unbound(
                &contract,
                maximum_samples,
                now,
            )
            .map_err(|_| "recording-codec")?,
        )))
    }
    pub(crate) fn needs_recording_scope(&self) -> bool {
        matches!(self,Self::RecordingLog(back) if !back.is_bound())
    }
    pub(crate) fn prepare_recording_scope(
        &mut self,
        scope: RecordingLogScope<'_>,
    ) -> Result<(), &'static str> {
        let Self::RecordingLog(back) = self else {
            return Err("recording-owner");
        };
        if back.is_bound() {
            back.validate_scope(scope.context, scope.first_sequence)
                .map_err(|_| "recording-scope-mismatch")
        } else {
            back.bind_scope(scope.context, scope.first_sequence)
                .map_err(|_| "recording-scope")
        }
    }
    pub fn recording_log(
        scope: RecordingLogScope<'_>,
        maximum_samples: u32,
        now: ReadMonotonicMillis,
    ) -> Result<Self, &'static str> {
        let contract = ObservationContract::prepare().map_err(|_| "recording-contract")?;
        Ok(Self::RecordingLog(Box::new(
            conduit_environment_runtime::recording::RecordingLogBack::prepare(
                scope.context,
                &contract,
                scope.first_sequence,
                maximum_samples,
                now,
            )
            .map_err(|_| "recording-codec")?,
        )))
    }
    /// All outcome channels of this producer share one actual sequence owner
    /// and one exclusively retained Face publication custody. No Face is invented.
    pub fn publication(
        contract: &PublicationContract,
        maximum_events: u32,
        sequence: PublicationSequence,
        publisher: &'a dyn PublicationTarget,
        now: ReadMonotonicMillis,
    ) -> Result<Self, &'static str> {
        Ok(Self::Publication(Box::new(PublicationFaceBack {
            back: PublicationBack::prepare(contract, maximum_events, sequence)
                .map_err(|_| "publication")?,
            publisher,
            now,
            last_sequence: 0,
            failed: false,
        })))
    }
}
impl<const P: usize> StepBack<P> for SoftwareBack<'_> {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        match self {
            Self::Tick(back) => back.step(io, input),
            Self::Trigger(back) => back.step(io, input),
            Self::Selector(back) => back.step(io, input),
            Self::Observation(back) => back.step(io, input),
            Self::RecordingLog(back) => back.step(io, input),
            Self::Publication(back) => back.step(io, input),
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Selector(back) => <SelectorBack as StepBack<P>>::prepared_output(back, port),
            Self::Observation(back) => {
                <ObservationBack as StepBack<P>>::prepared_output(back, port)
            }
            Self::RecordingLog(back) => <conduit_environment_runtime::recording::RecordingLogBack as StepBack<P>>::prepared_output(back,port),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Tick(back) => <conduit_time::TickBack as StepBack<P>>::step_committed(back),
            Self::Trigger(back) => {
                <conduit_time::TickTriggerBack as StepBack<P>>::step_committed(back)
            }
            Self::Selector(back) => <SelectorBack as StepBack<P>>::step_committed(back),
            Self::Observation(back) => <ObservationBack as StepBack<P>>::step_committed(back),
            Self::RecordingLog(back) => <conduit_environment_runtime::recording::RecordingLogBack as StepBack<P>>::step_committed(back),
            Self::Publication(back) => <PublicationFaceBack as StepBack<P>>::step_committed(back),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Tick(back) => <conduit_time::TickBack as StepBack<P>>::cancel(back),
            Self::Trigger(back) => <conduit_time::TickTriggerBack as StepBack<P>>::cancel(back),
            Self::Selector(back) => <SelectorBack as StepBack<P>>::cancel(back),
            Self::Observation(back) => <ObservationBack as StepBack<P>>::cancel(back),
            Self::RecordingLog(back) => {
                <conduit_environment_runtime::recording::RecordingLogBack as StepBack<P>>::cancel(
                    back,
                )
            }
            Self::Publication(back) => <PublicationFaceBack as StepBack<P>>::cancel(back),
        }
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
/// Pure checked selection runs as an ordinary StepBack, not a hardware Host Call.
pub struct SelectorBack {
    selector: PreparedCaptureSelector,
    maximum: usize,
    staged: bool,
}
impl SelectorBack {
    pub fn prepare(
        selector: conduit_core::StructuredSelector,
        input: usize,
        output: usize,
    ) -> Result<Self, &'static str> {
        if input == 0
            || output == 0
            || input > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || output > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err("selector-bound");
        }
        Ok(Self {
            selector: PreparedCaptureSelector::prepare(selector, input, output)
                .map_err(|_| "selector")?,
            maximum: output,
            staged: false,
        })
    }
    pub fn prepare_static(
        canonical: &'static [u8],
        input: usize,
        output: usize,
    ) -> Result<Self, &'static str> {
        if input == 0
            || output == 0
            || input > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || output > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err("selector-bound");
        }
        Ok(Self {
            selector: PreparedCaptureSelector::prepare_static(canonical, input, output)
                .map_err(|_| "selector")?,
            maximum: output,
            staged: false,
        })
    }
}
impl<const P: usize> StepBack<P> for SelectorBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        self.staged = false;
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(1);
        }
        let Some(reference) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                if io.consume_closed(PortId(0)).is_ok() {
                    StepOutcome::Complete
                } else {
                    fail(2)
                }
            } else {
                StepOutcome::Await
            };
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(bytes) = input.input(PortId(0)) else {
            return fail(3);
        };
        if bytes.len() != reference.byte_len as usize || io.consume_fuel(1).is_err() {
            return fail(4);
        }
        match self.selector.select(bytes) {
            Ok(Some(value)) if value.len() <= self.maximum => {
                if io.send_prepared(PortId(0), value.len() as u32).is_err() {
                    return fail(5);
                }
                self.staged = true;
            }
            Ok(None) => {}
            _ => return fail(6),
        }
        if io.consume(PortId(0)).is_err() {
            return fail(7);
        }
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then_some(self.selector.output())
    }
    fn step_committed(&mut self) {
        self.staged = false;
    }
    fn cancel(&mut self) {
        self.staged = false;
    }
}
pub struct PublicationFaceBack<'a> {
    back: PublicationBack,
    publisher: &'a dyn PublicationTarget,
    now: ReadMonotonicMillis,
    last_sequence: u64,
    failed: bool,
}
impl<const P: usize> StepBack<P> for PublicationFaceBack<'_> {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        if self.failed {
            return fail(20);
        }
        self.back.step(io, input)
    }
    fn step_committed(&mut self) {
        <PublicationBack as StepBack<P>>::step_committed(&mut self.back);
        let Some(snapshot) = self.back.snapshot() else {
            return;
        };
        if snapshot.sequence == self.last_sequence {
            return;
        }
        self.last_sequence = snapshot.sequence;
        self.failed = self.publisher.publish(snapshot, (self.now)()).is_err();
    }
    fn cancel(&mut self) {
        <PublicationBack as StepBack<P>>::cancel(&mut self.back);
    }
}
