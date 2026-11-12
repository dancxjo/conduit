//! Same bounded queue mechanics; only checked schema7 Fore has local custody.
use conduit_assigned_plan as a;
use conduit_kernel::{assigned::*, scheduler::*, *};

#[derive(Clone, Copy)]
struct Echo;
impl StepBack<1> for Echo {
    fn step(&mut self, io: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        if io.input(PortId(0)).is_some() {
            let value = io.consume(PortId(0)).unwrap();
            io.send(PortId(0), value).unwrap();
            StepOutcome::Progress
        } else if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0)).unwrap();
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
}
type Tables = AssignedKernelTables<1, 2, 1, 1, 1, 1>;
type Kernel = FixedScheduler<Echo, FixedValueStore<1, 4>, FixedSignLog<16>, 1, 2, 1, 2, 1, 1>;
fn capacity(slot_start: u16) -> CordCapacity {
    CordCapacity {
        slot_start,
        item_capacity: 1,
        byte_capacity: 4,
        pressure_policy: Default::default(),
    }
}
fn prepared() -> Tables {
    let fore = [
        a::AssignedForeRequirement::new(
            0,
            0,
            a::AssignedForeDirection::Input,
            a::AssignedIdentity::from_text("input"),
            [1; 32],
        ),
        a::AssignedForeRequirement::new(
            1,
            1,
            a::AssignedForeDirection::Output,
            a::AssignedIdentity::from_text("output"),
            [2; 32],
        ),
    ];
    let mut node = [0u8; 52];
    node[2..4].copy_from_slice(&8u16.to_le_bytes());
    let mut cords = [[0u8; 42]; 2];
    for (i, cord) in cords.iter_mut().enumerate() {
        cord[..2].copy_from_slice(&(i as u16).to_le_bytes());
        cord[28..30].copy_from_slice(&(i as u16).to_le_bytes());
        cord[30..32].copy_from_slice(&1u16.to_le_bytes());
        cord[32..36].copy_from_slice(&4u32.to_le_bytes());
        cord[37..41].copy_from_slice(&4u32.to_le_bytes());
    }
    cords[0][18] = 1; // Exact external input0 -> node0.port0.
    cords[1][23] = 1; // node0.port0 -> exact external output1.
    cords[1][24..26].copy_from_slice(&1u16.to_le_bytes());
    let route = [0, 0, 0, 0, 0, 0, 1, 0];
    let target = [1, 0, 1, 1, 0, 0, 0];
    let startup = [1, 0, 0];
    let records = [
        a::AssignedRecord {
            tag: a::ASSIGNED_NODE,
            payload: &node,
        },
        a::AssignedRecord {
            tag: a::ASSIGNED_CORD,
            payload: &cords[0],
        },
        a::AssignedRecord {
            tag: a::ASSIGNED_CORD,
            payload: &cords[1],
        },
        a::AssignedRecord {
            tag: a::ASSIGNED_ROUTE,
            payload: &route,
        },
        a::AssignedRecord {
            tag: a::ASSIGNED_ROUTE_TARGET,
            payload: &target,
        },
        a::AssignedRecord {
            tag: a::ASSIGNED_LOCAL_FORE,
            payload: fore[0].payload(),
        },
        a::AssignedRecord {
            tag: a::ASSIGNED_LOCAL_FORE,
            payload: fore[1].payload(),
        },
        a::AssignedRecord {
            tag: a::ASSIGNED_STARTUP,
            payload: &startup,
        },
    ];
    let required = a::AssignedPlanRequirements {
        host: a::AssignedIdentity::from_text("host"),
        boot: a::AssignedIdentity::from_text("boot"),
        host_calls: &[],
        resources: &[],
        remote_bindings: &[],
    };
    let mut bytes = vec![0u8; a::ASSIGNED_PLAN_HEADER_BYTES];
    bytes[..8].copy_from_slice(&a::assigned_plan_magic());
    bytes[8..10].copy_from_slice(&7u16.to_le_bytes());
    bytes[12..14].copy_from_slice(&128u16.to_le_bytes());
    bytes[48..64].copy_from_slice(&required.host.0);
    bytes[64..80].copy_from_slice(&required.boot.0);
    let mut counts = [0; a::ASSIGNED_PLAN_COUNT_KINDS];
    for record in records {
        counts[usize::from(record.tag - 1)] += 1;
        bytes.push(record.tag);
        bytes.extend_from_slice(&(record.payload.len() as u16).to_le_bytes());
        bytes.extend_from_slice(record.payload);
    }
    let length = bytes.len() as u16;
    bytes[10..12].copy_from_slice(&length.to_le_bytes());
    bytes[80..93].copy_from_slice(&counts);
    let digest = a::assigned_plan_payload_digest(&bytes[125..]);
    bytes[93..125].copy_from_slice(&digest);
    let pins: Vec<_> = records
        .iter()
        .map(|r| a::AssignedRecordRequirement::from_kernel_record(*r, 7))
        .collect();
    let tables = Tables::prepare_with_fore(
        &bytes,
        a::AssignedPlanMaxima::TINY_HOST,
        required,
        a::AssignedComputationProfile {
            maximum_text_bytes: 16,
            maximum_total_text_bytes: 16,
            requirements: &[],
        },
        &ResidentLayout {
            records: &pins,
            counts,
        },
        &fore,
    )
    .unwrap();
    let mut nodes = [NodeSpec {
        input_cords: [None],
        maximum_step_fuel: 0,
    }];
    let mut cords = [CordSpec::inactive(); 2];
    let mut routes = FixedRoutes::<1, 1>::new(1);
    let mut calls = FixedHostCallBindings::<1>::new(1);
    let identity = BoundedAssignedKernelTables::<1, 2, 1, 1, 1, 1>::write_with_fore(
        &bytes,
        a::AssignedPlanMaxima::TINY_HOST,
        required,
        a::AssignedComputationProfile {
            maximum_text_bytes: 16,
            maximum_total_text_bytes: 16,
            requirements: &[],
        },
        &ResidentLayout {
            records: &pins,
            counts,
        },
        &fore,
        &mut nodes,
        &mut cords,
        &mut routes,
        &mut calls,
    )
    .unwrap();
    assert_eq!(identity.counts, counts);
    assert_eq!(nodes, tables.nodes);
    assert_eq!(cords, tables.cords);
    assert!(routes.is_sealed() && calls.is_sealed());
    tables
}
fn kernel(tables: Tables, cords: [CordSpec; 2]) -> Result<Kernel, SchedulerError> {
    Kernel::new(
        tables.nodes,
        cords,
        tables.routes,
        [Echo],
        FixedValueStore::new(4).unwrap(),
        FixedSignLog::new_with_remote_storage(
            (16 * size_of::<KernelEvent>()) as u32,
            8,
            remote_sign_storage_bytes(8).unwrap(),
        )
        .unwrap(),
    )
}
fn frame(kernel: &mut Kernel, sequence: u64) {
    assert_eq!(
        kernel
            .admit_remote_input(RemoteEndpointId(0), CordId(0), sequence, b"data")
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence }
    );
    kernel.step().unwrap();
    let value = kernel
        .remote_egress_offer(RemoteEndpointId(1), CordId(1))
        .unwrap()
        .unwrap();
    assert_eq!(value.sequence, sequence);
    assert_eq!(kernel.values().get(value.value).unwrap(), b"data");
    kernel
        .remote_egress_accept(RemoteEndpointId(1), CordId(1), sequence)
        .unwrap();
    kernel
        .remote_egress_delivered(RemoteEndpointId(1), CordId(1), sequence)
        .unwrap();
    assert_eq!(kernel.values().used_items(), 0);
}
#[test]
fn checked_local_fore_rolls_value_evidence_while_line_custody_remains_mandatory() {
    let tables = prepared();
    let local_cords = tables.cords;
    let mut local = kernel(tables, local_cords).unwrap();
    for sequence in 0..128 {
        frame(&mut local, sequence);
    }
    local
        .close_remote_input(RemoteEndpointId(0), CordId(0))
        .unwrap();
    let mut drained = false;
    for _ in 0..4 {
        if local.step().unwrap() == SchedulerStatus::Drained {
            drained = true;
            break;
        }
    }
    assert!(drained);
    assert_eq!(local.signs().remote_len(), 0);
    assert!(local
        .signs()
        .events()
        .any(|e| e.kind == KernelEventKind::BackCompleted));
    assert!(!local.signs().events().any(|e| matches!(
        e.kind,
        KernelEventKind::RemoteInputAdmitted
            | KernelEventKind::RemoteValueDelivered
            | KernelEventKind::RemoteOutputClosed
    )));

    let tables = prepared();
    let plain = [
        CordSpec::remote_ingress(
            CordId(0),
            RemoteEndpointId(0),
            (NodeId(0), PortId(0)),
            capacity(0),
        ),
        CordSpec::remote_egress(
            CordId(1),
            (NodeId(0), PortId(0)),
            RemoteEndpointId(1),
            capacity(1),
        ),
    ];
    let mut line = kernel(tables, plain).unwrap();
    frame(&mut line, 0);
    frame(&mut line, 1);
    assert_eq!(line.signs().remote_len(), 8);
    assert_eq!(
        line.admit_remote_input(RemoteEndpointId(0), CordId(0), 2, b"data"),
        Err(SchedulerError::Sign(SignError::RemoteItemCapacityExceeded))
    );
    assert_eq!(line.cord_usage(CordId(0)).unwrap(), (0, 0));
}
#[test]
fn changing_prepared_fore_endpoint_cannot_reclassify_another_boundary() {
    let tables = prepared();
    let mut changed = tables.cords;
    changed[0].source = CordEndpoint::Remote(RemoteEndpointId(9));
    assert!(matches!(
        kernel(tables, changed),
        Err(SchedulerError::InvalidPlan)
    ));
}

#[cfg(feature = "alloc")]
#[test]
fn owned_local_value_session_rearms_without_fictitious_line_reservations() {
    use conduit_kernel::scheduler::prepared_session::{OwnedValueBoundary, PreparedKernelSession};
    let tables = prepared();
    let cords = tables.cords;
    let kernel = kernel(tables, cords).unwrap();
    let boundaries = [
        OwnedValueBoundary {
            cord: CordId(0),
            endpoint: RemoteEndpointId(0),
        },
        OwnedValueBoundary {
            cord: CordId(1),
            endpoint: RemoteEndpointId(1),
        },
    ];
    // Caller may retain its old conservative Line allowance, but classified
    // process-local Fore must not require any Line evidence reservation.
    let mut session =
        PreparedKernelSession::new_with_owned_value_boundaries(Box::new(kernel), &boundaries, 16)
            .unwrap_or_else(|_| panic!("exact local Fore needs no Line reservation"));
    let mut evidence = Vec::new();
    for sequence in 0..3 {
        assert_eq!(
            session
                .admit_owned_input(RemoteEndpointId(0), CordId(0), sequence, b"data")
                .unwrap(),
            RemoteIngressOutcome::Accepted { sequence }
        );
        session
            .close_owned_input(RemoteEndpointId(0), CordId(0))
            .unwrap();
        let mut drained = false;
        for _ in 0..8 {
            if let Some(value) = session
                .owned_output(RemoteEndpointId(1), CordId(1))
                .unwrap()
            {
                assert_eq!(value.sequence, sequence);
                session
                    .accept_owned_output(RemoteEndpointId(1), CordId(1), sequence)
                    .unwrap();
            }
            if session.step().unwrap() == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
        assert!(drained);
        assert_eq!(session.kernel().signs().remote_len(), 0);
        if sequence < 2 {
            let signs = FixedSignLog::new_with_remote_storage(
                (16 * size_of::<KernelEvent>()) as u32,
                8,
                remote_sign_storage_bytes(8).unwrap(),
            )
            .unwrap();
            evidence.push(
                session
                    .begin_next([()], signs)
                    .unwrap_or_else(|_| panic!("same owned local invocation")),
            );
        }
    }
    assert_eq!(evidence.len(), 2);
    assert!(evidence.iter().all(|old| old.signs.remote_len() == 0));
}

#[cfg(feature = "alloc")]
impl conduit_kernel::scheduler::prepared_session::ReusableStepBack<1> for Echo {
    type Next = ();
    fn preflight_next(&self, _: &(), _: &dyn ValueStorage) -> bool {
        true
    }
    fn begin_next(&mut self, _: ()) {}
}
