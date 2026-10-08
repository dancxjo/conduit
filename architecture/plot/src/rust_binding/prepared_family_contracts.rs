//! Static generated contract recipes with admission before reconstruction.
use crate::NativeTypeValueContract;
use alloc::{string::ToString, vec::Vec};
use conduit_core::{
    CheckedValueContract, ConstraintDefinitionError, IntervalEndpoint, ValueConstraint,
};
use core::mem::size_of;

#[derive(Debug)]
pub struct NativeFamilyContractDescriptor {
    pub representation_path: &'static str,
    pub value_kind: &'static str,
    pub maximum_bytes: u32,
    pub constraints: &'static [NativeFamilyConstraintDescriptor],
}

#[derive(Debug)]
pub enum NativeFamilyConstraintDescriptor {
    CanonicalMembership {
        members: &'static [&'static [u8]],
        negated: bool,
    },
    FixedIntegerRange {
        minimum: Option<&'static [u8]>,
        maximum: Option<&'static [u8]>,
        minimum_endpoint: IntervalEndpoint,
        maximum_endpoint: IntervalEndpoint,
    },
}

pub(super) fn storage_bound(recipes: &[NativeFamilyContractDescriptor]) -> Option<usize> {
    let mut bytes = recipes
        .len()
        .checked_mul(size_of::<NativeTypeValueContract>())?;
    for recipe in recipes {
        bytes = bytes
            .checked_add(recipe.representation_path.len())?
            .checked_add(recipe.value_kind.len())?
            .checked_add(
                recipe
                    .constraints
                    .len()
                    .checked_mul(size_of::<ValueConstraint>())?,
            )?;
        for constraint in recipe.constraints {
            let additional = match constraint {
                NativeFamilyConstraintDescriptor::CanonicalMembership { members, .. } => {
                    let mut bound = members.len().checked_mul(size_of::<Vec<u8>>())?;
                    for member in *members {
                        bound = bound.checked_add(member.len())?;
                    }
                    bound
                }
                NativeFamilyConstraintDescriptor::FixedIntegerRange {
                    minimum, maximum, ..
                } => minimum
                    .map_or(0, <[u8]>::len)
                    .checked_add(maximum.map_or(0, <[u8]>::len))?,
            };
            bytes = bytes.checked_add(additional)?;
        }
    }
    Some(bytes)
}

/// Supported recipes' definition checking uses allocation-free primitive decoders.
/// The enclosing family must admit `storage_bound` before calling this function.
pub(super) fn materialize(
    recipes: &[NativeFamilyContractDescriptor],
) -> Result<Vec<NativeTypeValueContract>, ConstraintDefinitionError> {
    let mut contracts = Vec::with_capacity(recipes.len());
    for recipe in recipes {
        let mut constraints = Vec::with_capacity(recipe.constraints.len());
        for constraint in recipe.constraints {
            constraints.push(match constraint {
                NativeFamilyConstraintDescriptor::CanonicalMembership { members, negated } => {
                    let mut values = Vec::with_capacity(members.len());
                    for member in *members {
                        values.push(member.to_vec());
                    }
                    ValueConstraint::CanonicalMembership {
                        members: values,
                        negated: *negated,
                    }
                }
                NativeFamilyConstraintDescriptor::FixedIntegerRange {
                    minimum,
                    maximum,
                    minimum_endpoint,
                    maximum_endpoint,
                } => ValueConstraint::FixedIntegerRange {
                    minimum: minimum.map(<[u8]>::to_vec),
                    maximum: maximum.map(<[u8]>::to_vec),
                    minimum_endpoint: *minimum_endpoint,
                    maximum_endpoint: *maximum_endpoint,
                },
            });
        }
        contracts.push(NativeTypeValueContract {
            representation_path: recipe.representation_path.to_string(),
            contract: CheckedValueContract::new(
                recipe.value_kind.into(),
                recipe.maximum_bytes,
                constraints,
            )?,
        });
    }
    Ok(contracts)
}
