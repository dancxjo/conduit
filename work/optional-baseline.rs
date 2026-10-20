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
#[derive(Clone)]
enum Part {
    Representation,
    Field(String),
    Index(u16),
    Variant(String),
}
struct Scope<'d> {
    path: Vec<Part>,
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
pub struct PreparedNativeAdmission<'d> {
    document: &'d CheckedSyntaxDocument,
    native: &'d CheckedNativeType,
    shape: PreparedStructuredValueValidator,
    scopes: Vec<Scope<'d>>,
    input: Vec<u8>,
    scratch: Vec<u8>,
    maximum: usize,
    next: usize,
    active: bool,
    failure: Option<PreparedNativeRefusal>,
}
impl<'d> PreparedNativeAdmission<'d> {
    pub fn new(
        document: &'d CheckedSyntaxDocument,
        identity: &KindId,
        bounds: PreparedNativeBounds,
    ) -> Result<Self, PreparedNativeRefusal> {
        if bounds.scopes == 0 || bounds.scopes > 4096 || bounds.invariants > 4096 {
            return Err(PreparedNativeRefusal::Bounds);
        }
        let native = lookup(document, identity)?;
        let shape =
            PreparedStructuredValueValidator::new(&native.value_type, bounds.canonical_bytes)
                .map_err(|_| PreparedNativeRefusal::Bounds)?;
        let mut scopes = Vec::new();
        let mut count = (0, 0);
        prepare(
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
    pub fn invariant_output(&self, scope: usize, invariant: usize) -> Option<&[u8]> {
        self.scopes
            .get(scope)?
            .evaluators
            .get(invariant)
            .map(|v| v.output_bytes())
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
        self.input.clear();
        self.input.extend_from_slice(bytes);
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
    /// One finite schema scope per call; all Source programs were prepared before begin.
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
        let index = self.next;
        let outcome = validate_scope(
            &mut self.scopes[index],
            &self.input,
            &mut self.scratch,
            self.maximum,
            index,
        );
        if let Err(reason) = outcome {
            return self.fail(reason);
        }
        self.next += 1;
        Ok(if self.next == self.scopes.len() {
            PreparedNativeProgress::Admitted
        } else {
            PreparedNativeProgress::Pending
        })
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
fn lookup<'d>(
    document: &'d CheckedSyntaxDocument,
    identity: &KindId,
) -> Result<&'d CheckedNativeType, PreparedNativeRefusal> {
    let mut result: Option<&CheckedNativeType> = None;
    for native in document
        .native_types
        .iter()
        .chain(document.retained_native_types.iter())
        .filter(|v| &v.identity == identity)
    {
        if let Some(prior) = result {
            if prior.value_type != native.value_type
                || prior.value_contracts != native.value_contracts
                || prior.invariants != native.invariants
            {
                return Err(PreparedNativeRefusal::ConflictingNativeMetadata);
            }
        } else {
            result = Some(native);
        }
    }
    result.ok_or(PreparedNativeRefusal::MissingNativeMetadata)
}
fn prepare<'d>(
    document: &'d CheckedSyntaxDocument,
    ty: &StructuredInfoType,
    path: &mut Vec<Part>,
    scopes: &mut Vec<Scope<'d>>,
    bounds: PreparedNativeBounds,
    count: &mut (usize, usize),
) -> Result<(), PreparedNativeRefusal> {
    count.1 += 1;
    if path.len() > 64 || count.1 > 4096 {
        return Err(PreparedNativeRefusal::Bounds);
    }
    let identity = match ty.shape() {
        Shape::Nominal { schema, .. }
        | Shape::Record { schema, .. }
        | Shape::Variant { schema, .. } => Some(schema),
        _ => None,
    };
    if let Some(identity) = identity {
        if scopes.len() >= bounds.scopes {
            return Err(PreparedNativeRefusal::Bounds);
        }
        let native = lookup(document, identity)?;
        if &native.value_type != ty {
            return Err(PreparedNativeRefusal::ConflictingNativeMetadata);
        }
        let mut evaluators = Vec::new();
        for program in &native.invariants {
            count.0 += 1;
            if count.0 > bounds.invariants
                || &program.input_type != ty
                || program.output_type
                    != StructuredInfoType::leaf(KindId::new("value/bool"))
                        .map_err(|_| PreparedNativeRefusal::InvalidProgram)?
            {
                return Err(PreparedNativeRefusal::InvalidProgram);
            }
            evaluators.push(
                PreparedPortableExpressionEvaluator::new(program)
                    .map_err(|_| PreparedNativeRefusal::InvalidProgram)?,
            );
        }
        scopes.push(Scope {
            path: path.clone(),
            native,
            evaluators,
        });
    }
    match ty.shape() {
        Shape::Leaf(_) => (),
        Shape::Nominal { representation, .. } => {
            if matches!(representation.shape(), Shape::Nominal { .. }) {
                return Err(PreparedNativeRefusal::MissingNativeMetadata);
            }
            path.push(Part::Representation);
            prepare(document, representation, path, scopes, bounds, count)?;
            path.pop();
        }
        Shape::Record { fields, .. } => {
            for field in fields {
                path.push(Part::Field(field.name().into()));
                prepare(document, field.value_type(), path, scopes, bounds, count)?;
                path.pop();
            }
        }
        Shape::Variant { cases, .. } => {
            for case in cases {
                path.push(Part::Variant(case.tag().into()));
                prepare(document, case.payload_type(), path, scopes, bounds, count)?;
                path.pop();
            }
        }
        Shape::Collection { element, length } => {
            prepare_elements(document, element, length, path, scopes, bounds, count)?
        }
        Shape::Sequence {
            element,
            maximum_items,
            ..
        } => prepare_elements(
            document,
            element,
            maximum_items,
            path,
            scopes,
            bounds,
            count,
        )?,
    }
    Ok(())
}
fn prepare_elements<'d>(
    document: &'d CheckedSyntaxDocument,
    element: &StructuredInfoType,
    length: u16,
    path: &mut Vec<Part>,
    scopes: &mut Vec<Scope<'d>>,
    bounds: PreparedNativeBounds,
    count: &mut (usize, usize),
) -> Result<(), PreparedNativeRefusal> {
    for index in 0..length {
        path.push(Part::Index(index));
        prepare(document, element, path, scopes, bounds, count)?;
        path.pop();
    }
    Ok(())
}
fn select<'a>(
    mut value: ValidatedCanonicalStructuredValue<'a>,
    path: &[Part],
) -> Result<Option<ValidatedCanonicalStructuredValue<'a>>, PreparedNativeRefusal> {
    for part in path {
        let next = match part {
            Part::Representation => Some(
                value
                    .representation()
                    .map_err(|_| PreparedNativeRefusal::Shape)?,
            ),
            Part::Field(name) => value
                .record_field(name)
                .map_err(|_| PreparedNativeRefusal::Shape)?,
            Part::Index(index) => value
                .collection_index(*index)
                .map_err(|_| PreparedNativeRefusal::Shape)?,
            Part::Variant(tag) => value
                .variant_payload(tag)
                .map_err(|_| PreparedNativeRefusal::Shape)?,
        };
        let Some(next) = next else {
            return Ok(None);
        };
        value = next;
    }
    Ok(Some(value))
}
fn validate_scope(
    scope: &mut Scope<'_>,
    input: &[u8],
    scratch: &mut Vec<u8>,
    maximum: usize,
    index: usize,
) -> Result<(), PreparedNativeRefusal> {
    let value =
        validate_canonical_structured_value(input).map_err(|_| PreparedNativeRefusal::Shape)?;
    let Some(value) = select(value, &scope.path)? else {
        return Ok(());
    };
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
