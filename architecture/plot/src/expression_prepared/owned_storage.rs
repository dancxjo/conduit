//! Narrow owned-storage qualification for the existing Some(opaque Ref) program.
//! Caller-owned checked program/Type material and its decoding are separate.
use super::*;
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape as Shape};
use core::mem::size_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualifiedExpressionStorageRefusal {
    UnsupportedProgram,
    InsufficientPreparation,
    InvalidPreparedStorage,
}
/// Requested-byte model excluding allocator metadata and caller-owned program.
/// Preparation includes all simultaneously live evaluator material and the
/// temporary Type serializations made by this exact existing preparation path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualifiedExpressionStorage {
    pub preparation_bytes: usize,
    pub retained_bytes: usize,
}
fn reference(ty: &StructuredInfoType) -> bool {
    matches!(ty.shape(), Shape::Leaf(id) if id.as_str() == conduit_core::RESOURCE_REFERENCE_INFO_ID)
}
fn optional(ty: &StructuredInfoType) -> bool {
    let Shape::Variant { schema, cases } = ty.shape() else {
        return false;
    };
    schema.as_str() == "conduit.conduitese.optional.v1"
        && cases.len() == 2
        && cases[0].tag() == "none"
        && matches!(cases[0].payload_type().shape(), Shape::Leaf(id) if id.as_str() == conduit_core::EMPTY_INFO_ID)
        && cases[1].tag() == "some"
        && reference(cases[1].payload_type())
}
pub(super) fn qualifies_view(program: ProgramView<'_>) -> bool {
    if !reference(program.input_type)
        || !optional(program.output_type)
        || program
            .input_maximum_bytes
            .is_some_and(|n| n as usize > conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES)
        || program.root.value_type != *program.output_type
    {
        return false;
    }
    let PortableExpressionOperation::Variant { tag, payload } = &program.root.operation else {
        return false;
    };
    tag == "some"
        && reference(&payload.value_type)
        && matches!(payload.operation, PortableExpressionOperation::Input)
}
fn view(program: &PortableExpressionProgram) -> ProgramView<'_> {
    ProgramView {
        input_maximum_bytes: program.input_maximum_bytes,
        input_type: &program.input_type,
        output_type: &program.output_type,
        root: &program.root,
    }
}
impl QualifiedExpressionStorage {
    /// Borrowed preflight: neither validates by serializing nor allocates an
    /// error string. Other expressions require their own reviewed model.
    pub fn for_resource_reference_some(
        program: &PortableExpressionProgram,
    ) -> Result<Self, QualifiedExpressionStorageRefusal> {
        if !qualifies_view(view(program)) {
            return Err(QualifiedExpressionStorageRefusal::UnsupportedProgram);
        }
        Ok(Self::resource_reference_some_model(
            program.input_maximum_bytes,
        ))
    }
    pub(super) fn resource_reference_some_model(maximum: Option<u32>) -> Self {
        let leaf = 5 + conduit_core::RESOURCE_REFERENCE_INFO_ID.len();
        let optional_prefix = 9
            + "conduit.conduitese.optional.v1".len()
            + 4
            + "none".len()
            + 5
            + conduit_core::EMPTY_INFO_ID.len()
            + 4
            + "some".len()
            + leaf;
        let input = maximum.map_or(
            conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES,
            |n| n as usize,
        );
        let output = optional_prefix + 9 + 5 + input;
        // Tape envelope/code/constants/child slots, boxed child evaluator,
        // child's leaf prefix/output, outer prefix/output and outer inline.
        let retained_bytes = size_of::<PreparedPortableExpressionEvaluator>()
            + PreparedStructuredExpression::qualified_some_tape_bound()
            + size_of::<PreparedPortableExpressionEvaluator>()
            + leaf
            + input
            + optional_prefix
            + output;
        // Root output-bound serialization and child leaf validation prefix
        // are short-lived. Charging both simultaneously is conservative.
        let preparation_bytes = retained_bytes + optional_prefix + leaf;
        Self {
            preparation_bytes,
            retained_bytes,
        }
    }
}
impl PreparedPortableExpressionEvaluator {
    pub fn new_qualified_resource_reference_some(
        program: &PortableExpressionProgram,
        admitted_bytes: usize,
    ) -> Result<Self, QualifiedExpressionStorageRefusal> {
        let receipt = QualifiedExpressionStorage::for_resource_reference_some(program)?;
        if admitted_bytes < receipt.preparation_bytes {
            return Err(QualifiedExpressionStorageRefusal::InsufficientPreparation);
        }
        let prepared = Self::new(program)
            .map_err(|_| QualifiedExpressionStorageRefusal::InvalidPreparedStorage)?;
        let actual = prepared.qualified_retained_backing_bytes()?
            + size_of::<PreparedPortableExpressionEvaluator>();
        if actual > receipt.retained_bytes {
            return Err(QualifiedExpressionStorageRefusal::InvalidPreparedStorage);
        }
        Ok(prepared)
    }
    /// Actual reachable capacities, excluding inline Self. This deliberately
    /// refuses engines outside the reviewed structured/reference shape.
    pub fn qualified_retained_backing_bytes(
        &self,
    ) -> Result<usize, QualifiedExpressionStorageRefusal> {
        if !matches!(self.input, PreparedInput::ResourceReference(_))
            || self.input_value_contract.is_some()
            || self.output_value_contract.is_some()
        {
            return Err(QualifiedExpressionStorageRefusal::UnsupportedProgram);
        }
        let root = match &self.root {
            PreparedRoot::Structured(value) => value.qualified_reference_storage(),
            PreparedRoot::ResourceReference(value) => value.qualified_reference_storage(),
            _ => None,
        }
        .ok_or(QualifiedExpressionStorageRefusal::UnsupportedProgram)?;
        root.checked_add(self.output.capacity())
            .ok_or(QualifiedExpressionStorageRefusal::InvalidPreparedStorage)
    }
}
