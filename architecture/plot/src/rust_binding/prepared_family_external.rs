//! Allocation-free, bounded comparison with generated owning-crate metadata.
use super::prepared_family::{
    NativeFamilyTypeDescriptor, PreparedNativeFamilyRefusal as Refusal, MAXIMUM_NATIVE_FAMILY_TYPES,
};
use super::prepared_family_contracts::NativeFamilyConstraintDescriptor;
use conduit_core::{MAXIMUM_STRUCTURED_CANONICAL_BYTES, MAXIMUM_STRUCTURED_INFO_NODES};

/// Bound the complete metadata graph before a generator copies any recipes.
pub(super) fn metadata_extent(root: &'static NativeFamilyTypeDescriptor) -> Result<usize, Refusal> {
    let ceiling = super::prepared_family::MAXIMUM_NATIVE_FAMILY_EXTERNAL_METADATA_BYTES;
    let mut budget = ceiling;
    let mut descriptors = [None; MAXIMUM_NATIVE_FAMILY_TYPES];
    descriptors[0] = Some(root);
    let mut count = 1;
    let mut next = 0;
    while next < count {
        let descriptor = descriptors[next].ok_or(Refusal::ConflictingDescriptor)?;
        next += 1;
        bounded(descriptor, &mut budget)?;
        validate_edges(descriptor, &mut budget)?;
        for &child in descriptor.children {
            if !descriptors[..count]
                .iter()
                .flatten()
                .any(|prior| core::ptr::eq(*prior, child))
            {
                if count == descriptors.len() {
                    return Err(Refusal::Capacity);
                }
                descriptors[count] = Some(child);
                count += 1;
            }
        }
    }
    Ok(ceiling - budget)
}

fn charge(budget: &mut usize, bytes: usize) -> Result<(), Refusal> {
    *budget = budget.checked_sub(bytes).ok_or(Refusal::Capacity)?;
    Ok(())
}

fn extent(budget: &mut usize, bytes: &[u8]) -> Result<(), Refusal> {
    if bytes.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
        return Err(Refusal::Capacity);
    }
    charge(budget, bytes.len().saturating_add(1))
}

fn bounded(descriptor: &NativeFamilyTypeDescriptor, budget: &mut usize) -> Result<(), Refusal> {
    if descriptor.children.len() > MAXIMUM_NATIVE_FAMILY_TYPES
        || descriptor.external_edges.len() > MAXIMUM_NATIVE_FAMILY_TYPES
        || descriptor.contracts.len() > MAXIMUM_STRUCTURED_INFO_NODES
    {
        return Err(Refusal::Capacity);
    }
    charge(budget, core::mem::size_of::<NativeFamilyTypeDescriptor>())?;
    extent(budget, descriptor.type_bytes)?;
    // Law count and expression-program extent are independent of Type count
    // and canonical Type bytes. Admit slice traversal against the static work
    // budget before visiting any law; the caller's law quota is checked later.
    charge(
        budget,
        descriptor
            .laws
            .len()
            .checked_mul(core::mem::size_of::<&[u8]>())
            .ok_or(Refusal::Capacity)?,
    )?;
    for law in descriptor.laws {
        if law.len() > crate::expression_program::MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES {
            return Err(Refusal::Capacity);
        }
        charge(budget, law.len().checked_add(1).ok_or(Refusal::Capacity)?)?;
    }
    for contract in descriptor.contracts {
        charge(budget, core::mem::size_of_val(contract))?;
        extent(budget, contract.representation_path.as_bytes())?;
        extent(budget, contract.value_kind.as_bytes())?;
        if contract.constraints.len() > MAXIMUM_STRUCTURED_INFO_NODES {
            return Err(Refusal::Capacity);
        }
        for constraint in contract.constraints {
            charge(budget, core::mem::size_of_val(constraint))?;
            match constraint {
                NativeFamilyConstraintDescriptor::CanonicalMembership { members, .. } => {
                    if members.len() > MAXIMUM_STRUCTURED_INFO_NODES {
                        return Err(Refusal::Capacity);
                    }
                    for member in *members {
                        extent(budget, member)?;
                    }
                }
                NativeFamilyConstraintDescriptor::FixedIntegerRange {
                    minimum, maximum, ..
                } => {
                    if let Some(value) = minimum {
                        extent(budget, value)?;
                    }
                    if let Some(value) = maximum {
                        extent(budget, value)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn compare(
    actual: &'static NativeFamilyTypeDescriptor,
    expected: &'static NativeFamilyTypeDescriptor,
    budget: &mut usize,
) -> Result<(), Refusal> {
    let mut pairs = [None; MAXIMUM_NATIVE_FAMILY_TYPES];
    pairs[0] = Some((actual, expected));
    let mut count = 1;
    let mut next = 0;
    while next < count {
        let (actual, expected) = pairs[next].ok_or(Refusal::ConflictingDescriptor)?;
        next += 1;
        bounded(actual, budget)?;
        bounded(expected, budget)?;
        // Shadows contain the complete original child graph; they never grant
        // external conversion or introduce a second chain of expected metadata.
        if !expected.external_edges.is_empty()
            || actual.type_bytes != expected.type_bytes
            || actual.laws != expected.laws
            || actual.contracts != expected.contracts
            || actual.conversion_profile != expected.conversion_profile
            || actual.children.len() != expected.children.len()
        {
            return Err(Refusal::ConflictingDescriptor);
        }
        for (&child, &shadow) in actual.children.iter().zip(expected.children) {
            let mut found = false;
            for &(prior, prior_shadow) in pairs[..count].iter().flatten() {
                if core::ptr::eq(prior, child) {
                    if !core::ptr::eq(prior_shadow, shadow) {
                        return Err(Refusal::ConflictingDescriptor);
                    }
                    found = true;
                    break;
                }
            }
            if !found {
                if count == pairs.len() {
                    return Err(Refusal::Capacity);
                }
                pairs[count] = Some((child, shadow));
                count += 1;
            }
        }
    }
    Ok(())
}

pub(super) fn validate_edges(
    descriptor: &'static NativeFamilyTypeDescriptor,
    budget: &mut usize,
) -> Result<(), Refusal> {
    if descriptor.external_edges.len() > MAXIMUM_NATIVE_FAMILY_TYPES {
        return Err(Refusal::Capacity);
    }
    for edge in descriptor.external_edges {
        charge(budget, core::mem::size_of_val(edge))?;
        if !descriptor
            .children
            .iter()
            .any(|child| core::ptr::eq(*child, edge.descriptor))
        {
            return Err(Refusal::ConflictingDescriptor);
        }
        compare(edge.descriptor, edge.expected, budget)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rust_binding::NativeFamilyConversionProfile;
    static TYPE: &[u8] = &[
        5, 1, 0, 0, 0, b'n', 0, 9, 0, 0, 0, b'v', b'a', b'l', b'u', b'e', b'/', b'u', b'6', b'4',
    ];
    fn descriptor(laws: &'static [&'static [u8]]) -> &'static NativeFamilyTypeDescriptor {
        alloc::boxed::Box::leak(alloc::boxed::Box::new(NativeFamilyTypeDescriptor {
            type_bytes: TYPE,
            laws,
            contracts: &[],
            children: &[],
            external_edges: &[],
            conversion_profile: NativeFamilyConversionProfile::Nominal,
            maximum_inline_bytes: 0,
        }))
    }
    #[test]
    fn metadata_observation_does_not_apply_type_count_to_law_count() {
        static LAWS: [&[u8]; 65] = [&[]; 65];
        assert!(metadata_extent(descriptor(&LAWS)).is_ok());
    }
    #[test]
    fn metadata_observation_does_not_apply_type_byte_extent_to_program_bytes() {
        static PROGRAM: [u8; MAXIMUM_STRUCTURED_CANONICAL_BYTES + 1] =
            [0; MAXIMUM_STRUCTURED_CANONICAL_BYTES + 1];
        static LAWS: [&[u8]; 1] = [&PROGRAM];
        // This observer only bounds static work. Actual expression decoding and
        // law validation remain required, and would reject these dummy bytes.
        assert!(metadata_extent(descriptor(&LAWS)).is_ok());
    }
}
