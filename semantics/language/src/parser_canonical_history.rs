//! Full historical Native frames with separately reserved typed readmission.
//! The complete original values are retained, never replaced by IDs or digests.
use crate::parser_session_execution::verification::PreparedSourceVerification;
use crate::parser_session_execution::{ParserSessionEntry, ParserSessionExecution};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{
    NativeBindingRefusal, PreparedNativeFamily, PreparedNativeRustBinding,
};
use core::marker::PhantomData;

/// All complete original canonical fields and the fixed Source entry witness.
/// Construction remains inside the Session's verified execution boundary.
pub struct ParserCanonicalHistory<I, O> {
    entry: ParserSessionEntry,
    ordinal: u64,
    input: Vec<u8>,
    output: Vec<u8>,
    source_custody: &'static str,
    source_programs: &'static str,
    binding: PhantomData<fn(I) -> O>,
}
impl<I, O> ParserCanonicalHistory<I, O> {
    pub fn entry(&self) -> ParserSessionEntry {
        self.entry
    }
    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }
    pub fn input_bytes(&self) -> &[u8] {
        &self.input
    }
    pub fn output_bytes(&self) -> &[u8] {
        &self.output
    }
    pub fn retained_frame_capacity_bytes(&self) -> Option<usize> {
        self.input.capacity().checked_add(self.output.capacity())
    }
    pub fn source_custody(&self) -> &'static str {
        self.source_custody
    }
    pub fn source_programs(&self) -> &'static str {
        self.source_programs
    }
    pub(crate) fn from_execution(execution: ParserSessionExecution<I, O>) -> (I, O, Self) {
        let source_custody = execution.source_custody();
        let source_programs = execution.source_programs();
        let (entry, ordinal, input, output, input_bytes, output_bytes) = execution.into_parts();
        (
            input,
            output,
            Self {
                entry,
                ordinal,
                input: input_bytes,
                output: output_bytes,
                source_custody,
                source_programs,
                binding: PhantomData,
            },
        )
    }
}
/// Exclusive admission for concurrently live historical Native conversion.
/// The family, stored frames and prepared Source evaluator are separate charges.
/// A view borrows this budget until both decoded Native values have been dropped.
pub struct ParserHistoricalReadmissionBudget {
    maximum_bytes: usize,
}
impl ParserHistoricalReadmissionBudget {
    pub fn new(maximum_bytes: usize) -> Option<Self> {
        (maximum_bytes != 0).then_some(Self { maximum_bytes })
    }
}
#[derive(Debug)]
pub enum ParserHistoricalReadmissionRefusal {
    Pressure,
    Entry,
    Descriptor,
    Source,
    DifferentOutput,
    Native(NativeBindingRefusal),
}
/// Fields drop before the exclusive budget borrow is released.
pub struct ParserHistoricalReadmission<'a, I, O> {
    input: I,
    output: O,
    _budget: &'a mut ParserHistoricalReadmissionBudget,
}
impl<I, O> ParserHistoricalReadmission<'_, I, O> {
    pub fn input(&self) -> &I {
        &self.input
    }
    pub fn output(&self) -> &O {
        &self.output
    }
}
impl<I: PreparedNativeRustBinding, O: PreparedNativeRustBinding> ParserCanonicalHistory<I, O> {
    /// The Session supplies its independently admitted evaluator for this exact
    /// entry. Full retained Source replay precedes Native readmission.
    pub(crate) fn replay_and_readmit<'a>(
        &self,
        verifier: &mut PreparedSourceVerification,
        family: &mut PreparedNativeFamily,
        budget: &'a mut ParserHistoricalReadmissionBudget,
    ) -> Result<ParserHistoricalReadmission<'a, I, O>, ParserHistoricalReadmissionRefusal> {
        use ParserHistoricalReadmissionRefusal as R;
        if verifier.entry() != self.entry {
            return Err(R::Entry);
        }
        if !family.contains_descriptor(I::PREPARED_DESCRIPTOR)
            || !family.contains_descriptor(O::PREPARED_DESCRIPTOR)
        {
            return Err(R::Descriptor);
        }
        // Conservatively reserve both complete generated conversion envelopes
        // before the first decode; no post-decode size substitutes for this peak.
        let peak = family
            .storage_receipt()
            .conversion_requested_bytes_bound
            .checked_mul(2)
            .ok_or(R::Pressure)?;
        if peak > budget.maximum_bytes {
            return Err(R::Pressure);
        }
        if verifier.evaluate(&self.input).map_err(|_| R::Source)? != self.output {
            return Err(R::DifferentOutput);
        }
        let input = family.decode::<I>(&self.input).map_err(R::Native)?;
        let output = family.decode::<O>(&self.output).map_err(R::Native)?;
        Ok(ParserHistoricalReadmission {
            input,
            output,
            _budget: budget,
        })
    }
}
