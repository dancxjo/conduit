//! Retained, bounded Native admission. Shape alone never yields a witness.
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
struct Scope {
    path: Vec<Part>,
    indices: Vec<u16>,
    latest: Vec<u16>,
    latest_generation: u64,
    checked_invariants: usize,
    native_index: usize,
    evaluators: Vec<PreparedPortableExpressionEvaluator>,
}
/// A finite admission profile; exceeding any bound refuses preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedNativeBounds {
    pub canonical_bytes: usize,
    pub scopes: usize,
    pub invariants: usize,
    pub contracts: usize,
}
/// Granted limits and actual prepared storage counts; these are not total heap bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedNativeCapacityReport {
    pub granted: PreparedNativeBounds,
    pub occurrence_limit: usize,
    pub prepared_scopes: usize,
    pub prepared_evaluators: usize,
    pub prepared_contracts: usize,
    /// Allocated u16 slots across current and retained contract cursors.
    pub contract_index_capacity: usize,
    /// Allocated NativePathStep slots across prepared contract paths.
    pub contract_path_step_capacity: usize,
    pub canonical_input_capacity: usize,
    pub canonical_scratch_capacity: usize,
    pub evaluator_output_capacity: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedNativeProgress {
    Pending,
    Admitted,
}
/// Private witness borrows the exact retained bytes and checked Native metadata.
/// It cannot cast a different schema or construct a generated Rust value.
pub struct AdmittedNativeCanonical<'a> {
    document: &'a CheckedSyntaxDocument,
    native: &'a CheckedNativeType,
    bytes: &'a [u8],
}
impl AdmittedNativeCanonical<'_> {
    pub fn source_document(&self) -> &CheckedSyntaxDocument {
        self.document
    }
    pub fn native_type(&self) -> &CheckedNativeType {
        self.native
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        self.bytes
    }
}
// Keep all consumers on the same private syntax-checking Native catalogue.
// Inspection metadata may detect a conflict but never creates a new owner.
pub(super) fn checked_native_owner<'d>(
    document: &'d CheckedSyntaxDocument,
    identity: &KindId,
) -> Result<&'d CheckedNativeType, PreparedNativeRefusal> {
    traversal::lookup(document, identity)
}

pub const MAXIMUM_PREPARED_NATIVE_OCCURRENCES: usize = 4096;
/// These are latest-occurrence observations, not historical outputs for every item.
pub struct NativeOccurrence<'a> {
    pub scope: usize,
    pub path: &'a [NativePathStep],
    pub indices: &'a [u16],
    pub checked_invariants: usize,
}
/// Latest actual primitive contract occurrence, including its full root path.
pub struct NativeContractOccurrence<'a> {
    pub scope: usize,
    pub contract: usize,
    pub path: &'a [NativePathStep],
    pub indices: &'a [u16],
    /// Whether validation was attempted; false on occurrence pressure.
    pub checked: bool,
}
#[path = "prepared_native/contracts.rs"]
mod contracts;
#[path = "prepared_native/facades.rs"]
mod facades;
#[path = "prepared_native/metadata.rs"]
mod metadata;
#[path = "prepared_native/registry.rs"]
mod registry;
#[path = "prepared_native/shared_facade.rs"]
mod shared_facade;
#[path = "prepared_native/structural_root.rs"]
mod structural_root;
use structural_root::RootDescriptor;
#[path = "prepared_native/traversal.rs"]
mod traversal;
pub use facades::{
    AdmittedOwnedNativeCanonical, OwnedPreparedNativeAdmission, PreparedNativeAdmission,
};
pub use registry::NativeLawRegistry;
pub(crate) use registry::PreparedRegisteredValue;
pub use shared_facade::{AdmittedRegisteredNativeCanonical, PreparedRegisteredNativeAdmission};
struct AdmissionEngine<'d> {
    metadata: metadata::Metadata<'d>,
    root: RootDescriptor,
    shape: PreparedStructuredValueValidator,
    scopes: Vec<Scope>,
    contracts: Vec<contracts::ContractCursor>,
    next_contract: usize,
    contract_occurrences: usize,
    latest_contract: Option<usize>,
    input: Vec<u8>,
    scratch: Vec<u8>,
    maximum: usize,
    bounds: PreparedNativeBounds,
    next: usize,
    generation: u64,
    latest_scope: Option<usize>,
    occurrences: usize,
    occurrence_limit: usize,
    active: bool,
    failure: Option<PreparedNativeRefusal>,
}
impl<'d> AdmissionEngine<'d> {
    fn prepare(
        metadata: metadata::Metadata<'d>,
        identity: &KindId,
        bounds: PreparedNativeBounds,
        occurrence_limit: usize,
    ) -> Result<Self, PreparedNativeRefusal> {
        Self::validate_profile(bounds, occurrence_limit)?;
        let index = metadata.lookup(identity)?;
        Self::prepare_root(
            metadata,
            RootDescriptor::Native(index),
            bounds,
            occurrence_limit,
        )
    }
    fn validate_profile(
        bounds: PreparedNativeBounds,
        occurrence_limit: usize,
    ) -> Result<(), PreparedNativeRefusal> {
        if occurrence_limit == 0 || occurrence_limit > MAXIMUM_PREPARED_NATIVE_OCCURRENCES {
            return Err(PreparedNativeRefusal::Bounds);
        }
        if bounds.scopes == 0
            || bounds.scopes > 4096
            || bounds.invariants > 4096
            || bounds.contracts > 4096
        {
            return Err(PreparedNativeRefusal::Bounds);
        }
        Ok(())
    }
    fn prepare_root(
        metadata: metadata::Metadata<'d>,
        root: RootDescriptor,
        bounds: PreparedNativeBounds,
        occurrence_limit: usize,
    ) -> Result<Self, PreparedNativeRefusal> {
        Self::validate_profile(bounds, occurrence_limit)?;
        let value_type = root.value_type(&metadata);
        let shape = PreparedStructuredValueValidator::new(value_type, bounds.canonical_bytes)
            .map_err(|_| PreparedNativeRefusal::Bounds)?;
        let mut scopes = Vec::new();
        let mut count = (0, 0);
        traversal::prepare(
            &metadata,
            value_type,
            &mut Vec::new(),
            &mut scopes,
            bounds,
            &mut count,
        )?;
        let contracts = contracts::prepare(&scopes, &metadata, bounds)?;
        Ok(Self {
            contracts,
            next_contract: 0,
            contract_occurrences: 0,
            latest_contract: None,
            metadata,
            root,
            shape,
            scopes,
            input: Vec::with_capacity(bounds.canonical_bytes),
            scratch: Vec::with_capacity(bounds.canonical_bytes),
            maximum: bounds.canonical_bytes,
            bounds,
            next: 0,
            generation: 0,
            latest_scope: None,
            occurrences: 0,
            occurrence_limit,
            active: false,
            failure: None,
        })
    }
    pub fn capacity_report(&self) -> PreparedNativeCapacityReport {
        PreparedNativeCapacityReport {
            granted: self.bounds,
            occurrence_limit: self.occurrence_limit,
            prepared_scopes: self.scopes.len(),
            prepared_contracts: self.contracts.len(),
            contract_index_capacity: self.contracts.iter().map(|v| v.index_capacity()).sum(),
            contract_path_step_capacity: self.contracts.iter().map(|v| v.path.capacity()).sum(),
            prepared_evaluators: self.scopes.iter().map(|scope| scope.evaluators.len()).sum(),
            canonical_input_capacity: self.input.capacity(),
            canonical_scratch_capacity: self.scratch.capacity(),
            evaluator_output_capacity: self
                .scopes
                .iter()
                .flat_map(|scope| &scope.evaluators)
                .map(PreparedPortableExpressionEvaluator::output_capacity)
                .sum(),
        }
    }
    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }
    pub fn scope_native_type(&self, scope: usize) -> Option<&CheckedNativeType> {
        self.scopes
            .get(scope)
            .map(|v| self.metadata.get(v.native_index))
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
    pub fn latest_contract_occurrence(&self) -> Option<NativeContractOccurrence<'_>> {
        let cursor = self.contracts.get(self.latest_contract?)?;
        if !self.active || cursor.latest_generation != self.generation {
            return None;
        }
        Some(NativeContractOccurrence {
            scope: cursor.scope,
            contract: cursor.contract,
            path: &cursor.path,
            indices: &cursor.latest,
            checked: cursor.checked,
        })
    }
    pub fn actual_contract_occurrences(&self) -> usize {
        self.contract_occurrences
    }
    /// Native-law occurrences; primitive contract work is counted separately.
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
        self.contract_occurrences = 0;
        self.next_contract = 0;
        self.latest_contract = None;
        contracts::reset(&mut self.contracts);
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
        if self.failure.is_none() {
            self.failure = Some(PreparedNativeRefusal::Cancelled);
        }
    }
    /// One Native-law/primitive-contract occurrence or bounded path backtrack.
    /// Canonical envelope validation and projection retain their finite scan bounds.
    pub fn step(&mut self) -> Result<PreparedNativeProgress, PreparedNativeRefusal> {
        if let Some(reason) = &self.failure {
            return Err(reason.clone());
        }
        if !self.active {
            return Err(PreparedNativeRefusal::State);
        }
        if self.finished() {
            return Ok(PreparedNativeProgress::Admitted);
        }
        let result = if self.next_contract < self.contracts.len() {
            contracts::step(self)
        } else {
            traversal::step(self)
        };
        match result {
            Ok(()) => Ok(if self.finished() {
                PreparedNativeProgress::Admitted
            } else {
                PreparedNativeProgress::Pending
            }),
            Err(reason) => self.fail(reason),
        }
    }
    fn finished(&self) -> bool {
        self.next_contract == self.contracts.len() && self.next == self.scopes.len()
    }
    /// Explicitly retire a successful admission before reusing its prepared storage.
    /// Failure/cancellation evidence cannot be silently reset by this entrance.
    pub fn retire_admitted(&mut self) -> Result<(), PreparedNativeRefusal> {
        if !self.active || !self.finished() || self.failure.is_some() {
            return Err(PreparedNativeRefusal::State);
        }
        self.input.clear();
        self.scratch.clear();
        self.next = 0;
        self.active = false;
        self.latest_scope = None;
        self.latest_contract = None;
        self.next_contract = 0;
        Ok(())
    }
    fn check_admitted(&self) -> Result<(), PreparedNativeRefusal> {
        if let Some(reason) = &self.failure {
            return Err(reason.clone());
        }
        if !self.active || !self.finished() {
            return Err(PreparedNativeRefusal::State);
        }
        Ok(())
    }
}
fn validate_scope(
    scope: &mut Scope,
    value: ValidatedCanonicalStructuredValue<'_>,
    scratch: &mut Vec<u8>,
    maximum: usize,
    index: usize,
) -> Result<(), PreparedNativeRefusal> {
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
