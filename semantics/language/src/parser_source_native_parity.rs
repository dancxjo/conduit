//! Full checked Source metadata parity for fixed generated Native roots.
//! A semantic Type alone cannot substitute for its ordered contracts and laws.
use conduit_core::{StructuredInfoType, ValueConstraint};
use conduit_plot::{
    rust_binding::{
        NativeFamilyConstraintDescriptor as Constraint, NativeFamilyTypeDescriptor,
        PreparedNativeFamily,
    },
    CheckedSyntaxDocument, NativeTypeValueContract, PortableExpressionProgram,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SourceNativeParityRefusal {
    Capacity,
    Readiness,
    Pressure,
    Encoding,
    Metadata,
}

/// Checks the whole fixed child closure. All decoding quotas are admitted by
/// an allocation-free first pass, before constructing any temporary owner.
/// The caller separately owns and charges the supplied checked Source document.
pub(crate) fn verify_source_native_parity(
    document: &CheckedSyntaxDocument,
    family: &PreparedNativeFamily,
    roots: &[&'static NativeFamilyTypeDescriptor],
    maximum_temporary_bytes: usize,
) -> Result<usize, SourceNativeParityRefusal> {
    use SourceNativeParityRefusal as R;
    let mut seen: [Option<&'static NativeFamilyTypeDescriptor>; 64] = [None; 64];
    let mut count = 0;
    fn insert(
        seen: &mut [Option<&'static NativeFamilyTypeDescriptor>; 64],
        count: &mut usize,
        item: &'static NativeFamilyTypeDescriptor,
    ) -> Result<(), SourceNativeParityRefusal> {
        if seen[..*count]
            .iter()
            .flatten()
            .any(|prior| core::ptr::eq(*prior, item))
        {
            return Ok(());
        }
        let slot = seen
            .get_mut(*count)
            .ok_or(SourceNativeParityRefusal::Capacity)?;
        *slot = Some(item);
        *count += 1;
        Ok(())
    }
    for root in roots {
        insert(&mut seen, &mut count, root)?;
    }
    let mut required = 0usize;
    let mut cursor = 0;
    while cursor < count {
        let descriptor = seen[cursor].ok_or(R::Metadata)?;
        if !family.contains_descriptor(descriptor) {
            return Err(R::Readiness);
        }
        for child in descriptor.children {
            insert(&mut seen, &mut count, child)?;
        }
        let ty = StructuredInfoType::canonical_decode_storage_bound(descriptor.type_bytes)
            .map_err(|_| R::Encoding)?;
        // Conservative temporary constructor validation allowance until the
        // shared decoder's exact subtree accounting is integrated.
        let ty = ty
            .checked_add(
                descriptor
                    .type_bytes
                    .len()
                    .checked_mul(3)
                    .ok_or(R::Pressure)?,
            )
            .ok_or(R::Pressure)?;
        let mut law = 0;
        for bytes in descriptor.laws {
            law = law.max(
                PortableExpressionProgram::canonical_decode_storage_bound(bytes)
                    .map_err(|_| R::Encoding)?,
            );
        }
        required = required.max(ty.checked_add(law).ok_or(R::Pressure)?);
        cursor += 1;
    }
    if required > maximum_temporary_bytes {
        return Err(R::Pressure);
    }
    for descriptor in seen[..count].iter().flatten() {
        let ty = StructuredInfoType::from_canonical_bytes(descriptor.type_bytes)
            .map_err(|_| R::Encoding)?;
        let mut matches = document
            .native_types
            .iter()
            .filter(|candidate| candidate.value_type == ty);
        let actual = matches.next().ok_or(R::Metadata)?;
        if matches.next().is_some()
            || actual.invariants.len() != descriptor.laws.len()
            || actual.value_contracts.len() != descriptor.contracts.len()
        {
            return Err(R::Metadata);
        }
        for (actual, expected) in actual.value_contracts.iter().zip(descriptor.contracts) {
            if !contract_equal(actual, expected) {
                return Err(R::Metadata);
            }
        }
        for (actual, bytes) in actual.invariants.iter().zip(descriptor.laws) {
            let expected =
                PortableExpressionProgram::from_canonical_bytes(bytes).map_err(|_| R::Encoding)?;
            if actual != &expected {
                return Err(R::Metadata);
            }
        }
    }
    Ok(required)
}

fn contract_equal(
    actual: &NativeTypeValueContract,
    expected: &conduit_plot::rust_binding::NativeFamilyContractDescriptor,
) -> bool {
    actual.representation_path == expected.representation_path
        && actual.contract.value_kind.as_str() == expected.value_kind
        && actual.contract.maximum_bytes == expected.maximum_bytes
        && actual.contract.constraints.len() == expected.constraints.len()
        && actual
            .contract
            .constraints
            .iter()
            .zip(expected.constraints)
            .all(|(actual, expected)| match (actual, expected) {
                (
                    ValueConstraint::CanonicalMembership {
                        members: a,
                        negated: an,
                    },
                    Constraint::CanonicalMembership {
                        members: b,
                        negated: bn,
                    },
                ) => {
                    an == bn
                        && a.len() == b.len()
                        && a.iter().zip(*b).all(|(a, b)| a.as_slice() == *b)
                }
                (
                    ValueConstraint::FixedIntegerRange {
                        minimum: a,
                        maximum: b,
                        minimum_endpoint: c,
                        maximum_endpoint: d,
                    },
                    Constraint::FixedIntegerRange {
                        minimum: e,
                        maximum: f,
                        minimum_endpoint: g,
                        maximum_endpoint: h,
                    },
                ) => a.as_deref() == *e && b.as_deref() == *f && c == g && d == h,
                _ => false,
            })
}
