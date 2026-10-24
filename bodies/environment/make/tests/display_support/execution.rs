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
pub(super) mod output_owner;
#[test]
fn checked_output_services_real_requests_and_exposes_refusal_without_pin_effects() {
    for mode in 0..3 {
        run_output_mode(mode, false, |prepared, software| {
            PreparedDisplayKernel::prepare(prepared.plan(), prepared.lowered(), software, 2)
                .unwrap()
        });
        run_output_mode(mode, false, |prepared, software| {
            let execution = PreparedDisplayKernel::prepare_with_storage(
                prepared.plan(),
                prepared.lowered(),
                software,
                2,
                k::HostedValueStore::new(12, 1024, 12 * 1024).unwrap(),
                k::HostedSignLog::new_with_remote_storage(
                    256,
                    256 * core::mem::size_of::<k::KernelEvent>() as u32,
                    256,
                    k::remote_sign_storage_bytes(256).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            software.release_transferred_metadata().unwrap();
            execution
        });
    }
}
pub(super) fn run_output_mode<S: k::ValueStorage, L: k::SignSink>(
    mode: u8,
    assigned_owner: bool,
    prepare: impl FnOnce(
        &PreparedDisplayOutput,
        &mut conduit_environment_runtime::appliance::software::ReadySoftwareBacks,
    ) -> PreparedDisplayKernel<S, L>,
) {
    run_output_mode_with_inspection(mode, assigned_owner, prepare, |_| ());
}
pub(super) fn run_output_mode_with_inspection<S: k::ValueStorage, L: k::SignSink, R>(
    mode: u8,
    assigned_owner: bool,
    prepare: impl FnOnce(
        &PreparedDisplayOutput,
        &mut conduit_environment_runtime::appliance::software::ReadySoftwareBacks,
    ) -> PreparedDisplayKernel<S, L>,
    inspect: impl FnOnce(&L) -> R,
) -> R {
    let brightness = if mode == 1 || mode == 3 { 8 } else { 3 };
    let source = DisplayOutputSource::compile().unwrap();
    let (host, grant, mut software) = fixture(&source);
    let prepared = PreparedDisplayOutput::prepare(
        &source,
        &host,
        &["fixture/base".into(), "conduit.base/local@1".into()],
        &[grant],
    )
    .unwrap();
    let mut execution = prepare(&prepared, &mut software);
    let edges = Rc::new(Cell::new(0));
    let mut owner = if assigned_owner {
        output_owner::bind_fore(&prepared, output_owner::Pins(edges.clone()))
    } else {
        output_owner::bind(&prepared, output_owner::Pins(edges.clone()))
    };
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
    if mode != 0 && mode != 3 {
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
        .encode_frame(second_registers, if mode == 3 { 8 } else { 3 }, true)
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
            let pending = native_service::DisplayService {
                kernel: &mut execution.kernel,
                selectors: &mut execution.selectors,
                write: execution.write,
                owner: &mut owner,
            }
            .take();
            if let Some(pending) = pending {
                let request = pending.request();
                let bytes = execution.kernel.host_value(request.input.value).unwrap();
                let writing = request.node == execution.write;
                if writing {
                    assert_eq!(
                        bytes,
                        if writes == 0 {
                            &expected
                        } else {
                            &second_expected
                        }
                    );
                    if mode == 4 {
                        // CPU Host boundary reports a failed write completion;
                        // no electrical provider is invoked for this case.
                        execution
                            .kernel
                            .complete_host_call(
                                request.node,
                                request.request,
                                k::HostCallOutcome {
                                    disposition: k::HostCallDisposition::Failed,
                                    output: None,
                                    failure: Some(k::Failure {
                                        code: k::FailureCode::HostCallFailed,
                                        detail: 42,
                                    }),
                                },
                            )
                            .unwrap();
                        continue;
                    }
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
                        assert!(native_service::DisplayService {
                            kernel: &mut execution.kernel,
                            selectors: &mut execution.selectors,
                            write: execution.write,
                            owner: &mut owner,
                        }
                        .service(pending, || false)
                        .is_err());
                        break;
                    }
                }
                let report = native_service::DisplayService {
                    kernel: &mut execution.kernel,
                    selectors: &mut execution.selectors,
                    write: execution.write,
                    owner: &mut owner,
                }
                .service(pending, || false)
                .unwrap();
                assert_eq!(report.request, request);
                assert_eq!(report.failure, None);
                if writing {
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
                }
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
                if mode == 3 && refusals == 1 {
                    assert_eq!(
                        execution
                            .kernel
                            .admit_remote_input(
                                execution.input.endpoint,
                                execution.input.cord,
                                1,
                                &second_frame,
                            )
                            .unwrap(),
                        RemoteIngressOutcome::Accepted { sequence: 1 }
                    );
                    execution
                        .kernel
                        .close_remote_input(execution.input.endpoint, execution.input.cord)
                        .unwrap();
                }
            }
            match execution.kernel.step() {
                Ok(SchedulerStatus::Drained) => {
                    drained = true;
                    break;
                }
                Err(k::scheduler::SchedulerError::BackFailed(_)) if mode == 4 => {
                    execution.kernel.cancel().unwrap();
                    owner.revoke(execution.write, k::HostCallId(0)).unwrap();
                    break;
                }
                result => {
                    result.unwrap();
                }
            }
        }
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    if mode == 2 || mode == 4 {
        assert_eq!((writes, refusals, edges.get()), (0, 0, 0));
        return inspect(execution.kernel.signs());
    }
    assert!(drained);
    assert_eq!(
        (writes, refusals),
        if brightness < 8 {
            (2, 0)
        } else if mode == 3 {
            (0, 2)
        } else {
            (0, 1)
        }
    );
    assert_eq!(edges.get() > 0, brightness < 8);
    inspect(execution.kernel.signs())
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

#[test]
fn admitted_storage_refusals_precede_software_transfer() {
    use conduit_environment_runtime::display_output::DisplayPreparationRefusal as E;
    use k::ValueStorage;
    let source = DisplayOutputSource::compile().unwrap();
    let (host, grant, mut software) = fixture(&source);
    let prepared = PreparedDisplayOutput::prepare(
        &source,
        &host,
        &["fixture/base".into(), "conduit.base/local@1".into()],
        &[grant],
    )
    .unwrap();
    for case in 0..6 {
        let mut values = k::HostedValueStore::new(
            if case == 0 { 11 } else { 12 },
            1024,
            if case == 0 {
                11 * 1024
            } else if case == 1 {
                12 * 1024 - 1
            } else {
                12 * 1024
            },
        )
        .unwrap();
        if case == 2 {
            values.store(&[1]).unwrap();
        }
        let mut signs = k::HostedSignLog::new_with_remote_storage(
            if case == 3 { 1 } else { 256 },
            (if case == 3 { 1 } else { 256 }) * core::mem::size_of::<k::KernelEvent>() as u32,
            if case == 4 {
                0
            } else if case == 3 {
                1
            } else {
                256
            },
            if case == 4 {
                0
            } else {
                k::remote_sign_storage_bytes(if case == 3 { 1 } else { 256 }).unwrap()
            },
        )
        .unwrap();
        if case == 5 {
            use k::SignSink;
            signs
                .record(k::NodeId(0), None, None, k::KernelEventKind::BackCompleted)
                .unwrap();
        }
        assert!(
            matches!(
                PreparedDisplayKernel::prepare_with_storage(
                    prepared.plan(),
                    prepared.lowered(),
                    &mut software,
                    2,
                    values,
                    signs,
                ),
                Err(E::Bounds)
            ),
            "case {case}"
        );
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
    }
}

#[path = "../../../../../targets/esp32/proof/tm1637/src/native_display_service.rs"]
pub(super) mod native_service;
