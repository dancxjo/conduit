//! Prepared canonical ingress for the production parser custody owner.
//!
//! Unlike the older typed Flow this boundary does not rebuild Native schema or
//! law trees through `into_structured`/`encode` during ingress. The target owns
//! ordinary Plan execution; returned bytes undergo exact fixed Source and
//! prepared-family admission before a whole execution receipt can be published.
use super::parser_session_execution::verification::{
    PreparedSourceVerification, VerificationRefusal,
};
use super::parser_session_execution::{
    ParserSessionEntry, ParserSessionExecution, ParserSessionVerificationLimits,
    ParserSessionVerificationReceipt,
};
use alloc::{rc::Rc, vec::Vec};
use conduit_plot::rust_binding::{
    NativeBindingRefusal, PreparedNativeFamily, PreparedNativeRustBinding,
};
use core::{cell::RefCell, marker::PhantomData};

/// An ordinary target executor whose complete input/output frames are prepared
/// before play. Implementations retain the exact checked Source and Plan.
/// This contract is not evidence that a particular target actually executed it.
pub trait ParserCanonicalSourceExecutor {
    type Error;
    fn entry(&self) -> &str;
    fn input_type_bytes(&self) -> &[u8];
    fn output_type_bytes(&self) -> &[u8];
    fn transact(
        &mut self,
        ordinal: u64,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, Self::Error>;
}
#[derive(Clone, Copy, Debug)]
pub struct ParserCanonicalIngressLimits {
    pub maximum_invocations: u32,
    pub maximum_input_bytes: usize,
    pub maximum_output_bytes: usize,
}
#[derive(Debug)]
pub enum ParserCanonicalIngressRefusal<E> {
    Limits,
    Entry,
    Ports,
    Verification,
    Storage,
    InputPressure,
    OutputPressure,
    InvocationPressure,
    Cancelled,
    Poisoned,
    DifferentOutput,
    Native(NativeBindingRefusal),
    Execution(E),
}
/// Both buffers are allocated before exposure of any consuming ingress.
/// Complete frames move into the immutable execution receipt, never a digest.
pub(crate) struct PreparedParserExecutionFrames {
    pub(super) input: Vec<u8>,
    pub(super) output: Vec<u8>,
}
impl PreparedParserExecutionFrames {
    pub(crate) fn prepare(input: usize, output: usize) -> Result<Self, ()> {
        if input == 0
            || output == 0
            || input > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || output > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err(());
        }
        let mut input_bytes = Vec::new();
        input_bytes.try_reserve_exact(input).map_err(|_| ())?;
        let mut output_bytes = Vec::new();
        output_bytes.try_reserve_exact(output).map_err(|_| ())?;
        output_bytes.resize(output, 0);
        Ok(Self {
            input: input_bytes,
            output: output_bytes,
        })
    }
    pub(crate) fn retained_capacity_bytes(&self) -> usize {
        self.input.capacity() + self.output.capacity()
    }
}
/// A family is prepared and charged once by the owning Session. Its conversion
/// bound must additionally be admitted for BOTH concurrently live Native input
/// and output values, alongside these canonical buffers and the target executor.
pub struct PreparedCanonicalParserSessionPort<I, O, E> {
    entry: ParserSessionEntry,
    executor: E,
    family: Rc<RefCell<PreparedNativeFamily>>,
    verifier: PreparedSourceVerification,
    verification_storage: ParserSessionVerificationReceipt,
    limits: ParserCanonicalIngressLimits,
    next_ordinal: u64,
    poisoned: bool,
    cancelled: bool,
    binding: PhantomData<fn(I) -> O>,
}
impl<
        I: PreparedNativeRustBinding,
        O: PreparedNativeRustBinding,
        E: ParserCanonicalSourceExecutor,
    > PreparedCanonicalParserSessionPort<I, O, E>
{
    pub fn prepare(
        entry: ParserSessionEntry,
        executor: E,
        family: Rc<RefCell<PreparedNativeFamily>>,
        limits: ParserCanonicalIngressLimits,
        verification: ParserSessionVerificationLimits,
    ) -> Result<Self, ParserCanonicalIngressRefusal<E::Error>> {
        use ParserCanonicalIngressRefusal as R;
        if limits.maximum_invocations == 0
            || limits.maximum_input_bytes == 0
            || limits.maximum_output_bytes == 0
            || limits.maximum_input_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || limits.maximum_output_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err(R::Limits);
        }
        if executor.entry() != entry.name() {
            return Err(R::Entry);
        }
        if !family.borrow().contains_descriptor(I::PREPARED_DESCRIPTOR)
            || !family.borrow().contains_descriptor(O::PREPARED_DESCRIPTOR)
        {
            return Err(R::Ports);
        }

        let (verifier, input, output, verification_storage) =
            PreparedSourceVerification::prepare(entry, verification).map_err(|e| match e {
                VerificationRefusal::Program => R::Verification,
                VerificationRefusal::Ports => R::Ports,
                VerificationRefusal::Storage(_) => R::Storage,
            })?;
        let input_bytes = input.canonical_bytes().map_err(|_| R::Ports)?;
        let output_bytes = output.canonical_bytes().map_err(|_| R::Ports)?;
        if input_bytes != I::PREPARED_DESCRIPTOR.type_bytes
            || output_bytes != O::PREPARED_DESCRIPTOR.type_bytes
            || executor.input_type_bytes() != input_bytes
            || executor.output_type_bytes() != output_bytes
        {
            return Err(R::Ports);
        }
        Ok(Self {
            entry,
            executor,
            family,
            verifier,
            verification_storage,
            limits,
            next_ordinal: 0,
            poisoned: false,
            cancelled: false,
            binding: PhantomData,
        })
    }
    pub fn verification_storage(&self) -> ParserSessionVerificationReceipt {
        self.verification_storage
    }
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }
    pub fn next_ordinal(&self) -> u64 {
        self.next_ordinal
    }
    /// Only the Session registry may supply one of its prepared reserved frames.
    pub(crate) fn execute(
        &mut self,
        input: &[u8],
        mut frames: PreparedParserExecutionFrames,
    ) -> Result<ParserSessionExecution<I, O>, ParserCanonicalIngressRefusal<E::Error>> {
        use ParserCanonicalIngressRefusal as R;
        if self.cancelled {
            return Err(R::Cancelled);
        }
        if self.poisoned {
            return Err(R::Poisoned);
        }
        if self.next_ordinal >= u64::from(self.limits.maximum_invocations) {
            return Err(R::InvocationPressure);
        }
        if input.len() > self.limits.maximum_input_bytes
            || frames.input.capacity() < self.limits.maximum_input_bytes
        {
            return Err(R::InputPressure);
        }
        if frames.output.len() < self.limits.maximum_output_bytes {
            return Err(R::OutputPressure);
        }
        let next = self
            .next_ordinal
            .checked_add(1)
            .ok_or(R::InvocationPressure)?;
        // Every allocation requested by this generated conversion is bounded by
        // the family's independently admitted conversion receipt.
        let native_input = self
            .family
            .borrow_mut()
            .decode::<I>(input)
            .map_err(R::Native)?;
        frames.input.clear();
        frames.input.extend_from_slice(input);
        // Admit the complete Source result and its generated Native conversion
        // before the target can consume input. This value is only provisional:
        // no receipt exists until the target returns exactly these whole bytes.
        let expected = self
            .verifier
            .evaluate(&frames.input)
            .map_err(|_| R::Verification)?;
        if expected.len() > self.limits.maximum_output_bytes {
            return Err(R::OutputPressure);
        }
        let native_output = self
            .family
            .borrow_mut()
            .decode::<O>(expected)
            .map_err(R::Native)?;
        self.poisoned = true;
        let result = (|| {
            let length = self
                .executor
                .transact(
                    self.next_ordinal,
                    &frames.input,
                    &mut frames.output[..self.limits.maximum_output_bytes],
                )
                .map_err(R::Execution)?;
            if length > self.limits.maximum_output_bytes {
                return Err(R::OutputPressure);
            }
            frames.output.truncate(length);
            if expected != frames.output {
                return Err(R::DifferentOutput);
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                let execution = ParserSessionExecution::from_verified_canonical(
                    self.entry,
                    self.next_ordinal,
                    native_input,
                    native_output,
                    frames.input,
                    frames.output,
                );
                self.next_ordinal = next;
                self.poisoned = false;
                Ok(execution)
            }
            Err(error) => {
                self.cancelled = true;
                Err(error)
            }
        }
    }
}
