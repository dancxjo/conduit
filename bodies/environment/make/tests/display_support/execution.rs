//! Checked Source/planner/lowering and retained software execute in the original
//! kernel. CPU Root identity, pin provider and authority are explicit fixtures.
use super::planning::fixture;
use conduit_environment_make::{DisplayOutputSource, PreparedDisplayOutput};
use conduit_environment_runtime::{
    display_output::PreparedDisplayKernel, display_packets::PreparedDisplayPackets,
};
use conduit_kernel::{self as k, scheduler::*};
use std::{cell::Cell, rc::Rc};
#[path = "output_owner.rs"]
mod output_owner;
#[test]
fn checked_output_services_real_requests_and_exposes_refusal_without_pin_effects() {
    for mode in 0..3 {
        let brightness = if mode == 1 { 8 } else { 3 };
        let source = DisplayOutputSource::compile().unwrap();
        let (host, grant, mut software) = fixture(&source);
        let prepared = PreparedDisplayOutput::prepare(
            &source,
            &host,
            &["fixture/base".into(), "conduit.base/local@1".into()],
            &[grant],
        )
        .unwrap();
        let mut execution =
            PreparedDisplayKernel::prepare(prepared.plan(), prepared.lowered(), &mut software, 2)
                .unwrap();
        let edges = Rc::new(Cell::new(0));
        let mut owner = output_owner::bind(&prepared, output_owner::Pins(edges.clone()));
        let mut encoder = PreparedDisplayPackets::prepare().unwrap();
        let registers = [0x7d, 0xff, 0x66, 0x71, 0, 0];
        let frame = encoder
            .encode_frame(registers, brightness, true)
            .unwrap()
            .to_vec();
        execution.input.validate_value(&frame).unwrap();
        assert_eq!(
            execution
                .kernel
                .admit_remote_input(execution.input.endpoint, execution.input.cord, 0, &frame)
                .unwrap(),
            RemoteIngressOutcome::Accepted { sequence: 0 }
        );
        if mode != 0 {
            execution
                .kernel
                .close_remote_input(execution.input.endpoint, execution.input.cord)
                .unwrap();
        }
        let expected = encoder
            .encode_canonical(registers, 3, true)
            .unwrap()
            .to_vec();
        let second_registers = [0, 0x6d, 0x6f, 0x76, 0, 0];
        let second_frame = encoder
            .encode_frame(second_registers, 3, true)
            .unwrap()
            .to_vec();
        let second_expected = encoder
            .encode_canonical(second_registers, 3, true)
            .unwrap()
            .to_vec();
        let mut writes = 0;
        let mut refusals = 0;
        let mut drained = false;
        let ((), memory) = crate::allocation_probe::observe(|| {
            for _ in 0..100 {
                if let Some(request) = execution.kernel.next_host_request() {
                    let bytes = execution.kernel.host_value(request.input.value).unwrap();
                    let output = if request.node == execution.write {
                        assert_eq!(
                            bytes,
                            if writes == 0 {
                                &expected
                            } else {
                                &second_expected
                            }
                        );
                        if mode == 2 {
                            execution.kernel.cancel().unwrap();
                            owner.revoke(request.node, request.call).unwrap();
                            assert!(execution
                                .kernel
                                .complete_host_call(
                                    request.node,
                                    request.request,
                                    k::HostCallOutcome {
                                        disposition: k::HostCallDisposition::Completed,
                                        output: None,
                                        failure: None,
                                    }
                                )
                                .is_err());
                            break;
                        }
                        owner
                            .invoke(request.node, request.call, request.request, bytes, || false)
                            .unwrap();
                        writes += 1;
                        if mode == 0 && writes == 1 {
                            execution.input.validate_value(&second_frame).unwrap();
                            assert_eq!(
                                execution
                                    .kernel
                                    .admit_remote_input(
                                        execution.input.endpoint,
                                        execution.input.cord,
                                        1,
                                        &second_frame
                                    )
                                    .unwrap(),
                                RemoteIngressOutcome::Accepted { sequence: 1 }
                            );
                            execution
                                .kernel
                                .close_remote_input(execution.input.endpoint, execution.input.cord)
                                .unwrap();
                        }
                        None
                    } else {
                        let selector = execution.selectors[usize::from(request.node.0)]
                            .as_mut()
                            .expect("exact selected selector");
                        selector.select(bytes).unwrap().map(|value| {
                            k::BoundedValueRef::new(
                                execution.kernel.store_host_value(value).unwrap(),
                                1024,
                            )
                            .unwrap()
                        })
                    };
                    execution
                        .kernel
                        .complete_host_call(
                            request.node,
                            request.request,
                            k::HostCallOutcome {
                                disposition: k::HostCallDisposition::Completed,
                                output,
                                failure: None,
                            },
                        )
                        .unwrap();
                }
                if let Some(offer) = execution
                    .kernel
                    .remote_egress_offer(execution.refusal.endpoint, execution.refusal.cord)
                    .unwrap()
                {
                    execution
                        .refusal
                        .validate_value(execution.kernel.host_value(offer.value).unwrap())
                        .unwrap();
                    execution
                        .kernel
                        .remote_egress_accept(offer.endpoint, offer.cord, offer.sequence)
                        .unwrap();
                    execution
                        .kernel
                        .remote_egress_delivered(offer.endpoint, offer.cord, offer.sequence)
                        .unwrap();
                    refusals += 1;
                }
                if execution.kernel.step().unwrap() == SchedulerStatus::Drained {
                    drained = true;
                    break;
                }
            }
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        if mode == 2 {
            assert_eq!((writes, refusals, edges.get()), (0, 0, 0));
            continue;
        }
        assert!(drained);
        assert_eq!(
            (writes, refusals),
            if brightness < 8 { (2, 0) } else { (0, 1) }
        );
        assert_eq!(edges.get() > 0, brightness < 8);
    }
}
#[test]
fn preparation_refusal_preserves_all_unselected_software() {
    use conduit_environment_runtime::display_output::DisplayPreparationRefusal as E;
    let source = DisplayOutputSource::compile().unwrap();
    let (host, grant, mut software) = fixture(&source);
    let prepared = PreparedDisplayOutput::prepare(
        &source,
        &host,
        &["fixture/base".into(), "conduit.base/local@1".into()],
        &[grant],
    )
    .unwrap();
    assert!(matches!(
        PreparedDisplayKernel::prepare(prepared.plan(), prepared.lowered(), &mut software, 0),
        Err(E::Bounds)
    ));
    let mut changed = prepared.lowered().clone();
    changed.node_specs[0].maximum_step_fuel += 1;
    assert!(matches!(
        PreparedDisplayKernel::prepare(prepared.plan(), &changed, &mut software, 2),
        Err(E::Lowering)
    ));
    let fragment = &prepared.plan().fragments[0];
    for gear in fragment
        .placements
        .iter()
        .filter(|p| p.kind_id.as_str() != conduit_tm1637_wire::WRITE_KIND)
    {
        software
            .check_selected(fragment, &gear.placement_id)
            .unwrap();
    }
    let placement = fragment
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() != conduit_tm1637_wire::WRITE_KIND)
        .unwrap();
    software
        .take_selected(fragment, &placement.placement_id)
        .unwrap();
    assert!(matches!(
        PreparedDisplayKernel::prepare(prepared.plan(), prepared.lowered(), &mut software, 2),
        Err(E::Software(_))
    ));
    for gear in fragment.placements.iter().filter(|p| {
        p.kind_id.as_str() != conduit_tm1637_wire::WRITE_KIND
            && p.placement_id != placement.placement_id
    }) {
        software
            .check_selected(fragment, &gear.placement_id)
            .unwrap();
    }
}
