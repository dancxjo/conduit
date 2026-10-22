//! Prepared schema templates and bounded actual canonical occurrence traversal.
use super::*;
pub(super) fn lookup<'d>(
    document: &'d CheckedSyntaxDocument,
    identity: &KindId,
) -> Result<&'d CheckedNativeType, PreparedNativeRefusal> {
    // Only the private catalogue retained by syntax checking grants authority.
    // Public inspection metadata may detect a conflict, never add a new owner.
    let result = document
        .retained_native_types
        .iter()
        .find(|native| &native.identity == identity)
        .ok_or(PreparedNativeRefusal::MissingNativeMetadata)?;
    for native in document
        .retained_native_types
        .iter()
        .chain(document.native_types.iter())
        .filter(|native| &native.identity == identity)
    {
        if result.value_type != native.value_type
            || result.value_contracts != native.value_contracts
            || result.invariants != native.invariants
        {
            return Err(PreparedNativeRefusal::ConflictingNativeMetadata);
        }
    }
    Ok(result)
}
pub(super) fn prepare<'d>(
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
        let native = match lookup(document, identity) {
            Ok(native) => Some(native),
            Err(PreparedNativeRefusal::MissingNativeMetadata) if exact_core_optional(ty) => None,
            Err(reason) => return Err(reason),
        };
        if let Some(native) = native {
            if scopes.len() >= bounds.scopes {
                return Err(PreparedNativeRefusal::Bounds);
            }
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
                indices: alloc::vec![0;path.len()],
                latest: alloc::vec![0;path.len()],
                latest_generation: 0,
                checked_invariants: 0,
                native,
                evaluators,
            });
        }
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
        Shape::Collection { element, .. } | Shape::Sequence { element, .. } => {
            path.push(Part::Each);
            prepare(document, element, path, scopes, bounds, count)?;
            path.pop();
        }
    }
    Ok(())
}
// Core owns the complete T? schema. Equality to its constructor is required;
// merely recognizing a schema label or matching some/none names is insufficient.
fn exact_core_optional(ty: &StructuredInfoType) -> bool {
    let Shape::Variant { cases, .. } = ty.shape() else {
        return false;
    };
    let Some(some) = cases.iter().find(|case| case.tag() == "some") else {
        return false;
    };
    conduit_core::optional_info_type(some.payload_type().clone())
        .is_ok_and(|expected| &expected == ty)
}

enum Selection<'a> {
    Value(ValidatedCanonicalStructuredValue<'a>),
    /// This alternative cannot exist; carry to an earlier enclosing Each.
    MissingBefore(usize),
}
fn select<'a>(
    mut value: ValidatedCanonicalStructuredValue<'a>,
    path: &[Part],
    indices: &[u16],
) -> Result<Selection<'a>, PreparedNativeRefusal> {
    for (position, part) in path.iter().enumerate() {
        value = match part {
            Part::Representation => value
                .representation()
                .map_err(|_| PreparedNativeRefusal::Shape)?,
            Part::Field(name) => value
                .record_field(name)
                .map_err(|_| PreparedNativeRefusal::Shape)?
                .ok_or(PreparedNativeRefusal::Shape)?,
            Part::Each => match value
                .collection_index(indices[position])
                .map_err(|_| PreparedNativeRefusal::Shape)?
            {
                Some(value) => value,
                None => return Ok(Selection::MissingBefore(position)),
            },
            Part::Variant(tag) => match value
                .variant_payload(tag)
                .map_err(|_| PreparedNativeRefusal::Shape)?
            {
                Some(value) => value,
                None => return Ok(Selection::MissingBefore(position)),
            },
        };
    }
    Ok(Selection::Value(value))
}
fn advance(
    path: &[Part],
    indices: &mut [u16],
    before: usize,
) -> Result<bool, PreparedNativeRefusal> {
    for position in (0..before).rev() {
        if matches!(path[position], Part::Each) {
            indices[position] = indices[position]
                .checked_add(1)
                .ok_or(PreparedNativeRefusal::Bounds)?;
            indices[position + 1..].fill(0);
            return Ok(true);
        }
    }
    Ok(false)
}
fn next_scope(admission: &mut PreparedNativeAdmission<'_>) {
    admission.next += 1;
    if let Some(scope) = admission.scopes.get_mut(admission.next) {
        scope.indices.fill(0);
    }
}
/// A single actual occurrence or bounded backtrack. No maximum Cartesian walk.
pub(super) fn step(
    admission: &mut PreparedNativeAdmission<'_>,
) -> Result<(), PreparedNativeRefusal> {
    let index = admission.next;
    let root = validate_canonical_structured_value(&admission.input)
        .map_err(|_| PreparedNativeRefusal::Shape)?;
    let scope = &mut admission.scopes[index];
    match select(root, &scope.path, &scope.indices)? {
        Selection::Value(value) => {
            scope.latest.copy_from_slice(&scope.indices);
            scope.latest_generation = admission.generation;
            scope.checked_invariants = 0;
            admission.latest_scope = Some(index);
            if admission.occurrences == admission.occurrence_limit {
                return Err(PreparedNativeRefusal::Bounds);
            }
            admission.occurrences += 1;
            validate_scope(
                scope,
                value,
                &mut admission.scratch,
                admission.maximum,
                index,
            )?;
            if !advance(&scope.path, &mut scope.indices, scope.path.len())? {
                next_scope(admission);
            }
        }
        Selection::MissingBefore(position) => {
            if !advance(&scope.path, &mut scope.indices, position)? {
                next_scope(admission);
            }
        }
    }
    Ok(())
}
