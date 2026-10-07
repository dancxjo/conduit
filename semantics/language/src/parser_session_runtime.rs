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

#[derive(Debug)]
pub enum ParserSourceFlowRefusal<E> {
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
    binding: PhantomData<fn(I) -> O>,
}
impl<I: NativeRustBinding, O: NativeRustBinding, E: ParserSourceExecutor>
    PreparedParserSourceFlow<I, O, E>
{
    pub fn new(executor: E) -> Result<Self, ParserSourceFlowRefusal<E::Error>> {
        use ParserSourceFlowRefusal::*;
        if executor.entry().is_empty() {
            return Err(EmptyEntry);
        }
        if executor.input_type() != &I::semantic_type().map_err(Native)? {
            return Err(InputType);
        }
        if executor.output_type() != &O::semantic_type().map_err(Native)? {
            return Err(OutputType);
        }
        Ok(Self {
            executor,
            next_ordinal: 0,
            poisoned: false,
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
    pub fn transact(&mut self, input: I) -> Result<O, ParserSourceFlowRefusal<E::Error>> {
        use ParserSourceFlowRefusal::*;
        if self.poisoned {
            return Err(Poisoned);
        }
        let following = self.next_ordinal.checked_add(1).ok_or(Exhausted)?;
        let input = input.into_structured().map_err(Native)?;
        // Poison before invoking: a returned error cannot prove non-consumption.
        self.poisoned = true;
        let output = self
            .executor
            .transact(self.next_ordinal, &input)
            .map_err(Execution)?;
        if output.value_type() != &O::semantic_type().map_err(Native)? {
            return Err(OutputType);
        }
        let output = O::from_structured(output).map_err(Native)?;
        self.next_ordinal = following;
        self.poisoned = false;
        Ok(output)
    }
}
