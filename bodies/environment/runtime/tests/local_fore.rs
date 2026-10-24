//! Exact local fore tables run checked Source in the ordinary kernel.
//! These CPU fixtures do not establish native authority or physical output.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "fore_support/plan.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_embedded_build as b;
use conduit_environment_runtime::{assigned::*, CaptureBack};
use conduit_kernel::{self as k, scheduler::*};
type Tables = AssignedKernelTables<1, 2, 1, 1, 1, 0>;
#[test]
fn exact_fore_tables_preserve_checked_execution_and_refuse_wrong_resident_pins() {
    let plan = fixture::plan(64);
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let generated = b::generate_fore_native_plan(
        &plan,
        &fragment.fragment_id,
        b::EmbeddedImageBounds::HOST_TOOLING,
        b::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536,
        },
        2,
    )
    .unwrap();
    let maxima = c::AssignedPlanMaxima {
        encoded_bytes: 16384,
        runtime_state_bytes: 32768,
        counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
    };
    let bytes = generated.encode(maxima).unwrap();
    let records = c::AssignedPlanRecords::new(&bytes)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let requirements = generated.layout_requirements(maxima).unwrap();
    let computations = b::assigned_computation_requirements(generated.kernel()).unwrap();
    let profile = c::AssignedComputationProfile {
        maximum_text_bytes: u16::MAX,
        maximum_total_text_bytes: u32::MAX,
        requirements: &computations,
    };
    let required = c::AssignedPlanRequirements {
        host: c::AssignedIdentity::from_text(fragment.host_id.as_str()),
        boot: c::AssignedIdentity::from_text(fragment.boot_id.as_str()),
        host_calls: &[],
        resources: &[],
        remote_bindings: &[],
    };
    let mut counts = [0; c::ASSIGNED_PLAN_COUNT_KINDS];
    counts.copy_from_slice(&bytes[80..93]);
    let layout = ResidentLayout {
        records: &requirements,
        counts,
    };
    let ((tables, borrowed), memory) = allocation_probe::observe(|| {
        assert!(Tables::prepare(&bytes, maxima, required, profile, &layout).is_err());
        assert!(Tables::prepare_records(
            &bytes[..125],
            &records,
            maxima,
            required,
            profile,
            &layout
        )
        .is_err());
        let contiguous =
            Tables::prepare_with_fore(&bytes, maxima, required, profile, &layout, generated.fore())
                .unwrap();
        let borrowed = Tables::prepare_records_with_fore(
            &bytes[..125],
            &records,
            maxima,
            required,
            profile,
            &layout,
            generated.fore(),
        )
        .unwrap();
        assert_eq!(contiguous.identity, borrowed.identity);
        assert_eq!(contiguous.nodes, borrowed.nodes);
        assert_eq!(contiguous.cords, borrowed.cords);
        assert_eq!(contiguous.startup_order(), borrowed.startup_order());
        assert!(contiguous
            .routes
            .route(k::NodeId(0), k::PortId(0))
            .unwrap()
            .eq(borrowed.routes.route(k::NodeId(0), k::PortId(0)).unwrap()));
        assert!(contiguous.calls.is_sealed() && borrowed.calls.is_sealed());
        (contiguous, borrowed)
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(
        tables.nodes[0].input_cords[0],
        lowered.node_specs[0].input_cords[0]
    );
    assert_eq!(
        tables.nodes[0].maximum_step_fuel,
        lowered.node_specs[0].maximum_step_fuel
    );
    for (actual, expected) in tables.cords.iter().zip(&lowered.cords) {
        assert_eq!(actual, &expected.spec);
    }
    for change in 0..5 {
        let mut pins = generated.fore().to_vec();
        let pin = pins[0];
        pins[0] = c::AssignedForeRequirement::new(
            if change == 0 {
                pin.endpoint() + 1
            } else {
                pin.endpoint()
            },
            if change == 1 {
                pin.cord() + 1
            } else {
                pin.cord()
            },
            if change == 2 {
                c::AssignedForeDirection::Output
            } else {
                pin.direction()
            },
            if change == 4 {
                c::AssignedIdentity::from_text("foreign/front-port")
            } else {
                c::AssignedIdentity(pin.payload()[5..21].try_into().unwrap())
            },
            if change == 3 {
                c::assigned_plan_payload_digest(b"wrong ABI")
            } else {
                pin.payload()[21..].try_into().unwrap()
            },
        );
        let ((), memory) = allocation_probe::observe(|| {
            assert!(
                Tables::prepare_with_fore(&bytes, maxima, required, profile, &layout, &pins)
                    .is_err()
            );
            assert!(Tables::prepare_records_with_fore(
                &bytes[..125],
                &records,
                maxima,
                required,
                profile,
                &layout,
                &pins
            )
            .is_err());
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
    // Execution tables own their numeric data, not assignment payload storage.
    drop(records);
    drop(bytes);
    drop(requirements);
    drop(computations);
    drop(generated);
    let c::ConfigurationValue::Text(program) = &fragment.placements[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let input = lowered
        .fore_ports
        .iter()
        .find(|f| f.direction == c::PortDirection::Input)
        .unwrap();
    let output = lowered
        .fore_ports
        .iter()
        .find(|f| f.direction == c::PortDirection::Output)
        .unwrap();
    let make_kernel = |tables: Tables| {
        let back =
            CaptureBack::expression(&program, c::PortTemporal::Flow { closes: true }).unwrap();
        let values = k::FixedValueStore::<4, 64>::new(256).unwrap();
        let signs = k::FixedSignLog::<128>::new_with_remote_storage(
            128 * core::mem::size_of::<k::KernelEvent>() as u32,
            128,
            k::remote_sign_storage_bytes(128).unwrap(),
        )
        .unwrap();
        FixedScheduler::<_, _, _, 1, 2, 1, 2, 1, 1, 0, 1>::new(
            tables.nodes,
            tables.cords,
            tables.routes,
            [back],
            values,
            signs,
        )
        .unwrap()
    };
    let mut kernel = make_kernel(tables);
    let mut cancelled = make_kernel(borrowed);
    let ((), memory) = allocation_probe::observe(|| {
        input.validate_value(&[41]).unwrap();
        assert_eq!(
            kernel
                .admit_remote_input(input.endpoint, input.cord, 0, &[41])
                .unwrap(),
            RemoteIngressOutcome::Accepted { sequence: 0 }
        );
        assert_eq!(
            kernel
                .admit_remote_input(input.endpoint, input.cord, 1, &[42])
                .unwrap(),
            RemoteIngressOutcome::Full { sequence: 1 }
        );
        let mut delivered = 0;
        let mut drained = false;
        for _ in 0..64 {
            if let Some(offer) = kernel
                .remote_egress_offer(output.endpoint, output.cord)
                .unwrap()
            {
                let value = kernel.host_value(offer.value).unwrap();
                output.validate_value(value).unwrap();
                assert_eq!(value, &[42 + delivered]);
                kernel
                    .remote_egress_accept(offer.endpoint, offer.cord, offer.sequence)
                    .unwrap();
                kernel
                    .remote_egress_delivered(offer.endpoint, offer.cord, offer.sequence)
                    .unwrap();
                delivered += 1;
                if delivered == 1 {
                    kernel
                        .admit_remote_input(input.endpoint, input.cord, 1, &[42])
                        .unwrap();
                    kernel
                        .close_remote_input(input.endpoint, input.cord)
                        .unwrap();
                }
            }
            if kernel.step().unwrap() == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
        assert!(drained);
        assert_eq!(delivered, 2);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    let ((), memory) = allocation_probe::observe(|| {
        cancelled
            .admit_remote_input(input.endpoint, input.cord, 0, &[41])
            .unwrap();
        cancelled.cancel().unwrap();
        assert_eq!(cancelled.step().unwrap(), SchedulerStatus::Cancelled);
        assert_eq!(
            cancelled.remote_egress_offer(output.endpoint, output.cord),
            Err(SchedulerError::Cancelled)
        );
        assert_eq!(
            cancelled.admit_remote_input(input.endpoint, input.cord, 1, &[42]),
            Err(SchedulerError::Cancelled)
        );
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
