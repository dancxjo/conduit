//! Target-independent invocation boundary for retained parser Source flows.
//!
//! A target prepares and owns the ordinary Plan executor. Language checks the
//! complete Native port Types and admits every response. This boundary neither
//! chooses parser actions nor authorizes stable or committed facts.
use conduit_core::{StructuredInfoType, StructuredInfoValue};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
use core::marker::PhantomData;

/// A retained executor for one checked Source entry, supplied by its target.
/// Implementations must retain the checked Source and prepared Plan, and execute
/// the exact declared ports through ordinary scheduler owners. Port declarations
/// alone are metadata, not proof that a target performed that execution.
pub trait ParserSourceExecutor {
    type Error;
    fn entry(&self) -> &str;
    fn input_type(&self) -> &StructuredInfoType;
    fn output_type(&self) -> &StructuredInfoType;
    fn transact(
        &mut self,
        ordinal: u64,
        input: &StructuredInfoValue,
    ) -> Result<StructuredInfoValue, Self::Error>;
}

/// Finite ingress and response storage admitted before executor ownership.
/// Bounds describe this port, not linguistic commitment or played delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParserSourceFlowLimits {
    pub max_invocations: u16,
    /// Maximum canonical input admitted to the executor.
    pub max_input_bytes: u32,
    /// Maximum canonical response accepted by this port. The target must
    /// separately budget production/allocation of its response before execution.
    pub max_output_bytes: u32,
}

#[derive(Debug)]
pub enum ParserSourceFlowRefusal<E> {
    Limits,
    InvocationPressure,
    InputPressure,
    OutputPressure,
    Cancelled,
    InputType,
    OutputType,
    EmptyEntry,
    Exhausted,
    Poisoned,
    Native(NativeBindingRefusal),
    Execution(E),
}

/// A sealed typed port over one retained target executor.
/// A failed transaction poisons this port: its input may already have been
/// consumed, so silently replaying the same ordinal could duplicate effects.
pub struct PreparedParserSourceFlow<I, O, E> {
    executor: E,
    next_ordinal: u64,
    poisoned: bool,
    cancelled: bool,
    limits: ParserSourceFlowLimits,
    input_type: StructuredInfoType,
    output_type: StructuredInfoType,
    binding: PhantomData<fn(I) -> O>,
}
impl<I: NativeRustBinding, O: NativeRustBinding, E: ParserSourceExecutor>
    PreparedParserSourceFlow<I, O, E>
{
    pub fn new(
        executor: E,
        limits: ParserSourceFlowLimits,
    ) -> Result<Self, ParserSourceFlowRefusal<E::Error>> {
        use ParserSourceFlowRefusal::*;
        if limits.max_invocations == 0
            || limits.max_input_bytes == 0
            || limits.max_output_bytes == 0
        {
            return Err(Limits);
        }
        if executor.entry().is_empty() {
            return Err(EmptyEntry);
        }
        let input_type = I::semantic_type().map_err(Native)?;
        let output_type = O::semantic_type().map_err(Native)?;
        if executor.input_type() != &input_type {
            return Err(InputType);
        }
        if executor.output_type() != &output_type {
            return Err(OutputType);
        }
        Ok(Self {
            executor,
            next_ordinal: 0,
            poisoned: false,
            cancelled: false,
            limits,
            input_type,
            output_type,
            binding: PhantomData,
        })
    }
    pub fn entry(&self) -> &str {
        self.executor.entry()
    }
    pub fn next_ordinal(&self) -> u64 {
        self.next_ordinal
    }
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }
    pub fn limits(&self) -> ParserSourceFlowLimits {
        self.limits
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    /// Stop future ingress while retaining the executor and admitted custody.
    /// Cancellation does not retract prior semantic or external effects.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
    pub fn transact(&mut self, input: I) -> Result<O, ParserSourceFlowRefusal<E::Error>> {
        use ParserSourceFlowRefusal::*;
        if self.cancelled {
            return Err(Cancelled);
        }
        if self.poisoned {
            return Err(Poisoned);
        }
        if self.next_ordinal >= u64::from(self.limits.max_invocations) {
            return Err(InvocationPressure);
        }
        let following = self.next_ordinal.checked_add(1).ok_or(Exhausted)?;
        let input = input.into_structured().map_err(Native)?;
        if input.value_type() != &self.input_type {
            return Err(InputType);
        }
        if input
            .canonical_bytes()
            .map_err(|error| Native(NativeBindingRefusal::InvalidValue(error)))?
            .len()
            > self.limits.max_input_bytes as usize
        {
            return Err(InputPressure);
        }
        // Poison before invoking: a returned error cannot prove non-consumption.
        self.poisoned = true;
        let output = self
            .executor
            .transact(self.next_ordinal, &input)
            .map_err(Execution)?;
        if output
            .canonical_bytes()
            .map_err(|error| Native(NativeBindingRefusal::InvalidValue(error)))?
            .len()
            > self.limits.max_output_bytes as usize
        {
            return Err(OutputPressure);
        }
        if output.value_type() != &self.output_type {
            return Err(OutputType);
        }
        let output = O::from_structured(output).map_err(Native)?;
        self.next_ordinal = following;
        self.poisoned = false;
        Ok(output)
    }
}
