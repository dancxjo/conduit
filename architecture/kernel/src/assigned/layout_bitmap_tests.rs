//! Layout multiset checks after Core's separate canonical record validation.
//! Shapes mirror canonical tag counts; these fixtures do not grant a Plan/Play.
extern crate std;
use super::*;
use std::{vec, vec::Vec};

type Row = (u8, [u8; 8]);
fn rows(counts: [u8; c::ASSIGNED_PLAN_COUNT_KINDS]) -> Vec<Row> {
    counts
        .iter()
        .enumerate()
        .flat_map(|(index, count)| {
            (0..*count).map(move |ordinal| {
                let mut payload = [0; 8];
                payload[0] = ordinal;
                // Equal ordinals intentionally have equal payloads across tags.
                (index as u8 + 1, payload)
            })
        })
        .collect()
}
fn requirements(data: &[Row]) -> Vec<c::AssignedRecordRequirement> {
    data.iter()
        .filter(|r| !ignored(r.0))
        .map(|(tag, payload)| {
            c::AssignedRecordRequirement::from_kernel_record(
                c::AssignedRecord { tag: *tag, payload },
                c::ASSIGNED_KERNEL_PLAN_SCHEMA,
            )
        })
        .collect()
}
fn check(
    data: &[Row],
    required: &[c::AssignedRecordRequirement],
) -> Result<(), AssignedKernelRefusal> {
    let records: Vec<_> = data
        .iter()
        .map(|(tag, payload)| c::AssignedRecord { tag: *tag, payload })
        .collect();
    let mut header = [0; c::ASSIGNED_PLAN_HEADER_BYTES];
    header[8..10].copy_from_slice(&c::ASSIGNED_KERNEL_PLAN_SCHEMA.to_le_bytes());
    validate(
        RecordInput::Records {
            header: &header,
            records: &records,
        },
        &ResidentLayout {
            records: required,
            counts: [0; c::ASSIGNED_PLAN_COUNT_KINDS],
        },
        None,
    )
}
#[test]
fn original_workspace_covers_721_source_shape_and_complete_canonical_domain() {
    assert_eq!(SEEN_BYTES, core::mem::size_of::<[bool; 256]>());
    assert_eq!(TAG_BYTES, 32);
    assert_eq!(TAGS_PER_PASS, 8);
    let source = rows([76, 162, 64, 95, 76, 92, 2, 49, 0, 12, 157, 160, 0]);
    let required = requirements(&source);
    assert_eq!(required.len(), 721);
    assert_eq!(check(&source, &required), Ok(()));
    let mut complete = rows([255; c::ASSIGNED_PLAN_COUNT_KINDS]);
    let mut required = requirements(&complete);
    assert_eq!(complete.len(), 3315);
    assert_eq!(required.len(), 2550); // All nonignored tags, beyond2048.
    assert_eq!(check(&complete, &required), Ok(()));
    complete.reverse();
    required.rotate_left(1001);
    assert_eq!(check(&complete, &required), Ok(()));
}
#[test]
fn exact_duplicate_multiplicity_and_tag_identity_survive_both_groups() {
    let first = (c::ASSIGNED_NODE, [7; 8]);
    let second = (c::ASSIGNED_STARTUP, [7; 8]);
    let data = vec![first, second, first, second];
    let required = requirements(&data);
    assert_eq!(check(&data, &required), Ok(()));
    assert_eq!(
        check(&data[..3], &required),
        Err(AssignedKernelRefusal::Layout)
    );
    let mut excess = data.clone();
    excess.push(second);
    assert_eq!(
        check(&excess, &required),
        Err(AssignedKernelRefusal::Layout)
    );
    let mut changed_tag = data.clone();
    changed_tag[3].0 = c::ASSIGNED_NODE;
    assert_eq!(
        check(&changed_tag, &required),
        Err(AssignedKernelRefusal::Layout)
    );
    let mut changed_payload = data.clone();
    changed_payload[3].1[7] ^= 1;
    assert_eq!(
        check(&changed_payload, &required),
        Err(AssignedKernelRefusal::Layout)
    );
}
#[test]
fn per_tag_255_boundary_and_unmatchable_requirements_remain_checked() {
    let data = rows([255; c::ASSIGNED_PLAN_COUNT_KINDS]);
    let required = requirements(&data);
    let mut excess = required.clone();
    excess.push(required[0]);
    assert_eq!(check(&data, &excess), Err(AssignedKernelRefusal::Capacity));
    let mut foreign = required.clone();
    foreign[0].tag = 14;
    assert_eq!(check(&data, &foreign), Err(AssignedKernelRefusal::Layout));
    foreign[0].tag = 0;
    assert_eq!(check(&data, &foreign), Err(AssignedKernelRefusal::Layout));
    foreign[0].tag = c::ASSIGNED_CONFIGURATION;
    assert_eq!(check(&data, &foreign), Err(AssignedKernelRefusal::Layout));
    let mut wrong_digest = required.clone();
    wrong_digest[2048].payload_digest[31] ^= 1;
    assert_eq!(
        check(&data, &wrong_digest),
        Err(AssignedKernelRefusal::Layout)
    );
    assert_eq!(
        check(&data, &required[..required.len() - 1]),
        Err(AssignedKernelRefusal::Layout)
    );
}
