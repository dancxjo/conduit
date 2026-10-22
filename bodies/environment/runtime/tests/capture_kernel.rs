//! Actual planning, lowering, kernel routing and closure over transcript effects.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "support/assignment.rs"]
mod assignment;
#[path = "support/back.rs"]
mod back;
#[path = "support/plan.rs"]
mod plan;
#[path = "support/pulse.rs"]
mod pulse;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use back::{Back, Observed};
use conduit_core::{
    assigned_plan_payload_digest, validate_canonical_structured_value as validate,
    ConfigurationValue, StructuredSelector, ASSIGNED_HOST_CALL, ASSIGNED_NATIVE_CALL_BYTES,
    ASSIGNED_PLAN_COUNT_KINDS, ASSIGNED_PLAN_HEADER_BYTES,
};
use conduit_environment_runtime::{CaptureBack, PreparedCaptureSelector};
use conduit_kernel::{
    scheduler::{FixedScheduler, SchedulerStatus},
    *,
};
use conduit_plot::PortableExpressionProgram;
use conduit_pulse_capture::*;
const N: usize = 13;
const C: usize = 12;
const P: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const R: usize = N * P;

#[test]
fn checked_source_traverses_the_production_kernel_and_closes_after_each_outcome() {
    for scenario in 0..5 {
        run(scenario)
    }
}
fn run(scenario: usize) {
    let (ready, effects) = pulse::ready(scenario);
    let ((plan, lowered), preparation) = allocation_probe::observe(|| plan::prepare(&ready));
    if scenario == 0 {
        // Exact baseline for this authored Source and checked expansion. Source
        // byte changes or upstream Cord provenance intentionally change these IDs.
        assert_eq!(
            (
                plan.plan_id.as_str(),
                plan.fragments[0].fragment_id.as_str()
            ),
            (
                "4c8197aec4f210bd183d4dd62d2649d1d093563d388fea57b7e71f6986066323",
                "664dab5fadf452d0aa6e039e0e38bf54bf169002dc7fdc7c2b482126b628fc8d"
            )
        );
        eprintln!(
            "environment_source_planning_peak_bytes={} preparation_requested_bytes={}",
            preparation.peak_bytes, preparation.requested_bytes
        );
    }
    let fragment = &plan.fragments[0];
    assert_eq!(lowered.nodes.len(), N);
    assert_eq!(lowered.cords.len(), C);
    let image = conduit_embedded_build::generate_embedded_plan_with_computations(
        fragment,
        &lowered,
        conduit_embedded_build::EmbeddedImageBounds {
            maximum_nodes: N,
            maximum_cords: C,
            maximum_routes: N,
            maximum_route_targets: C,
            maximum_host_calls: N,
            maximum_resources: 1,
            maximum_sign_expectations: 128,
            maximum_configuration_entries: 16,
            maximum_ports_per_node: P,
            maximum_remote_endpoints: 0,
            maximum_cord_value_slots: C as u16,
            maximum_cord_value_bytes: 16384,
            maximum_sign_items: 128,
            maximum_sign_bytes: 32768,
        },
        conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536,
        },
    )
    .unwrap();
    let native_projection = assignment::NativeProjection::prepare(&image);
    let (mut pulse_owner, selected_node) =
        pulse::bind_assigned(ready, &plan, &lowered, &native_projection).unwrap();
    assert_eq!(image.plan_id, plan.plan_id.as_str());
    let tables = assignment::prepare::<N, C, P, R, C, N>(&image, scenario == 0);
    assert_eq!(
        tables.nodes,
        core::array::from_fn(|i| lowered.node_specs[i])
    );
    assert_eq!(
        tables.cords,
        core::array::from_fn(|i| lowered.cords[i].spec)
    );
    let observed: [Observed; 4] = core::array::from_fn(|_| Observed::new());
    let mut selectors: [Option<PreparedCaptureSelector>; N] = core::array::from_fn(|_| None);
    let mut pulse_node = None;
    let mut build_back = |i: usize| {
        let node = &lowered.nodes[i];
        let gear = fragment
            .placements
            .iter()
            .find(|g| g.placement_id == node.placement_id)
            .unwrap();
        let kind = gear.kind_id.as_str();
        if kind == format!("{}/begin", plan::PREFIX) {
            return Back::Begin {
                sent: false,
                staged: false,
            };
        }
        if let Some(index) = [
            "decoded",
            "timed-out",
            "provider-failed",
            "geometry-refused",
        ]
        .iter()
        .position(|name| kind == format!("{}/{name}", plan::PREFIX))
        {
            return Back::Observe {
                observed: &observed[index],
                pending: Box::new([0; 4096]),
                length: 0,
                staged: false,
            };
        }
        if kind == PULSE_READ_KIND {
            pulse_node = Some(node.node);
            return Back::Native(CaptureBack::pulse(1024));
        }
        let ConfigurationValue::Text(text) = &gear.configuration[0].value else {
            panic!("checked metadata")
        };
        if gear.configuration[0].key == "program" {
            Back::Native(
                CaptureBack::expression(
                    &PortableExpressionProgram::from_canonical_hex(text).unwrap(),
                    gear.inputs[0].temporal,
                )
                .unwrap(),
            )
        } else {
            selectors[i] = Some(
                PreparedCaptureSelector::prepare(
                    StructuredSelector::from_canonical_hex(text).unwrap(),
                    1024,
                    1024,
                )
                .unwrap(),
            );
            Back::Native(CaptureBack::selector(1024))
        }
    };
    let mut preparation_order = [NodeId(u16::MAX); N];
    let mut prepared = 0;
    let backs: [Back<'_>; N] = tables
        .prepare_backs(|node| {
            preparation_order[prepared] = node;
            prepared += 1;
            Ok::<_, core::convert::Infallible>(build_back(usize::from(node.0)))
        })
        .unwrap();
    assert_eq!(&preparation_order, tables.startup_order());
    assert!(preparation_order
        .iter()
        .map(|n| n.0)
        .eq(image.startup_order.iter().copied()));
    let pulse_node = pulse_node.unwrap();
    let values = FixedValueStore::<64, 1024>::new(65536).unwrap();
    let signs = FixedSignLog::<128>::new(lowered.sign_bytes).unwrap();
    let mut kernel = FixedScheduler::<_, _, _, N, C, P, C, R, C, N, N>::new_with_host_calls(
        tables.nodes,
        tables.cords,
        tables.routes,
        tables.calls,
        backs,
        values,
        signs,
    )
    .unwrap();
    assert_eq!(pulse_node, selected_node);
    let mut pulse_calls = 0;
    let (complete, allocation) = allocation_probe::observe(|| {
        for _ in 0..1000 {
            if let Some(request) = kernel.next_host_request() {
                let input = kernel.host_value(request.input.value).unwrap();
                let output = if request.node == pulse_node {
                    pulse_calls += 1;
                    Some(
                        ready_immediate(pulse_owner.invoke(
                            request.node,
                            request.call,
                            request.request,
                            input,
                        ))
                        .unwrap(),
                    )
                } else {
                    selectors[usize::from(request.node.0)]
                        .as_mut()
                        .unwrap()
                        .select(input)
                        .unwrap()
                };
                let output = output.map(|bytes| {
                    BoundedValueRef::new(kernel.store_host_value(bytes).unwrap(), 1024).unwrap()
                });
                kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        HostCallOutcome {
                            disposition: HostCallDisposition::Completed,
                            output,
                            failure: None,
                        },
                    )
                    .unwrap();
            }
            if kernel.step().unwrap() == SchedulerStatus::Drained {
                return true;
            }
        }
        false
    });
    assert!(complete, "scenario {scenario} did not close");
    assert_eq!(pulse_calls, 1, "no implicit retry");
    assert_eq!(effects.get(), usize::from(scenario != 4));
    assert_eq!(pulse_owner.inspection().completed_operations, 1);
    assert_eq!(pulse_owner.inspection().in_flight_operations, 0);
    assert_eq!(allocation.allocations, 0);
    assert_eq!(allocation.reallocations, 0);
    let target = match scenario {
        0 | 1 => 0,
        2 => 1,
        3 => 2,
        _ => 3,
    };
    for (index, witness) in observed.iter().enumerate() {
        assert_eq!(
            witness.count.get(),
            usize::from(index == target),
            "scenario {scenario}, port {index}"
        );
    }
    if scenario <= 1 {
        let bytes = observed[0].bytes.borrow();
        let result = validate(&bytes.0[..bytes.1]).unwrap();
        if scenario == 0 {
            let frame = result.variant_payload("frame").unwrap().unwrap();
            for (field, expected) in [("temperature", 198_i128), ("humidity", 560_i128)] {
                let value = frame.record_field(field).unwrap().unwrap();
                assert_eq!(
                    i128::from_le_bytes(value.value_node()[5..].try_into().unwrap()),
                    expected
                );
            }
        } else {
            assert!(result
                .variant_payload("unavailable")
                .unwrap()
                .unwrap()
                .variant_payload("checksum")
                .unwrap()
                .is_some())
        }
    } else if scenario == 2 {
        let bytes = observed[1].bytes.borrow();
        let capture = validate(&bytes.0[..bytes.1]).unwrap();
        assert_eq!(
            &capture
                .record_field("length")
                .unwrap()
                .unwrap()
                .value_node()[5..],
            &[2]
        );
    }
}

fn ready_immediate<T>(future: impl core::future::Future<Output = T>) -> T {
    let mut future = core::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
    {
        std::task::Poll::Ready(result) => result,
        std::task::Poll::Pending => panic!("immediate transcript provider"),
    }
}

#[test]
fn a_replacement_native_provider_cannot_consume_the_existing_plan() {
    let (ready, _) = pulse::ready(0);
    let (plan, lowered) = plan::prepare(&ready);
    let mut identity = pulse::identity();
    identity.provider_generation += 1;
    let (replacement, effects) = pulse::ready_with_identity(0, identity);
    assert_eq!(
        pulse::bind(replacement, &plan, &lowered).err(),
        Some(conduit_pulse_capture::owner::PulseCallRefusal::WrongBinding)
    );
    assert_eq!(effects.get(), 0);
}

#[test]
fn a_fresh_boot_reuses_the_resident_layout_with_a_fresh_plan_identity() {
    let (original, _) = pulse::ready(0);
    let (original_plan, original_lowered) = plan::prepare(&original);
    let mut native = pulse::identity();
    native.boot_id = "fixture/environment/boot-next".into();
    let (next, _) = pulse::ready_with_identity(0, native.clone());
    let (next_plan, next_lowered) = plan::prepare_for_boot(&next, &native.boot_id);
    assert_ne!(original_plan.plan_id, next_plan.plan_id);
    // An inactive fixed local template may be prepared for a fresh Boot using
    // the ordinary core sealer, without a Source compiler or a private hash.
    let ((rebound, _), measured) = allocation_probe::observe(|| {
        assert!(original_plan.activations.is_empty());
        let mut fragment = original_plan.fragments[0].clone();
        assert!(fragment.shared_pools.is_empty() && fragment.fore_ports.is_empty());
        assert!(fragment
            .connections
            .iter()
            .all(|cord| cord.selected_line.is_none()
                && cord.admitted_lines.is_empty()
                && cord.resource.is_none()));
        fragment.boot_id = native.boot_id.clone();
        for gear in &mut fragment.placements {
            gear.boot_id = native.boot_id.clone();
            for authority in &mut gear.authority {
                authority.boot_id = native.boot_id.clone();
            }
        }
        let rebound = conduit_core::seal_plan_with_realization_backs_and_completion(
            conduit_core::PlotIdentity {
                source_document_id: original_plan.source_document_id.clone(),
                checked_plot_id: original_plan.checked_plot_id.clone(),
                expanded_plot_id: original_plan.expanded_plot_id.clone(),
            },
            original_plan.completion_policy,
            original_plan.realization_backs.clone(),
            vec![fragment],
        );
        let lowered =
            conduit_plan_lowering::lowering::lower_plan_fragment(&rebound.fragments[0]).unwrap();
        (rebound, lowered)
    });
    assert!(
        rebound == next_plan,
        "ordinary sealing of the fixed local template must equal fresh ordinary planning"
    );
    eprintln!(
        "environment_fixed_local_template_preparation_peak_bytes={} requested_bytes={}",
        measured.peak_bytes, measured.requested_bytes
    );

    let generate =
        |plan: &conduit_core::Plan,
         lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment| {
            conduit_embedded_build::generate_embedded_plan_with_computations(
                &plan.fragments[0],
                lowered,
                conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING,
                conduit_embedded_build::EmbeddedComputationBounds {
                    maximum_program_bytes: 32768,
                    maximum_total_program_bytes: 65536,
                },
            )
            .unwrap()
        };
    let resident = generate(&original_plan, &original_lowered);
    let incoming = generate(&next_plan, &next_lowered);
    let tables = assignment::prepare_from_resident::<N, C, P, R, C, N>(&incoming, &resident, false);
    assert_eq!(
        tables.identity.boot,
        conduit_core::AssignedIdentity::from_text(native.boot_id.as_str())
    );
    assert_eq!(
        tables.nodes,
        core::array::from_fn(|i| next_lowered.node_specs[i])
    );
}

#[test]
fn assigned_native_facts_cannot_substitute_the_local_grant_or_attachment() {
    let (ready, _) = pulse::ready(0);
    let (plan, lowered) = plan::prepare(&ready);
    let image = conduit_embedded_build::generate_embedded_plan_with_computations(
        &plan.fragments[0],
        &lowered,
        conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING,
        conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536,
        },
    )
    .unwrap();
    for offset in [
        47, 63, 79, 95, 103, 119, 135, 143, 159, 176, 192, 208, 224, 240, 256, 272, 288, 290, 306,
        322,
    ] {
        let mut projection = assignment::NativeProjection::prepare(&image);
        let mut cursor = ASSIGNED_PLAN_HEADER_BYTES;
        loop {
            let length = usize::from(u16::from_le_bytes([
                projection.bytes[cursor + 1],
                projection.bytes[cursor + 2],
            ]));
            if projection.bytes[cursor] == ASSIGNED_HOST_CALL
                && length == ASSIGNED_NATIVE_CALL_BYTES
            {
                break;
            }
            cursor += 3 + length;
        }
        projection.bytes[cursor + 3 + offset] ^= 1;
        let digest = assigned_plan_payload_digest(&projection.bytes[ASSIGNED_PLAN_HEADER_BYTES..]);
        let start = 80 + ASSIGNED_PLAN_COUNT_KINDS;
        projection.bytes[start..start + 32].copy_from_slice(&digest);
        let (ready, effects) = pulse::ready(0);
        assert_eq!(
            pulse::bind_assigned(ready, &plan, &lowered, &projection).err(),
            Some(conduit_pulse_capture::owner::PulseCallRefusal::WrongBinding),
            "native field offset {offset}"
        );
        assert_eq!(effects.get(), 0);
    }
}
