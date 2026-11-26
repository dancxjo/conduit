//! Owning-model preparation bound for the explicitly selected generic Mask.
//! This intentionally excludes configured/recursive Plan families. Fence before
//! deserializing: a post-parse refusal would not bound constructor workspace.
use crate::json_model_preflight::{generic_mask_plan_json_shape, JsonModelShape};
use alloc::vec::Vec;
use core::mem::size_of;

pub(super) fn generic_preparation_bytes(bytes: &[u8]) -> Result<usize, &'static str> {
    if bytes.is_empty() || bytes.len() > super::MAX_EXPANDED_BYTES {
        return Err("generic Mask Plan expanded capacity");
    }
    let shape =
        generic_mask_plan_json_shape(bytes).map_err(|_| "unsupported generic Mask Plan model")?;
    model_preparation_bytes(shape).ok_or("generic Mask Plan storage overflow")
}

/// Conditional on the generic-family preparse fence above. Includes owning Vec
/// growth, boxed fixed Quantities, all strings and parser scratch. Excludes the
/// expanded input, inflater, caller clones and subsequent validation workspace.
pub(super) fn model_preparation_bytes(shape: JsonModelShape) -> Option<usize> {
    use conduit_core::*;
    let largest = [
        size_of::<PlanFragment>(),
        size_of::<PlannedGear>(),
        size_of::<PlannedConnection>(),
        size_of::<PlannedForePort>(),
        size_of::<PortDescriptor>(),
        size_of::<HostCallRequirement>(),
        size_of::<ResourceBinding>(),
        size_of::<AuthorityBinding>(),
        size_of::<AdmittedLine>(),
        size_of::<StartupDependency>(),
        size_of::<ExpectedTerminal>(),
        size_of::<ExpectedSign>(),
        size_of::<FragmentCommitment>(),
        size_of::<PlacementId>(),
        size_of::<CheckedValueContract>(),
        size_of::<ValueConstraint>(),
        size_of::<TextPatternState>(),
        size_of::<TextPatternTransition>(),
        size_of::<Vec<u8>>(),
        size_of::<alloc::string::String>(),
    ]
    .into_iter()
    .max()?;
    let complex = shape
        .object_or_string_array_elements
        .checked_mul(2)?
        .checked_add(shape.arrays.checked_mul(4)?)?
        .checked_mul(largest)?;
    // Numeric arrays only own bytes; use eight bytes per scalar conservatively,
    // including Vec's minimum first allocation for each array.
    let numeric = shape
        .numeric_array_elements
        .checked_mul(2)?
        .checked_add(shape.arrays.checked_mul(8)?)?
        .checked_mul(8)?;
    let boxes = shape.arrays.checked_mul(size_of::<Quantity>())?;
    let strings_and_scratch = shape.string_source_bytes.checked_mul(4)?;
    size_of::<Plan>()
        .checked_add(complex)?
        .checked_add(numeric)?
        .checked_add(boxes)?
        .checked_add(strings_and_scratch)?
        .checked_add(8)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generic_fence_accepts_ordinary_owners_and_refuses_custom_families_before_parse() {
        assert!(generic_preparation_bytes(br#"{"fragments":[{"placements":[{"configuration":[],"semantic_contract":{"configuration":[],"laws":[]},"resources":[{"protected":null,"compute":null}]}]}]}"#).is_ok());
        for bytes in [
            br#"{"activations":[{}]}"#.as_slice(),
            br#"{"configuration":[{"value":{"Structured":{}}}]}"#,
            br#"{"semantic_contract":{"laws":["custom"]}}"#,
            br#"{"resources":[{"content":{}}]}"#,
            br#"{"execution_regions":[{}]}"#,
            br#"{"base":{}}"#,
        ] {
            assert!(generic_preparation_bytes(bytes).is_err());
        }
        assert!(generic_preparation_bytes(br#"{"\u0063onfiguration":[{}]}"#).is_err());
    }
    #[test]
    fn model_bound_fences_overflow() {
        assert!(model_preparation_bytes(JsonModelShape::default()).is_some());
        assert!(model_preparation_bytes(JsonModelShape {
            arrays: usize::MAX,
            ..Default::default()
        })
        .is_none());
    }
}
