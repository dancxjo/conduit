//! Numeric table and ordinary delivery conformance; no live Line is admitted.
use conduit_assigned_plan as a;
use conduit_kernel::{assigned::*, scheduler::*, *};
mod allocation_probe {
    use std::thread_local;
    include!("../../plot/tests/common/allocation_probe.rs");
}
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

type Tables = AssignedKernelTables<1, 2, 1, 1, 1, 1>;
type Bounded = BoundedAssignedKernelTables<2, 3, 1, 2, 2, 2>;
struct Fixture {
    bytes: Vec<u8>,
    records: Vec<(u8, Vec<u8>)>,
    pins: Vec<a::AssignedRecordRequirement>,
    counts: [u8; 13],
    local: Vec<a::AssignedForeRequirement>,
    lines: Vec<a::AssignedLineForeRequirement>,
    bindings: Vec<a::AssignedRemoteBinding>,
}
impl Fixture {
    fn new(selected: [bool; 2]) -> Self {
        let host = a::AssignedIdentity::from_text("host");
        let boot = a::AssignedIdentity::from_text("boot");
        let binding = a::AssignedRemoteBinding {
            line: a::AssignedIdentity::from_text("line"),
            local_host: host,
            local_boot: boot,
            peer_host: a::AssignedIdentity::from_text("peer"),
            peer_boot: a::AssignedIdentity::from_text("peer-boot"),
        };
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
        let mut node = vec![0; 52];
        node[2..4].copy_from_slice(&8u16.to_le_bytes());
        let mut records = vec![(a::ASSIGNED_NODE, node)];
        for i in 0..2u16 {
            let mut cord = vec![0; 42];
            cord[..2].copy_from_slice(&i.to_le_bytes());
            cord[28..30].copy_from_slice(&i.to_le_bytes());
            cord[30..32].copy_from_slice(&1u16.to_le_bytes());
            cord[32..36].copy_from_slice(&4u32.to_le_bytes());
            cord[37..41].copy_from_slice(&4u32.to_le_bytes());
            let offset = if i == 0 { 18 } else { 23 };
            cord[offset] = 1;
            cord[offset + 1..offset + 3].copy_from_slice(&i.to_le_bytes());
            records.push((a::ASSIGNED_CORD, cord));
        }
        records.push((a::ASSIGNED_ROUTE, vec![0, 0, 0, 0, 0, 0, 1, 0]));
        records.push((a::ASSIGNED_ROUTE_TARGET, vec![1, 0, 1, 1, 0, 0, 0]));
        let mut local = Vec::new();
        let mut lines = Vec::new();
        for (i, fore) in fore.into_iter().enumerate() {
            let payload = if selected[i] {
                let line = a::AssignedLineForeRequirement::new(
                    fore,
                    binding,
                    a::AssignedIdentity::from_text("session"),
                    a::AssignedIdentity::from_text("session-profile"),
                    [3; 32],
                );
                let payload = line.payload().to_vec();
                lines.push(line);
                payload
            } else {
                local.push(fore);
                a::assigned_line_fore_local_payload(&fore).to_vec()
            };
            records.push((a::ASSIGNED_REMOTE_ENDPOINT, payload));
        }
        records.push((a::ASSIGNED_STARTUP, vec![1, 0, 0]));
        let mut result = Self {
            bytes: Vec::new(),
            records,
            pins: Vec::new(),
            counts: [0; 13],
            local,
            lines,
            bindings: if selected.iter().any(|line| *line) {
                vec![binding]
            } else {
                vec![]
            },
        };
        result.reseal();
        result
    }
    fn reseal(&mut self) {
        self.bytes = vec![0; 125];
        self.bytes[..8].copy_from_slice(&a::assigned_plan_magic());
        self.bytes[8..10].copy_from_slice(&8u16.to_le_bytes());
        self.bytes[12..14].copy_from_slice(&128u16.to_le_bytes());
        self.bytes[48..64].copy_from_slice(&a::AssignedIdentity::from_text("host").0);
        self.bytes[64..80].copy_from_slice(&a::AssignedIdentity::from_text("boot").0);
        self.counts = [0; 13];
        self.pins.clear();
        for (tag, payload) in &self.records {
            self.counts[usize::from(*tag - 1)] += 1;
            self.pins
                .push(a::AssignedRecordRequirement::from_kernel_record(
                    a::AssignedRecord { tag: *tag, payload },
                    8,
                ));
            self.bytes.push(*tag);
            self.bytes
                .extend_from_slice(&(payload.len() as u16).to_le_bytes());
            self.bytes.extend_from_slice(payload);
        }
        let len = self.bytes.len() as u16;
        self.bytes[10..12].copy_from_slice(&len.to_le_bytes());
        self.bytes[80..93].copy_from_slice(&self.counts);
        let digest = a::assigned_plan_payload_digest(&self.bytes[125..]);
        self.bytes[93..125].copy_from_slice(&digest);
    }
    fn required(&self) -> a::AssignedPlanRequirements<'_> {
        a::AssignedPlanRequirements {
            host: a::AssignedIdentity::from_text("host"),
            boot: a::AssignedIdentity::from_text("boot"),
            host_calls: &[],
            resources: &[],
            remote_bindings: &self.bindings,
        }
    }
    fn computations(&self) -> a::AssignedComputationProfile<'static> {
        a::AssignedComputationProfile {
            maximum_text_bytes: 0,
            maximum_total_text_bytes: 0,
            requirements: &[],
        }
    }
    fn layout(&self) -> ResidentLayout<'_> {
        ResidentLayout {
            records: &self.pins,
            counts: self.counts,
        }
    }
    fn profile(&self) -> a::AssignedLineForeProfile<'_> {
        a::AssignedLineForeProfile {
            local: &self.local,
            lines: &self.lines,
        }
    }
    fn tables(&self) -> Result<Tables, AssignedKernelRefusal> {
        Tables::prepare_with_line_fore(
            &self.bytes,
            a::AssignedPlanMaxima::TINY_HOST,
            self.required(),
            self.computations(),
            &self.layout(),
            self.profile(),
        )
    }
}
#[test]
fn exact_and_active_prefix_writers_preserve_endpoints_and_original_bounds_without_allocation() {
    let fixture = Fixture::new([false, true]);
    let (result, allocations) = allocation_probe::observe(|| fixture.tables());
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    let tables = result.unwrap();
    assert_eq!(
        tables.cords[0].source,
        CordEndpoint::Remote(RemoteEndpointId(0))
    );
    assert_eq!(
        tables.cords[1].sink,
        CordEndpoint::Remote(RemoteEndpointId(1))
    );
    for (i, cord) in tables.cords.iter().enumerate() {
        assert_eq!(
            (
                cord.slot_start,
                cord.item_capacity,
                cord.byte_capacity,
                cord.maximum_value_bytes
            ),
            (i as u16, 1, 4, 4)
        );
        assert_eq!(cord.pressure_policy, AssignedPressurePolicy::PreserveOrder);
        assert_eq!(cord.track, AssignedConnectionTrack::Payload);
    }
    let bounded = Bounded::prepare_with_line_fore(
        &fixture.bytes,
        a::AssignedPlanMaxima::TINY_HOST,
        fixture.required(),
        fixture.computations(),
        &fixture.layout(),
        fixture.profile(),
    )
    .unwrap();
    assert_eq!((bounded.active_nodes(), bounded.active_cords()), (1, 2));
    assert_eq!(&bounded.cords[..2], &tables.cords);
    assert_eq!(bounded.cords[2], CordSpec::inactive());
    let mut nodes = [tables.nodes[0]; 2];
    let mut cords = [CordSpec::inactive(); 3];
    let mut routes = FixedRoutes::new(1);
    let mut calls = FixedHostCallBindings::new(1);
    let (_, allocations) = allocation_probe::observe(|| {
        Bounded::write_with_line_fore(
            &fixture.bytes,
            a::AssignedPlanMaxima::TINY_HOST,
            fixture.required(),
            fixture.computations(),
            &fixture.layout(),
            fixture.profile(),
            &mut nodes,
            &mut cords,
            &mut routes,
            &mut calls,
        )
        .unwrap()
    });
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    assert_eq!(cords, bounded.cords);
    assert!(routes.is_sealed() && calls.is_sealed());
    let mut nodes = tables.nodes;
    let mut cords = tables.cords;
    let mut routes = FixedRoutes::new(1);
    let mut calls = FixedHostCallBindings::new(1);
    Tables::write_with_line_fore(
        &fixture.bytes,
        a::AssignedPlanMaxima::TINY_HOST,
        fixture.required(),
        fixture.computations(),
        &fixture.layout(),
        fixture.profile(),
        &mut nodes,
        &mut cords,
        &mut routes,
        &mut calls,
    )
    .unwrap();
    assert_eq!(cords, tables.cords);
}
#[test]
fn old_and_borrowed_entry_points_still_refuse_schema8_and_mutated_layouts() {
    let fixture = Fixture::new([false, true]);
    assert_eq!(
        Tables::prepare(
            &fixture.bytes,
            a::AssignedPlanMaxima::TINY_HOST,
            fixture.required(),
            fixture.computations(),
            &fixture.layout()
        )
        .err(),
        Some(AssignedKernelRefusal::Unsupported)
    );
    assert_eq!(
        Tables::prepare_with_fore(
            &fixture.bytes,
            a::AssignedPlanMaxima::TINY_HOST,
            fixture.required(),
            fixture.computations(),
            &fixture.layout(),
            &fixture.local
        )
        .err(),
        Some(AssignedKernelRefusal::Unsupported)
    );
    let records: Vec<_> = fixture
        .records
        .iter()
        .map(|(tag, payload)| a::AssignedRecord { tag: *tag, payload })
        .collect();
    assert_eq!(
        Tables::prepare_records_with_fore(
            &fixture.bytes[..125],
            &records,
            a::AssignedPlanMaxima::TINY_HOST,
            fixture.required(),
            fixture.computations(),
            &fixture.layout(),
            &fixture.local
        )
        .err(),
        Some(AssignedKernelRefusal::Unsupported)
    );
    let mut changed = Fixture::new([true, true]);
    let original_pins = changed.pins.clone();
    changed.records[2].1[32] ^= 1;
    changed.reseal();
    changed.pins = original_pins;
    assert_eq!(changed.tables().err(), Some(AssignedKernelRefusal::Layout));
    // Even independently updated structural pins cannot override the exact Fore
    // endpoint or direction expected from the original separate Root profile.
    let mut changed = Fixture::new([true, true]);
    changed.records[2].1[24] = 7;
    changed.reseal();
    assert_eq!(
        changed.tables().err(),
        Some(AssignedKernelRefusal::Unsupported)
    );
    let mut changed = Fixture::new([true, true]);
    changed.records[1].1[18] = 0;
    changed.records[1].1[23] = 1;
    changed.reseal();
    assert_eq!(
        changed.tables().err(),
        Some(AssignedKernelRefusal::Unsupported)
    );
    let mut changed = Fixture::new([true, true]);
    changed.records[6].1[166] ^= 1;
    changed.reseal();
    assert!(matches!(
        changed.tables(),
        Err(AssignedKernelRefusal::Projection(
            a::AssignedPlanRefusal::StaleRemoteEndpoint
        ))
    ));
}
#[derive(Clone, Copy)]
struct Echo;
impl StepBack<1> for Echo {
    fn step(&mut self, io: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        if io.input(PortId(0)).is_some() {
            let value = io.consume(PortId(0)).unwrap();
            io.send(PortId(0), value).unwrap();
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }
}
type Kernel = FixedScheduler<Echo, FixedValueStore<1, 4>, FixedSignLog<16>, 1, 2, 1, 2, 1, 1>;
fn kernel(fixture: &Fixture) -> Kernel {
    let tables = fixture.tables().unwrap();
    Kernel::new(
        tables.nodes,
        tables.cords,
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
    .unwrap()
}
fn frame(kernel: &mut Kernel, sequence: u64) {
    kernel
        .admit_remote_input(RemoteEndpointId(0), CordId(0), sequence, b"data")
        .unwrap();
    kernel.step().unwrap();
    let offer = kernel
        .remote_egress_offer(RemoteEndpointId(1), CordId(1))
        .unwrap()
        .unwrap();
    assert_eq!(offer.sequence, sequence);
    assert_eq!(kernel.values().get(offer.value).unwrap(), b"data");
    assert_eq!(
        kernel
            .remote_egress_offer(RemoteEndpointId(1), CordId(1))
            .unwrap(),
        Some(offer)
    );
    assert_eq!(kernel.cord_usage(CordId(1)).unwrap(), (1, 4));
    kernel
        .remote_egress_accept(RemoteEndpointId(1), CordId(1), sequence)
        .unwrap();
    // Transport acceptance is not delivery, and neither event attests durable
    // recording. The original queue value remains owned until delivery.
    assert_eq!(kernel.cord_usage(CordId(1)).unwrap(), (1, 4));
    kernel
        .remote_egress_delivered(RemoteEndpointId(1), CordId(1), sequence)
        .unwrap();
    assert_eq!(kernel.values().used_items(), 0);
}
#[test]
fn selected_line_variants_keep_mandatory_remote_signs_and_delivery_pressure() {
    for (selected, remote_signs) in [
        ([false, false], 0),
        ([false, true], 3),
        ([true, false], 1),
        ([true, true], 4),
    ] {
        let mut kernel = kernel(&Fixture::new(selected));
        frame(&mut kernel, 0);
        assert_eq!(kernel.signs().remote_len(), remote_signs);
    }
    let mut kernel = kernel(&Fixture::new([true, true]));
    frame(&mut kernel, 0);
    frame(&mut kernel, 1);
    assert_eq!(kernel.signs().remote_len(), 8);
    assert_eq!(
        kernel.admit_remote_input(RemoteEndpointId(0), CordId(0), 2, b"data"),
        Err(SchedulerError::Sign(SignError::RemoteItemCapacityExceeded))
    );
    assert_eq!(kernel.cord_usage(CordId(0)).unwrap(), (0, 0));
}

#[test]
fn mixed_profile_boot_local_name_changes_pass_but_selected_line_identity_stays_exact() {
    let mut fixture = Fixture::new([false, true]);
    let original: Vec<_> = a::AssignedPlanRecords::new(&fixture.bytes)
        .unwrap()
        .map(Result::unwrap)
        .map(|r| {
            a::AssignedRecordRequirement::from_kernel_record_with_line_fore(r, fixture.profile())
        })
        .collect();
    let next_boot = a::AssignedIdentity::from_text("synthetic/local-fore/next-boot");
    let local = fixture
        .records
        .iter_mut()
        .find(|(tag, payload)| *tag == a::ASSIGNED_CORD && payload[..2] == 0u16.to_le_bytes())
        .unwrap();
    local.1[2..18].copy_from_slice(&next_boot.0);
    fixture.reseal();
    fixture.pins = original.clone();
    assert!(fixture.tables().is_ok());
    let line = fixture
        .records
        .iter_mut()
        .find(|(tag, payload)| *tag == a::ASSIGNED_CORD && payload[..2] == 1u16.to_le_bytes())
        .unwrap();
    line.1[2..18].copy_from_slice(&next_boot.0);
    fixture.reseal();
    fixture.pins = original;
    assert!(matches!(
        fixture.tables(),
        Err(AssignedKernelRefusal::Layout)
    ));
}
