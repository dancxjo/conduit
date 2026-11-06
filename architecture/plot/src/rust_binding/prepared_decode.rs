//! Storage gate for opt-in generated one-shot Native materialization.
//! Admission remains the existing checked Native Source-law execution.
use super::{AdmittedNativeCanonical, NativePrimitive};
use crate::{
    CheckedNativeType, CheckedSyntaxDocument, NativeTypeValueContract, PortableExpressionProgram,
};
use alloc::{string::String, vec::Vec};
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparedDecodeRefusal {
    Preparation,
    Unsupported,
    ForeignWitness,
    Pressure,
    State,
    Canonical,
    Cancelled,
}
#[derive(Clone, Copy, Debug)]
pub struct PreparedDecodeBounds {
    pub canonical_bytes: usize,
    pub leaf_steps: usize,
    pub text_bytes: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparedDecodeProgress {
    Pending,
    Complete,
}
/// Used only at preparation by generated closed-closure checks.
pub fn check_prepared_decode_native<'d>(
    document: &'d CheckedSyntaxDocument,
    identity: &str,
    semantic: &[u8],
    contracts: &[NativeTypeValueContract],
    invariants: &[PortableExpressionProgram],
) -> Result<&'d CheckedNativeType, PreparedDecodeRefusal> {
    let n = super::prepared_native::checked_native_owner(document, &identity.into())
        .map_err(|_| PreparedDecodeRefusal::Preparation)?;
    if n.value_type
        .canonical_bytes()
        .map_err(|_| PreparedDecodeRefusal::Preparation)?
        != semantic
        || n.value_contracts != contracts
        || n.invariants != invariants
    {
        return Err(PreparedDecodeRefusal::Preparation);
    }
    Ok(n)
}
/// Its public helpers cannot construct a generated Native value. The generated
/// staging struct owns this gate privately and exposes no raw-field mutation.
pub struct PreparedDecodeGate<'d> {
    native: &'d CheckedNativeType,
    input: Vec<u8>,
    maximum: usize,
    total: usize,
    next: usize,
    failed: Option<PreparedDecodeRefusal>,
}
impl<'d> PreparedDecodeGate<'d> {
    pub fn new(
        native: &'d CheckedNativeType,
        total: usize,
        bounds: PreparedDecodeBounds,
    ) -> Result<Self, PreparedDecodeRefusal> {
        if total == 0
            || total > bounds.leaf_steps
            || bounds.leaf_steps > 4096
            || bounds.canonical_bytes == 0
            || bounds.canonical_bytes > 65536
            || bounds.text_bytes > 65536
        {
            return Err(PreparedDecodeRefusal::Preparation);
        }
        Ok(Self {
            native,
            input: Vec::with_capacity(bounds.canonical_bytes),
            maximum: bounds.canonical_bytes,
            total,
            next: 0,
            failed: None,
        })
    }
    fn correlate(
        &mut self,
        witness: &AdmittedNativeCanonical<'_>,
    ) -> Result<(), PreparedDecodeRefusal> {
        if let Some(error) = self.failed {
            return Err(error);
        }
        let result = if !core::ptr::eq(self.native, witness.native_type()) {
            Err(PreparedDecodeRefusal::ForeignWitness)
        } else if witness.canonical_bytes().len() > self.maximum {
            Err(PreparedDecodeRefusal::Pressure)
        } else if !self.input.is_empty() && self.input != witness.canonical_bytes() {
            Err(PreparedDecodeRefusal::ForeignWitness)
        } else {
            Ok(())
        };
        if let Err(error) = result {
            self.failed = Some(error);
            return Err(error);
        }
        if self.input.is_empty() {
            self.input.extend_from_slice(witness.canonical_bytes());
        }
        Ok(())
    }
    pub fn step_value<'a>(
        &mut self,
        witness: &'a AdmittedNativeCanonical<'_>,
    ) -> Result<(usize, ValidatedCanonicalStructuredValue<'a>), PreparedDecodeRefusal> {
        self.correlate(witness)?;
        if self.next >= self.total {
            return Err(PreparedDecodeRefusal::State);
        }
        let value = validate_canonical_structured_value(witness.canonical_bytes())
            .map_err(|_| PreparedDecodeRefusal::Canonical)?;
        Ok((self.next, value))
    }
    pub fn advance(&mut self) -> PreparedDecodeProgress {
        self.next += 1;
        if self.next == self.total {
            PreparedDecodeProgress::Complete
        } else {
            PreparedDecodeProgress::Pending
        }
    }
    pub fn fail(&mut self, reason: PreparedDecodeRefusal) -> PreparedDecodeRefusal {
        self.failed = Some(reason);
        reason
    }
    pub fn finish(
        &mut self,
        witness: &AdmittedNativeCanonical<'_>,
    ) -> Result<(), PreparedDecodeRefusal> {
        self.correlate(witness)?;
        if self.next != self.total {
            return Err(PreparedDecodeRefusal::State);
        }
        Ok(())
    }
    pub fn cancel(&mut self) {
        self.failed = Some(PreparedDecodeRefusal::Cancelled);
    }
    pub fn canonical_capacity(&self) -> usize {
        self.input.capacity()
    }
}
pub fn prepared_decode_primitive<T: NativePrimitive>(
    value: ValidatedCanonicalStructuredValue<'_>,
    kind: &str,
) -> Result<T, PreparedDecodeRefusal> {
    let bytes = value
        .primitive_representation_bytes(kind)
        .map_err(|_| PreparedDecodeRefusal::Canonical)?;
    T::decode_primitive(bytes).ok_or(PreparedDecodeRefusal::Canonical)
}
pub fn prepared_decode_text(
    output: &mut String,
    value: ValidatedCanonicalStructuredValue<'_>,
) -> Result<(), PreparedDecodeRefusal> {
    let bytes = value
        .primitive_representation_bytes("value/text")
        .map_err(|_| PreparedDecodeRefusal::Canonical)?;
    let text = core::str::from_utf8(bytes).map_err(|_| PreparedDecodeRefusal::Canonical)?;
    if text.len() > output.capacity() {
        return Err(PreparedDecodeRefusal::Pressure);
    }
    output.clear();
    output.push_str(text);
    Ok(())
}

/// Actual-work gate for a generated variable-length materializer. The schema
/// ceiling and the owner's work budget are distinct: a smaller actual value can
/// finish below the ceiling, while exhaustion retains the same private staging.
/// Like the fixed gate, this helper cannot construct or publish a Native value.
pub struct PreparedDynamicDecodeGate<'d> {
    gate: PreparedDecodeGate<'d>,
    budget: usize,
    complete: bool,
    pending: bool,
}
impl<'d> PreparedDynamicDecodeGate<'d> {
    pub fn new(
        native: &'d CheckedNativeType,
        maximum_steps: usize,
        bounds: PreparedDecodeBounds,
    ) -> Result<Self, PreparedDecodeRefusal> {
        if maximum_steps == 0
            || maximum_steps > 4096
            || bounds.leaf_steps == 0
            || bounds.leaf_steps > 4096
        {
            return Err(PreparedDecodeRefusal::Preparation);
        }
        // The inner fixed gate supplies witness/canonical custody and the
        // immutable schema ceiling. This wrapper enforces the original budget
        // before every actual step; this does not increase an owner's grant.
        let gate = PreparedDecodeGate::new(
            native,
            maximum_steps,
            PreparedDecodeBounds {
                leaf_steps: maximum_steps,
                ..bounds
            },
        )?;
        Ok(Self {
            gate,
            budget: bounds.leaf_steps,
            complete: false,
            pending: false,
        })
    }
    pub fn step_value<'a>(
        &mut self,
        witness: &'a AdmittedNativeCanonical<'_>,
    ) -> Result<ValidatedCanonicalStructuredValue<'a>, PreparedDecodeRefusal> {
        self.gate.correlate(witness)?;
        if self.complete || self.pending {
            return Err(PreparedDecodeRefusal::State);
        }
        if self.gate.next >= self.budget || self.gate.next >= self.gate.total {
            return Err(self.gate.fail(PreparedDecodeRefusal::Pressure));
        }
        let value = validate_canonical_structured_value(witness.canonical_bytes())
            .map_err(|_| self.gate.fail(PreparedDecodeRefusal::Canonical))?;
        self.pending = true;
        Ok(value)
    }
    pub fn advance(
        &mut self,
        complete: bool,
    ) -> Result<PreparedDecodeProgress, PreparedDecodeRefusal> {
        if let Some(reason) = self.gate.failed {
            return Err(reason);
        }
        if !self.pending || self.complete {
            return Err(PreparedDecodeRefusal::State);
        }
        self.pending = false;
        self.gate.next += 1;
        self.complete = complete;
        Ok(if complete {
            PreparedDecodeProgress::Complete
        } else {
            PreparedDecodeProgress::Pending
        })
    }
    pub fn finish(
        &mut self,
        witness: &AdmittedNativeCanonical<'_>,
    ) -> Result<(), PreparedDecodeRefusal> {
        self.gate.correlate(witness)?;
        if !self.complete || self.pending {
            return Err(PreparedDecodeRefusal::State);
        }
        Ok(())
    }
    pub fn fail(&mut self, reason: PreparedDecodeRefusal) -> PreparedDecodeRefusal {
        self.pending = false;
        self.gate.fail(reason)
    }
    pub fn cancel(&mut self) {
        self.pending = false;
        self.gate.cancel();
    }
    pub fn consumed_steps(&self) -> usize {
        self.gate.next
    }
    pub fn canonical_capacity(&self) -> usize {
        self.gate.canonical_capacity()
    }
}
