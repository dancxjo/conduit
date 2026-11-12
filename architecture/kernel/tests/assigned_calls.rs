//! Numeric table capacity only; this fixture issues no operation authority.
use conduit_assigned_plan as a;
use conduit_kernel::{assigned::*, *};
struct Fixture {
    bytes: Vec<u8>,
    counts: [u8; a::ASSIGNED_PLAN_COUNT_KINDS],
    pins: Vec<a::AssignedRecordRequirement>,
    calls: Vec<a::AssignedIdentity>,
}
impl Fixture {
    fn new(calls: &[(u16, u16)]) -> Self {
        let mut records = Vec::new();
        for node in 0u16..2 {
            let mut payload = vec![0; 52];
            payload[..2].copy_from_slice(&node.to_le_bytes());
            payload[2..4].copy_from_slice(&8u16.to_le_bytes());
            records.push((a::ASSIGNED_NODE, payload));
        }
        let mut required = Vec::new();
        for (index, (node, call)) in calls.iter().enumerate() {
            let id = a::AssignedIdentity::from_text(&format!("operation/{index}"));
            required.push(id);
            let mut payload = vec![0; 46];
            payload[..2].copy_from_slice(&node.to_le_bytes());
            payload[2..4].copy_from_slice(&call.to_le_bytes());
            payload[4..20].copy_from_slice(&id.0);
            payload[36..38].copy_from_slice(&1u16.to_le_bytes());
            payload[38..42].copy_from_slice(&1u32.to_le_bytes());
            payload[42..46].copy_from_slice(&1u32.to_le_bytes());
            records.push((a::ASSIGNED_HOST_CALL, payload));
        }
        for node in 0u16..2 {
            let mut payload = vec![1];
            payload.extend_from_slice(&node.to_le_bytes());
            records.push((a::ASSIGNED_STARTUP, payload));
        }
        let mut bytes = vec![0; a::ASSIGNED_PLAN_HEADER_BYTES];
        bytes[..8].copy_from_slice(&a::assigned_plan_magic());
        bytes[8..10].copy_from_slice(&a::ASSIGNED_KERNEL_PLAN_SCHEMA.to_le_bytes());
        bytes[12..14].copy_from_slice(&256u16.to_le_bytes());
        bytes[48..64].copy_from_slice(&a::AssignedIdentity::from_text("host").0);
        bytes[64..80].copy_from_slice(&a::AssignedIdentity::from_text("boot").0);
        let mut counts = [0; a::ASSIGNED_PLAN_COUNT_KINDS];
        let mut pins = Vec::new();
        for (tag, payload) in records {
            counts[usize::from(tag - 1)] += 1;
            pins.push(a::AssignedRecordRequirement::from_record(
                a::AssignedRecord {
                    tag,
                    payload: &payload,
                },
            ));
            bytes.push(tag);
            bytes.extend_from_slice(&(payload.len() as u16).to_le_bytes());
            bytes.extend_from_slice(&payload);
        }
        let length = bytes.len() as u16;
        bytes[10..12].copy_from_slice(&length.to_le_bytes());
        bytes[80..93].copy_from_slice(&counts);
        let digest = a::assigned_plan_payload_digest(&bytes[a::ASSIGNED_PLAN_HEADER_BYTES..]);
        bytes[93..125].copy_from_slice(&digest);
        Self {
            bytes,
            counts,
            pins,
            calls: required,
        }
    }
    fn prepare<const H: usize>(
        &self,
    ) -> Result<AssignedKernelTables<2, 0, 1, 0, 0, H>, AssignedKernelRefusal> {
        AssignedKernelTables::prepare(
            &self.bytes,
            a::AssignedPlanMaxima::TINY_HOST,
            a::AssignedPlanRequirements {
                host: a::AssignedIdentity::from_text("host"),
                boot: a::AssignedIdentity::from_text("boot"),
                host_calls: &self.calls,
                resources: &[],
                remote_bindings: &[],
            },
            a::AssignedComputationProfile {
                maximum_text_bytes: 0,
                maximum_total_text_bytes: 0,
                requirements: &[],
            },
            &ResidentLayout {
                records: &self.pins,
                counts: self.counts,
            },
        )
    }
}
fn input() -> BoundedValueRef {
    BoundedValueRef::new(
        ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 1,
        },
        1,
    )
    .unwrap()
}
#[test]
fn zero_calls_and_existing_partial_single_call_rows_preserve_their_capacities() {
    assert!(Fixture::new(&[]).prepare::<0>().is_ok());
    let single = Fixture::new(&[(0, 0)]).prepare::<1>().unwrap();
    assert!(single
        .calls
        .admit_request(NodeId(0), HostCallId(0), input())
        .is_ok());
    assert!(single
        .calls
        .admit_request(NodeId(1), HostCallId(0), input())
        .is_err());
    let tail = Fixture::new(&[(1, 0)]).prepare::<2>().unwrap();
    assert!(tail
        .calls
        .admit_request(NodeId(1), HostCallId(0), input())
        .is_ok());
    assert!(Fixture::new(&[(1, 0)]).prepare::<1>().is_err());
}
#[test]
fn two_checked_calls_use_only_the_existing_funded_slots() {
    let f = Fixture::new(&[(0, 0), (0, 1)]);
    let table = f.prepare::<2>().unwrap();
    for call in [0, 1] {
        assert!(table
            .calls
            .admit_request(NodeId(0), HostCallId(call), input())
            .is_ok());
    }
    assert!(table
        .calls
        .admit_request(NodeId(0), HostCallId(2), input())
        .is_err());
    assert!(matches!(
        f.prepare::<1>(),
        Err(AssignedKernelRefusal::Capacity)
    ));
}
#[test]
fn sparse_overflowing_and_duplicate_call_ids_never_enlarge_table_or_hide_collision() {
    let mut foreign = Fixture::new(&[(0, 0), (0, 1)]);
    foreign.calls[1] = a::AssignedIdentity::from_text("operation/foreign");
    assert!(matches!(
        foreign.prepare::<2>(),
        Err(AssignedKernelRefusal::Projection(
            a::AssignedPlanRefusal::UnknownHostCall
        ))
    ));

    let sparse = Fixture::new(&[(0, 2)]);
    assert!(sparse
        .prepare::<3>()
        .unwrap()
        .calls
        .admit_request(NodeId(0), HostCallId(2), input())
        .is_ok());
    assert!(matches!(
        sparse.prepare::<2>(),
        Err(AssignedKernelRefusal::Capacity)
    ));
    assert!(matches!(
        Fixture::new(&[(0, u16::MAX)]).prepare::<4>(),
        Err(AssignedKernelRefusal::Capacity)
    ));
    assert!(matches!(
        Fixture::new(&[(0, 0), (0, 0)]).prepare::<2>(),
        Err(AssignedKernelRefusal::Kernel(
            ProtocolError::HostCallTableInvalid
        ))
    ));
}
