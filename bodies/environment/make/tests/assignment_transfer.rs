//! Framing controls only. Canonical integrity and independent layout admission
//! are exercised by the actual appliance round trip in root_control.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_environment_runtime::appliance::{
    assigned::ResidentSourceIdentity,
    assignment_transfer::{self as t, TransferRefusal as E},
};
const SOURCE: ResidentSourceIdentity = ResidentSourceIdentity {
    expanded_plot_digest: [42; 32],
};
const TEXT: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 1];
const RESIDENT: [c::AssignedRecord<'static>; 1] = [c::AssignedRecord {
    tag: c::ASSIGNED_CONFIGURATION,
    payload: TEXT,
}];
const EMPTY: c::AssignedRecord<'static> = c::AssignedRecord {
    tag: 0,
    payload: &[],
};
fn wire() -> Vec<u8> {
    let records = [
        RESIDENT[0],
        c::AssignedRecord {
            tag: c::ASSIGNED_NODE,
            payload: &[7, 8, 9],
        },
    ];
    let mut bytes = vec![0; t::MAXIMUM_WIRE_BYTES];
    let size = t::encode(
        &[0; c::ASSIGNED_PLAN_HEADER_BYTES],
        &records,
        SOURCE,
        &RESIDENT,
        &mut bytes,
    )
    .unwrap();
    bytes.truncate(size);
    bytes
}
fn refuses(bytes: &[u8], expected: E) {
    let mut slots = [EMPTY; 2];
    let (result, memory) =
        allocation_probe::observe(|| t::decode(bytes, SOURCE, &RESIDENT, &mut slots));
    assert_eq!(result.err(), Some(expected));
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn every_truncation_and_extra_byte_refuses_without_allocation() {
    let bytes = wire();
    for end in 0..bytes.len() {
        refuses(&bytes[..end], E::Frame);
        if end >= 169 {
            let mut truncated = bytes[..end].to_vec();
            truncated[8..10].copy_from_slice(&(end as u16).to_le_bytes());
            refuses(&truncated, E::Frame);
        }
    }
    let mut extra = bytes;
    extra.push(0);
    refuses(&extra, E::Frame);
    let size = extra.len() as u16;
    extra[8..10].copy_from_slice(&size.to_le_bytes());
    refuses(&extra, E::Frame);
}
#[test]
fn source_slots_tags_and_finite_capacities_are_checked() {
    let bytes = wire();
    let mut bad = bytes.clone();
    bad[0] ^= 1;
    refuses(&bad, E::Frame);
    bad = bytes.clone();
    bad[12] ^= 1;
    refuses(&bad, E::Source);
    bad = bytes.clone();
    bad[169] = 2;
    refuses(&bad, E::Frame);
    bad = bytes.clone();
    bad[170] = 1;
    refuses(&bad, E::Reference);
    bad = bytes.clone();
    bad[171..173].copy_from_slice(&[1, 0]);
    refuses(&bad, E::Reference);
    bad = bytes.clone();
    bad[10..12].copy_from_slice(&257u16.to_le_bytes());
    refuses(&bad, E::Capacity);
    refuses(&vec![0; t::MAXIMUM_WIRE_BYTES + 1], E::Capacity);
    let mut slots = [];
    assert_eq!(
        t::decode(&bytes, SOURCE, &RESIDENT, &mut slots).err(),
        Some(E::Capacity)
    );
    let mut slots = [EMPTY; 2];
    assert_eq!(
        t::decode(&bytes, SOURCE, &[], &mut slots).err(),
        Some(E::Resident)
    );
    assert_eq!(
        t::decode(&bytes, SOURCE, &[RESIDENT[0]; 2], &mut slots).err(),
        Some(E::Resident)
    );
    let missing = &bytes[171..];
    let mut no_reference = bytes[..169].to_vec();
    no_reference.extend_from_slice(missing);
    let size = no_reference.len() as u16;
    no_reference[8..10].copy_from_slice(&size.to_le_bytes());
    no_reference[10..12].copy_from_slice(&1u16.to_le_bytes());
    refuses(&no_reference, E::Reference);
}
#[test]
fn encoder_requires_all_resident_records_exactly_once_and_respects_output_bound() {
    let mut bytes = [0; t::MAXIMUM_WIRE_BYTES];
    let header = [0; c::ASSIGNED_PLAN_HEADER_BYTES];
    assert_eq!(
        t::encode(&header, &[RESIDENT[0]; 2], SOURCE, &RESIDENT, &mut bytes),
        Err(E::Reference)
    );
    assert_eq!(
        t::encode(&header, &[EMPTY], SOURCE, &RESIDENT, &mut bytes),
        Err(E::Reference)
    );
    assert_eq!(
        t::encode(&header, &RESIDENT, SOURCE, &RESIDENT, &mut bytes[..170]),
        Err(E::Capacity)
    );
    let huge = vec![0; t::MAXIMUM_WIRE_BYTES];
    assert_eq!(
        t::encode(
            &header,
            &[
                RESIDENT[0],
                c::AssignedRecord {
                    tag: 1,
                    payload: &huge
                }
            ],
            SOURCE,
            &RESIDENT,
            &mut bytes
        ),
        Err(E::Capacity)
    );
    let mut slots = [EMPTY; 2];
    let bytes = wire();
    let decoded = t::decode(&bytes, SOURCE, &RESIDENT, &mut slots).unwrap();
    assert!(core::ptr::eq(
        decoded.records[0].payload.as_ptr(),
        RESIDENT[0].payload.as_ptr()
    ));
    assert_eq!(decoded.records[1].payload, [7, 8, 9]);
}
