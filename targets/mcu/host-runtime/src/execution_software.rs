//! Standard prepared software Backs shared by MCU Hosts. Source selects these
//! implementations; constructors create no offer, admission, authority or Play.
use conduit_dht11_observation::{ObservationBack, ObservationContract};
use conduit_semantic_catalog::PreparedCanonicalSelector;
extern crate alloc;
use alloc::boxed::Box;
use conduit_environment_runtime::{
    publication::{PublicationBack, PublicationContract, PublicationSequence, PublicationSnapshot},
    publication_face_custody::{CustodyRefusal, Publisher},
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
    LinkedSource(Box<conduit_typed_retention::PreparedLinkedSource<'static>>),
    SeededState(Box<conduit_composite::SeededStateBack>),
    Zip(Box<conduit_composite::FlowZipBack>),
    Merge(Box<conduit_composite::FlowMergeFiniteBack>),
    Tick(conduit_time::TickBack),
    Trigger(conduit_time::TickTriggerBack),
    Selector(Box<SelectorBack>),
    Observation(Box<ObservationBack>),
    RecordingLog(Box<conduit_environment_runtime::recording::RecordingLogBack>),
    Publication(Box<PublicationFaceBack<'a>>),
}
impl<'a> SoftwareBack<'a> {
    /// Borrow immutable checked material from the installed image. Root still
    /// selects the exact Kind/identity and funds this owner before Start.
    pub fn linked_source_static(
        schema: &'static [u8],
        value: &'static [u8],
        maximum_bytes: usize,
    ) -> Result<Self, &'static str> {
        conduit_typed_retention::PreparedLinkedSource::prepare(schema, value, maximum_bytes)
            .map(|back| Self::LinkedSource(Box::new(back)))
            .map_err(|_| "linked-source-material")
    }

    pub fn seeded_until(
        contract: &conduit_core::CheckedValueContract,
        schema: &conduit_core::StructuredInfoType,
    ) -> Result<Self, &'static str> {
        conduit_composite::SeededStateBack::prepare_until(contract, schema)
            .map(|back| Self::SeededState(Box::new(back)))
    }
    pub fn seeded_until_static(
        contract: &conduit_core::CheckedValueContract,
        schema: &conduit_core::StructuredInfoType,
        canonical: &'static [u8],
    ) -> Result<Self, &'static str> {
        conduit_composite::SeededStateBack::prepare_until_static(contract, schema, canonical)
            .map(|back| Self::SeededState(Box::new(back)))
    }
    pub fn zip(
        left: &conduit_core::CheckedValueContract,
        left_type: conduit_core::StructuredInfoType,
        right: &conduit_core::CheckedValueContract,
        right_type: conduit_core::StructuredInfoType,
        feedback: bool,
    ) -> Result<Self, &'static str> {
        let back = if feedback {
            conduit_composite::FlowZipBack::prepare_typed_feedback(
                left, left_type, right, right_type,
            )
        } else {
            conduit_composite::FlowZipBack::prepare_typed_finite(left, left_type, right, right_type)
        }
        .map_err(|_| "zip-definition")?;
        Ok(Self::Zip(Box::new(back)))
    }
    pub fn merge(
        schema: &conduit_core::StructuredInfoType,
        maximum_bytes: u32,
    ) -> Result<Self, &'static str> {
        conduit_composite::FlowMergeFiniteBack::prepare(schema, maximum_bytes)
            .map(|back| Self::Merge(Box::new(back)))
            .map_err(|_| "merge-definition")
    }

    pub fn merge_static(
        schema: &conduit_core::StructuredInfoType,
        canonical: &'static [u8],
        maximum_bytes: u32,
    ) -> Result<Self, &'static str> {
        conduit_composite::FlowMergeFiniteBack::prepare_static(schema, canonical, maximum_bytes)
            .map(|back| Self::Merge(Box::new(back)))
            .map_err(|_| "merge-definition")
    }

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
#[path = "execution_software_step.rs"]
mod step;

fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
/// Pure checked selection runs as an ordinary StepBack, not a hardware Host Call.
pub struct SelectorBack {
    selector: PreparedCanonicalSelector,
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
            selector: PreparedCanonicalSelector::prepare(selector, input, output)
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
            selector: PreparedCanonicalSelector::prepare_static(canonical, input, output)
                .map_err(|_| "selector")?,
            maximum: output,
            staged: false,
        })
    }
}
impl<const P: usize> StepBack<P> for SelectorBack {
    #[cfg(feature = "schema-value-storage")]
    fn canonical_input_schema(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0))
            .then(|| self.selector.input_schema_bytes())
            .flatten()
    }
    #[cfg(feature = "schema-value-storage")]
    fn canonical_output_schema(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0))
            .then(|| self.selector.output_schema_bytes())
            .flatten()
    }

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
        let canonical = input.input(PortId(0));
        #[cfg(feature = "schema-value-storage")]
        let parts = input.canonical_parts(PortId(0));
        #[cfg(not(feature = "schema-value-storage"))]
        let parts: Option<conduit_kernel::CanonicalValueParts<'_>> = None;
        let length = match (canonical, parts) {
            (Some(bytes), None) => Some(bytes.len()),
            (None, Some(parts)) => parts.schema.len().checked_add(parts.node.len()),
            _ => None,
        };
        if length != Some(reference.byte_len as usize) || io.consume_fuel(1).is_err() {
            return fail(4);
        }
        let selected = match (canonical, parts) {
            (Some(bytes), None) => self.selector.select(bytes),
            (None, Some(parts)) => self.selector.select_parts(parts.schema, parts.node),
            _ => return fail(3),
        };
        match selected {
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

#[cfg(all(test, feature = "schema-value-storage"))]
#[path = "execution_selector_schema_tests.rs"]
mod selector_schema_tests;

#[path = "execution_software_schema.rs"]
mod schema;
