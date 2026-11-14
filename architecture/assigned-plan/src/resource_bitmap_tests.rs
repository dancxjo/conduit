//! Exact multiset admission across the existing one-byte resource record domain.
use super::*;
use crate::record_view_tests::allocation_probe;
use std::{vec, vec::Vec};

fn image(resources: &[u16]) -> Vec<u8> {
    assert!(resources.len() <= u8::MAX as usize);
    let mut bytes = vec![0; ASSIGNED_PLAN_HEADER_BYTES];
    bytes[..8].copy_from_slice(&assigned_plan_magic());
    bytes[8..10].copy_from_slice(&ASSIGNED_PLAN_SCHEMA.to_le_bytes());
    bytes[48..64].copy_from_slice(&AssignedIdentity::from_text("host").0);
    bytes[64..80].copy_from_slice(&AssignedIdentity::from_text("boot").0);
    bytes[80 + usize::from(ASSIGNED_RESOURCE - 1)] = resources.len() as u8;
    for (node, resource) in resources.iter().enumerate() {
        bytes.push(ASSIGNED_RESOURCE);
        bytes.extend_from_slice(&8u16.to_le_bytes());
        bytes.extend_from_slice(&(node as u16).to_le_bytes());
        bytes.extend_from_slice(&resource.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
    }
    let length = u16::try_from(bytes.len()).unwrap();
    bytes[10..12].copy_from_slice(&length.to_le_bytes());
    let digest = assigned_plan_payload_digest(&bytes[ASSIGNED_PLAN_HEADER_BYTES..]);
    bytes[93..125].copy_from_slice(&digest);
    bytes
}
fn decode(bytes: &[u8], resources: &[u16]) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    let required = AssignedPlanRequirements {
        host: AssignedIdentity::from_text("host"),
        boot: AssignedIdentity::from_text("boot"),
        host_calls: &[],
        resources,
        remote_bindings: &[],
    };
    let maxima = AssignedPlanMaxima {
        encoded_bytes: u16::MAX,
        runtime_state_bytes: 1,
        counts: [u8::MAX; ASSIGNED_PLAN_COUNT_KINDS],
    };
    let (result, allocations) =
        allocation_probe::observe(|| decode_assigned_plan(bytes, maxima, required));
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    result
}
#[test]
fn original_storage_accepts_49_equal_resource_requirements_and_255_boundary() {
    assert_eq!(RESOURCE_SEEN_BYTES, 32);
    assert_eq!(
        core::mem::size_of::<[u8; RESOURCE_SEEN_BYTES]>(),
        core::mem::size_of::<[bool; 32]>()
    );
    let repeated = [0; 49];
    assert!(decode(&image(&repeated), &repeated).is_ok());
    let full = [0; 255];
    assert!(decode(&image(&full), &full).is_ok());
    let excess = [0; 256];
    assert_eq!(
        decode(&image(&full), &excess),
        Err(AssignedPlanRefusal::MalformedRecord)
    );
}
#[test]
fn late_bits_preserve_unordered_exact_multiset_matching_and_missing_checks() {
    let required: Vec<_> = (0..255).map(|n| n as u16).collect();
    let mut reverse = required.clone();
    reverse.reverse();
    assert!(decode(&image(&reverse), &required).is_ok());
    // The final inventory bit must be independently marked, never aliased to
    // another byte/bit by a narrow counter or silently inferred from counts.
    assert_eq!(
        decode(&image(&required[..254]), &required),
        Err(AssignedPlanRefusal::MissingResource)
    );
    let mut duplicate = required.clone();
    duplicate[254] = 253;
    assert_eq!(
        decode(&image(&duplicate), &required),
        Err(AssignedPlanRefusal::UnknownResource)
    );
    let mut unexpected = required.clone();
    unexpected[254] = 999;
    assert_eq!(
        decode(&image(&unexpected), &required),
        Err(AssignedPlanRefusal::UnknownResource)
    );
}
#[test]
fn repeated_ordinals_are_consumed_once_per_requirement_and_payload_digest_stays_exact() {
    let required = [7; 49];
    assert_eq!(
        decode(&image(&required[..48]), &required),
        Err(AssignedPlanRefusal::MissingResource)
    );
    assert_eq!(
        decode(&image(&[7; 50]), &required),
        Err(AssignedPlanRefusal::UnknownResource)
    );
    let mut changed = image(&required);
    changed[ASSIGNED_PLAN_HEADER_BYTES + 5] ^= 1;
    assert_eq!(
        decode(&changed, &required),
        Err(AssignedPlanRefusal::DigestMismatch)
    );
}
