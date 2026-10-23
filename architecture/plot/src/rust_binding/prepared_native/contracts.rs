//! Prepared exact contract paths; one actual primitive check/backtrack per step.
use super::*;
use crate::NativeTypeValueContract;

pub(super) struct ContractCursor<'d> {
    pub(super) scope: usize,
    pub(super) contract: usize,
    pub(super) path: Vec<Part>,
    indices: Vec<u16>,
    pub(super) latest: Vec<u16>,
    pub(super) latest_generation: u64,
    pub(super) checked: bool,
    source: &'d NativeTypeValueContract,
}

impl ContractCursor<'_> {
    pub(super) fn index_capacity(&self) -> usize {
        self.indices.capacity() + self.latest.capacity()
    }
}

/// Compile all contracts, including inherited flattened ones. No runtime path parsing.
pub(super) fn prepare<'d>(
    scopes: &[Scope<'d>],
    bounds: PreparedNativeBounds,
) -> Result<Vec<ContractCursor<'d>>, PreparedNativeRefusal> {
    if bounds.contracts > 4096 {
        return Err(PreparedNativeRefusal::Bounds);
    }
    let mut result = Vec::new();
    for (scope_index, scope) in scopes.iter().enumerate() {
        for (contract_index, source) in scope.native.value_contracts.iter().enumerate() {
            if result.len() >= bounds.contracts {
                return Err(PreparedNativeRefusal::Bounds);
            }
            let mut path = scope.path.clone();
            compile_path(
                &scope.native.value_type,
                &source.representation_path,
                source.contract.value_kind.as_str(),
                &mut path,
            )?;
            let depth = path.len();
            result.push(ContractCursor {
                scope: scope_index,
                contract: contract_index,
                path,
                indices: alloc::vec![0; depth],
                latest: alloc::vec![0; depth],
                latest_generation: 0,
                checked: false,
                source,
            });
        }
    }
    Ok(result)
}

fn push(path: &mut Vec<Part>, part: Part) -> Result<(), PreparedNativeRefusal> {
    if path.len() >= 64 {
        return Err(PreparedNativeRefusal::Bounds);
    }
    path.push(part);
    Ok(())
}

fn component(path: &str) -> (&str, &str) {
    let end = path
        .char_indices()
        .find_map(|(index, character)| {
            (index > 0 && matches!(character, '.' | '|' | '[' | '?')).then_some(index)
        })
        .unwrap_or(path.len());
    path.split_at(end)
}

/// Every selected edge is checked against the exact retained schema.
fn compile_path(
    mut ty: &StructuredInfoType,
    mut remaining: &str,
    primitive: &str,
    path: &mut Vec<Part>,
) -> Result<(), PreparedNativeRefusal> {
    loop {
        while let Shape::Nominal { representation, .. } = ty.shape() {
            push(path, Part::Representation)?;
            ty = representation;
        }
        if remaining.is_empty() {
            return match ty.shape() {
                Shape::Leaf(kind) if kind.as_str() == primitive => Ok(()),
                _ => Err(PreparedNativeRefusal::InvalidProgram),
            };
        }
        if let Some(rest) = remaining.strip_prefix("[]") {
            ty = match ty.shape() {
                Shape::Collection { element, .. } | Shape::Sequence { element, .. } => element,
                _ => return Err(PreparedNativeRefusal::InvalidProgram),
            };
            push(path, Part::Each)?;
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("?some") {
            if !super::traversal::exact_core_optional(ty) {
                return Err(PreparedNativeRefusal::InvalidProgram);
            }
            let Shape::Variant { cases, .. } = ty.shape() else {
                return Err(PreparedNativeRefusal::InvalidProgram);
            };
            ty = cases
                .iter()
                .find(|case| case.tag() == "some")
                .ok_or(PreparedNativeRefusal::InvalidProgram)?
                .payload_type();
            push(path, Part::Variant("some".into()))?;
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix('.') {
            let (name, rest) = component(rest);
            let Shape::Record { fields, .. } = ty.shape() else {
                return Err(PreparedNativeRefusal::InvalidProgram);
            };
            ty = fields
                .iter()
                .find(|field| field.name() == name)
                .ok_or(PreparedNativeRefusal::InvalidProgram)?
                .value_type();
            push(path, Part::Field(name.into()))?;
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix('|') {
            let (tag, rest) = component(rest);
            let Shape::Variant { cases, .. } = ty.shape() else {
                return Err(PreparedNativeRefusal::InvalidProgram);
            };
            ty = cases
                .iter()
                .find(|case| case.tag() == tag)
                .ok_or(PreparedNativeRefusal::InvalidProgram)?
                .payload_type();
            push(path, Part::Variant(tag.into()))?;
            remaining = rest;
        } else {
            return Err(PreparedNativeRefusal::InvalidProgram);
        }
    }
}

/// Called before native-law traversal. Returns after one check or backtrack.
pub(super) fn step(
    admission: &mut PreparedNativeAdmission<'_>,
) -> Result<(), PreparedNativeRefusal> {
    let index = admission.next_contract;
    let root = validate_canonical_structured_value(&admission.input)
        .map_err(|_| PreparedNativeRefusal::Shape)?;
    let cursor = &mut admission.contracts[index];
    match super::traversal::select(root, &cursor.path, &cursor.indices)? {
        super::traversal::Selection::Value(value) => {
            cursor.latest.copy_from_slice(&cursor.indices);
            cursor.latest_generation = admission.generation;
            cursor.checked = false;
            admission.latest_contract = Some(index);
            // Keep native and contract counts separate, but enforce one total cap.
            let total = admission
                .occurrences
                .checked_add(admission.contract_occurrences)
                .ok_or(PreparedNativeRefusal::Bounds)?;
            if total >= admission.occurrence_limit {
                return Err(PreparedNativeRefusal::Bounds);
            }
            admission.contract_occurrences += 1;
            let bytes = value
                .primitive_bytes(cursor.source.contract.value_kind.as_str())
                .map_err(|_| PreparedNativeRefusal::Shape)?;
            // `checked` means attempted; false primitive-law result remains evidence.
            cursor.checked = true;
            cursor.source.contract.validate(bytes).map_err(|refusal| {
                PreparedNativeRefusal::Contract {
                    scope: cursor.scope,
                    contract: cursor.contract,
                    refusal,
                }
            })?;
            if !super::traversal::advance(&cursor.path, &mut cursor.indices, cursor.path.len())? {
                admission.next_contract += 1;
            }
        }
        super::traversal::Selection::MissingBefore(position) => {
            if !super::traversal::advance(&cursor.path, &mut cursor.indices, position)? {
                admission.next_contract += 1;
            }
        }
    }
    Ok(())
}

pub(super) fn reset(cursors: &mut [ContractCursor<'_>]) {
    for cursor in cursors {
        cursor.indices.fill(0);
        // Also generation-gate public evidence; no stale output for absent alternatives.
        cursor.checked = false;
    }
}
