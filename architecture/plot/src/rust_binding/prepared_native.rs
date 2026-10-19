//! Retained, bounded Native admission. Shape alone never yields a witness.
use super::validate_native_contracts_borrowed;
use crate::{CheckedNativeType, CheckedSyntaxDocument, PreparedPortableExpressionEvaluator};
use alloc::{string::String, vec::Vec};
use conduit_core::{
    validate_canonical_structured_value, InfoBool, KindId, PreparedStructuredValueValidator,
    StructuredInfoType, StructuredInfoTypeShape as Shape, ValidatedCanonicalStructuredValue,
    ValueConstraintRefusal,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedNativeRefusal {
    Bounds,
    MissingNativeMetadata,
    ConflictingNativeMetadata,
    InvalidProgram,
    Shape,
    Contract {
        scope: usize,
        contract: usize,
        refusal: ValueConstraintRefusal,
    },
    Invariant {
        scope: usize,
        invariant: usize,
    },
    Evaluation {
        scope: usize,
        invariant: usize,
    },
    State,
    Cancelled,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativePathStep {
    Representation,
    Field(String),
    Each,
    Variant(String),
}
type Part = NativePathStep;
struct Scope<'d> {
    path: Vec<Part>,
    indices: Vec<u16>,
    latest: Vec<u16>,
    latest_generation: u64,
    checked_invariants: usize,
    native: &'d CheckedNativeType,
    evaluators: Vec<PreparedPortableExpressionEvaluator>,
}
/// A finite admission profile; exceeding either bound refuses preparation.
#[derive(Debug, Clone, Copy)]
pub struct PreparedNativeBounds {
    pub canonical_bytes: usize,
    pub scopes: usize,
    pub invariants: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedNativeProgress {
    Pending,
    Admitted,
}
/// Private witness borrows the exact retained bytes and checked Native metadata.
/// It cannot cast a different schema or construct a generated Rust value.
pub struct AdmittedNativeCanonical<'a> {
    native: &'a CheckedNativeType,
    bytes: &'a [u8],
}
impl AdmittedNativeCanonical<'_> {
    pub fn native_type(&self) -> &CheckedNativeType {
        self.native
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        self.bytes
    }
}
pub const MAXIMUM_PREPARED_NATIVE_OCCURRENCES: usize = 4096;
/// These are latest-occurrence observations, not historical outputs for every item.
pub struct NativeOccurrence<'a> {
    pub scope: usize,
    pub path: &'a [NativePathStep],
    pub indices: &'a [u16],
    pub checked_invariants: usize,
}
#[path = "prepared_native/traversal.rs"]
mod traversal;
pub struct PreparedNativeAdmission<'d> {
    document: &'d CheckedSyntaxDocument,
    native: &'d CheckedNativeType,
    shape: PreparedStructuredValueValidator,
    scopes: Vec<Scope<'d>>,
    input: Vec<u8>,
    scratch: Vec<u8>,
    maximum: usize,
    next: usize,
    generation: u64,
    latest_scope: Option<usize>,
    occurrences: usize,
    occurrence_limit: usize,
    active: bool,
    failure: Option<PreparedNativeRefusal>,
}
impl<'d> PreparedNativeAdmission<'d> {
    pub fn new(
        document: &'d CheckedSyntaxDocument,
        identity: &KindId,
        bounds: PreparedNativeBounds,
    ) -> Result<Self, PreparedNativeRefusal> {
        Self::new_with_occurrence_limit(
            document,
            identity,
            bounds,
            MAXIMUM_PREPARED_NATIVE_OCCURRENCES,
        )
    }
    pub fn new_with_occurrence_limit(
        document: &'d CheckedSyntaxDocument,
        identity: &KindId,
        bounds: PreparedNativeBounds,
        occurrence_limit: usize,
    ) -> Result<Self, PreparedNativeRefusal> {
        if occurrence_limit == 0 || occurrence_limit > MAXIMUM_PREPARED_NATIVE_OCCURRENCES {
            return Err(PreparedNativeRefusal::Bounds);
        }
        if bounds.scopes == 0 || bounds.scopes > 4096 || bounds.invariants > 4096 {
            return Err(PreparedNativeRefusal::Bounds);
        }
        let native = traversal::lookup(document, identity)?;
        let shape =
            PreparedStructuredValueValidator::new(&native.value_type, bounds.canonical_bytes)
                .map_err(|_| PreparedNativeRefusal::Bounds)?;
        let mut scopes = Vec::new();
        let mut count = (0, 0);
        traversal::prepare(
            document,
            &native.value_type,
            &mut Vec::new(),
            &mut scopes,
            bounds,
            &mut count,
        )?;
        Ok(Self {
            document,
            native,
            shape,
            scopes,
            input: Vec::with_capacity(bounds.canonical_bytes),
            scratch: Vec::with_capacity(bounds.canonical_bytes),
            maximum: bounds.canonical_bytes,
            next: 0,
            generation: 0,
            latest_scope: None,
            occurrences: 0,
            occurrence_limit,
            active: false,
            failure: None,
        })
    }
    pub fn source_document(&self) -> &CheckedSyntaxDocument {
        self.document
    }
    pub fn native_type(&self) -> &CheckedNativeType {
        self.native
    }
    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }
    pub fn scope_native_type(&self, scope: usize) -> Option<&CheckedNativeType> {
        self.scopes.get(scope).map(|v| v.native)
    }
    /// Output is only from this template's latest actual occurrence in the
    /// current admission; earlier occurrence outputs are deliberately not claimed.
    pub fn invariant_output(&self, scope: usize, invariant: usize) -> Option<&[u8]> {
        if !self.active {
            return None;
        }
        let scope = self.scopes.get(scope)?;
        if scope.latest_generation != self.generation || invariant >= scope.checked_invariants {
            return None;
        }
        scope.evaluators.get(invariant).map(|v| v.output_bytes())
    }
    pub fn latest_occurrence(&self) -> Option<NativeOccurrence<'_>> {
        let index = self.latest_scope?;
        let scope = &self.scopes[index];
        Some(NativeOccurrence {
            scope: index,
            path: &scope.path,
            indices: &scope.latest,
            checked_invariants: scope.checked_invariants,
        })
    }
    pub fn actual_occurrences(&self) -> usize {
        self.occurrences
    }
    pub fn actual_occurrence_limit(&self) -> usize {
        self.occurrence_limit
    }
    pub fn retained_bytes(&self) -> &[u8] {
        &self.input
    }
    pub fn failure(&self) -> Option<&PreparedNativeRefusal> {
        self.failure.as_ref()
    }
    /// Copies only into admitted storage. Pressure/state refusal leaves evidence intact.
    pub fn begin(&mut self, bytes: &[u8]) -> Result<(), PreparedNativeRefusal> {
        if self.active || self.failure.is_some() {
            return Err(PreparedNativeRefusal::State);
        }
        if bytes.len() > self.maximum {
            return Err(PreparedNativeRefusal::Bounds);
        }
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(PreparedNativeRefusal::Bounds)?;
        self.input.clear();
        self.input.extend_from_slice(bytes);
        self.generation = generation;
        self.latest_scope = None;
        self.occurrences = 0;
        if let Some(scope) = self.scopes.first_mut() {
            scope.indices.fill(0);
        }
        self.next = 0;
        self.active = true;
        if self.shape.validate(&self.input).is_err() {
            return self.fail(PreparedNativeRefusal::Shape);
        }
        Ok(())
    }
    fn fail<T>(&mut self, reason: PreparedNativeRefusal) -> Result<T, PreparedNativeRefusal> {
        self.failure = Some(reason.clone());
        Err(reason)
    }
    pub fn cancel(&mut self) {
        self.failure = Some(PreparedNativeRefusal::Cancelled);
    }
    /// One actual Native occurrence or bounded path backtrack per call; all Source programs were prepared before begin.
    pub fn step(&mut self) -> Result<PreparedNativeProgress, PreparedNativeRefusal> {
        if let Some(reason) = &self.failure {
            return Err(reason.clone());
        }
        if !self.active {
            return Err(PreparedNativeRefusal::State);
        }
        if self.next == self.scopes.len() {
            return Ok(PreparedNativeProgress::Admitted);
        }
        match traversal::step(self) {
            Ok(()) => Ok(if self.next == self.scopes.len() {
                PreparedNativeProgress::Admitted
            } else {
                PreparedNativeProgress::Pending
            }),
            Err(reason) => self.fail(reason),
        }
    }
    /// Explicitly retire a successful admission before reusing its prepared storage.
    /// Failure/cancellation evidence cannot be silently reset by this entrance.
    pub fn retire_admitted(&mut self) -> Result<(), PreparedNativeRefusal> {
        if !self.active || self.next != self.scopes.len() || self.failure.is_some() {
            return Err(PreparedNativeRefusal::State);
        }
        self.input.clear();
        self.scratch.clear();
        self.next = 0;
        self.active = false;
        self.latest_scope = None;
        Ok(())
    }
    pub fn admitted(&self) -> Result<AdmittedNativeCanonical<'_>, PreparedNativeRefusal> {
        if let Some(reason) = &self.failure {
            return Err(reason.clone());
        }
        if !self.active || self.next != self.scopes.len() {
            return Err(PreparedNativeRefusal::State);
        }
        Ok(AdmittedNativeCanonical {
            native: self.native,
            bytes: &self.input,
        })
    }
}
fn validate_scope(
    scope: &mut Scope<'_>,
    value: ValidatedCanonicalStructuredValue<'_>,
    scratch: &mut Vec<u8>,
    maximum: usize,
    index: usize,
) -> Result<(), PreparedNativeRefusal> {
    if let Err(reason) = validate_native_contracts_borrowed(value, &scope.native.value_contracts) {
        let contract = scope
            .native
            .value_contracts
            .iter()
            .position(|v| v.representation_path == reason.representation_path)
            .ok_or(PreparedNativeRefusal::Shape)?;
        return Err(PreparedNativeRefusal::Contract {
            scope: index,
            contract,
            refusal: reason.refusal,
        });
    }
    if value
        .type_bytes()
        .len()
        .checked_add(value.value_node().len())
        .filter(|v| *v <= maximum)
        .is_none()
    {
        return Err(PreparedNativeRefusal::Bounds);
    }
    scratch.clear();
    scratch.extend_from_slice(value.type_bytes());
    scratch.extend_from_slice(value.value_node());
    for (invariant, evaluator) in scope.evaluators.iter_mut().enumerate() {
        let result =
            evaluator
                .evaluate(scratch)
                .map_err(|_| PreparedNativeRefusal::Evaluation {
                    scope: index,
                    invariant,
                })?;
        scope.checked_invariants = invariant + 1;
        if !InfoBool::decode(result)
            .map_err(|_| PreparedNativeRefusal::Evaluation {
                scope: index,
                invariant,
            })?
            .get()
        {
            return Err(PreparedNativeRefusal::Invariant {
                scope: index,
                invariant,
            });
        }
    }
    Ok(())
}
