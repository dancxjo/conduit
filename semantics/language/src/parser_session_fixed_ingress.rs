//! Fixed canonical Session ingress with complete generated Native admission.
//! The Session preparation owner validates original checked Source and Plan
//! correlation before assembling ports; histories retain complete original frames.
use crate::{
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_execution::{verification::PreparedSourceVerification, ParserSessionEntry},
};
use alloc::{rc::Rc, vec::Vec};
use conduit_core::Plan;
use conduit_plot::rust_binding::{
    NativeBindingRefusal, NativeFamilyTypeDescriptor, PreparedNativeFamily,
    PreparedNativeRustBinding,
};
use core::cell::RefCell;

/// A target retains its original sealed Plan throughout execution. Metadata is
/// admitted by Session preparation; whole canonical responses are checked again.
pub trait ParserSessionExecutor: ParserCanonicalSourceExecutor {
    fn original_plan(&self) -> &Plan;
}
type Readmit = fn(&mut PreparedNativeFamily, &[u8]) -> Result<(), NativeBindingRefusal>;
fn readmit<T: PreparedNativeRustBinding>(
    family: &mut PreparedNativeFamily,
    bytes: &[u8],
) -> Result<(), NativeBindingRefusal> {
    drop(family.decode::<T>(bytes)?);
    Ok(())
}
pub(crate) struct ParserFixedFrames {
    input: Vec<u8>,
    output: Vec<u8>,
}
impl ParserFixedFrames {
    pub(crate) fn prepare(
        input: usize,
        output: usize,
    ) -> Result<Self, FixedRefusal<core::convert::Infallible>> {
        if input == 0
            || output == 0
            || input > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || output > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err(FixedRefusal::Pressure);
        }
        let mut input_buffer = Vec::new();
        let mut output_buffer = Vec::new();
        input_buffer
            .try_reserve_exact(input)
            .map_err(|_| FixedRefusal::Pressure)?;
        output_buffer
            .try_reserve_exact(output)
            .map_err(|_| FixedRefusal::Pressure)?;
        output_buffer.resize(output, 0);
        Ok(Self {
            input: input_buffer,
            output: output_buffer,
        })
    }
    pub(crate) fn into_candidate_buffer(self) -> Vec<u8> {
        self.input
    }
    pub(crate) fn retained_bytes(&self) -> Option<usize> {
        self.input.capacity().checked_add(self.output.capacity())
    }
}
pub(crate) struct ParserFixedHistory {
    pub(crate) entry: ParserSessionEntry,
    pub(crate) ordinal: u64,
    pub(crate) input: Vec<u8>,
    pub(crate) output: Vec<u8>,
    pub(crate) original_plan: Rc<Plan>,
    input_descriptor: &'static NativeFamilyTypeDescriptor,
    output_descriptor: &'static NativeFamilyTypeDescriptor,
    input_readmit: Readmit,
    output_readmit: Readmit,
}
#[derive(Debug)]
pub(crate) enum FixedRefusal<E> {
    Cancelled,
    Descriptor,
    Entry,
    Plan,
    Pressure,
    Source,
    DifferentOutput,
    Native(NativeBindingRefusal),
    Target(E),
}
pub(crate) struct PreparedParserFixedIngress<E: ParserSessionExecutor> {
    executor: E,
    entry: ParserSessionEntry,
    original_plan: Rc<Plan>,
    family: Rc<RefCell<PreparedNativeFamily>>,
    verifier: PreparedSourceVerification,
    input_descriptor: &'static NativeFamilyTypeDescriptor,
    output_descriptor: &'static NativeFamilyTypeDescriptor,
    input_readmit: Readmit,
    output_readmit: Readmit,
    maximum_invocations: u32,
    next_ordinal: u64,
    cancelled: bool,
}
impl<E: ParserSessionExecutor> PreparedParserFixedIngress<E> {
    /// Whole Source/Native-law parity and the original Plan seal/correlation are
    /// admitted by the fixed Session constructor, never by caller-selected Types.
    pub(crate) fn from_prepared<I: PreparedNativeRustBinding, O: PreparedNativeRustBinding>(
        executor: E,
        entry: ParserSessionEntry,
        original_plan: Rc<Plan>,
        family: Rc<RefCell<PreparedNativeFamily>>,
        verifier: PreparedSourceVerification,
        source_input_type: &conduit_core::StructuredInfoType,
        source_output_type: &conduit_core::StructuredInfoType,
        maximum_port_encoding_requested_bytes: usize,
        maximum_invocations: u32,
    ) -> Result<Self, FixedRefusal<E::Error>> {
        if maximum_invocations == 0 {
            return Err(FixedRefusal::Pressure);
        }
        if !family.borrow().contains_descriptor(I::PREPARED_DESCRIPTOR)
            || !family.borrow().contains_descriptor(O::PREPARED_DESCRIPTOR)
        {
            return Err(FixedRefusal::Descriptor);
        }
        if verifier.entry() != entry
            || executor.entry() != entry.name()
            || executor.input_type_bytes() != I::PREPARED_DESCRIPTOR.type_bytes
            || executor.output_type_bytes() != O::PREPARED_DESCRIPTOR.type_bytes
        {
            return Err(FixedRefusal::Entry);
        }
        // Both complete Type encoding requests are admitted before allocating
        // the first buffer. Core reserves the exact allocation-free length.
        let input_length = source_input_type
            .canonical_byte_length()
            .map_err(|_| FixedRefusal::Entry)?;
        let output_length = source_output_type
            .canonical_byte_length()
            .map_err(|_| FixedRefusal::Entry)?;
        if input_length
            .checked_add(output_length)
            .is_none_or(|sum| sum > maximum_port_encoding_requested_bytes)
        {
            return Err(FixedRefusal::Pressure);
        }
        if source_input_type
            .canonical_bytes()
            .map_err(|_| FixedRefusal::Entry)?
            != I::PREPARED_DESCRIPTOR.type_bytes
            || source_output_type
                .canonical_bytes()
                .map_err(|_| FixedRefusal::Entry)?
                != O::PREPARED_DESCRIPTOR.type_bytes
        {
            return Err(FixedRefusal::Entry);
        }
        if executor.original_plan() != original_plan.as_ref() {
            return Err(FixedRefusal::Plan);
        }
        Ok(Self {
            executor,
            entry,
            original_plan,
            family,
            verifier,
            input_descriptor: I::PREPARED_DESCRIPTOR,
            output_descriptor: O::PREPARED_DESCRIPTOR,
            input_readmit: readmit::<I>,
            output_readmit: readmit::<O>,
            maximum_invocations,
            next_ordinal: 0,
            cancelled: false,
        })
    }
    pub(crate) fn entry(&self) -> ParserSessionEntry {
        self.entry
    }
    pub(crate) fn cancel(&mut self) {
        self.cancelled = true;
        self.executor.cancel();
    }
    pub(crate) fn execute(
        &mut self,
        input: &[u8],
        mut frames: ParserFixedFrames,
    ) -> Result<ParserFixedHistory, FixedRefusal<E::Error>> {
        if self.cancelled {
            return Err(FixedRefusal::Cancelled);
        }
        if self.next_ordinal >= u64::from(self.maximum_invocations)
            || input.len() > frames.input.capacity()
            || frames.output.is_empty()
        {
            return Err(FixedRefusal::Pressure);
        }
        if self.executor.original_plan() != self.original_plan.as_ref() {
            return Err(FixedRefusal::Plan);
        }
        (self.input_readmit)(&mut self.family.borrow_mut(), input).map_err(FixedRefusal::Native)?;
        frames.input.extend_from_slice(input);
        let expected = self
            .verifier
            .evaluate(&frames.input)
            .map_err(|_| FixedRefusal::Source)?;
        if expected.len() > frames.output.len() {
            return Err(FixedRefusal::Pressure);
        }
        (self.output_readmit)(&mut self.family.borrow_mut(), expected)
            .map_err(FixedRefusal::Native)?;
        self.cancelled = true;
        struct Consumed<'a, E: ParserSessionExecutor> {
            target: &'a mut E,
            published: bool,
        }
        impl<E: ParserSessionExecutor> Drop for Consumed<'_, E> {
            fn drop(&mut self) {
                if !self.published {
                    self.target.cancel();
                }
            }
        }
        let mut consumed = Consumed {
            target: &mut self.executor,
            published: false,
        };
        let length = consumed
            .target
            .transact(self.next_ordinal, &frames.input, &mut frames.output)
            .map_err(FixedRefusal::Target)?;
        if length > frames.output.len() {
            return Err(FixedRefusal::Pressure);
        }
        frames.output.truncate(length);
        if frames.output != expected {
            return Err(FixedRefusal::DifferentOutput);
        }
        let history = ParserFixedHistory {
            entry: self.entry,
            ordinal: self.next_ordinal,
            input: frames.input,
            output: frames.output,
            original_plan: self.original_plan.clone(),
            input_descriptor: self.input_descriptor,
            output_descriptor: self.output_descriptor,
            input_readmit: self.input_readmit,
            output_readmit: self.output_readmit,
        };
        consumed.published = true;
        drop(consumed);
        self.next_ordinal += 1;
        self.cancelled = false;
        Ok(history)
    }
}
impl<E: ParserSessionExecutor> Drop for PreparedParserFixedIngress<E> {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl ParserFixedHistory {
    /// Sequential generated readmission holds at most one decoded Native frame.
    /// Source evaluator, retained frames and original Plan are separate owners.
    pub(crate) fn replay(
        &self,
        verifier: &mut PreparedSourceVerification,
        family: &mut PreparedNativeFamily,
        expected_original_plan: &Plan,
        maximum_live_native_bytes: usize,
    ) -> Result<(), FixedRefusal<core::convert::Infallible>> {
        if family.storage_receipt().conversion_requested_bytes_bound > maximum_live_native_bytes {
            return Err(FixedRefusal::Pressure);
        }
        if verifier.entry() != self.entry {
            return Err(FixedRefusal::Entry);
        }
        if self.original_plan.as_ref() != expected_original_plan {
            return Err(FixedRefusal::Plan);
        }
        if !family.contains_descriptor(self.input_descriptor)
            || !family.contains_descriptor(self.output_descriptor)
        {
            return Err(FixedRefusal::Descriptor);
        }
        (self.input_readmit)(family, &self.input).map_err(FixedRefusal::Native)?;
        if verifier
            .evaluate(&self.input)
            .map_err(|_| FixedRefusal::Source)?
            != self.output
        {
            return Err(FixedRefusal::DifferentOutput);
        }
        (self.output_readmit)(family, &self.output).map_err(FixedRefusal::Native)
    }
}
