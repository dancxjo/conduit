use super::*;
use conduit_body::ResidentPlot;
use std::time::Duration;

struct NoTimer;

impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {}
}

#[derive(Default)]
struct CapturedFore(Vec<ExternalForeDelivery>);

impl BodyForeOutputAdapter for CapturedFore {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        self.0.push(output.clone());
        Ok(())
    }
}

fn sixty_four_commands() -> Vec<ExternalForeInput> {
    (0..64)
        .map(|index| ExternalForeInput {
            front_port_id: conduit_core::port_id("commands"),
            track: conduit_core::ConnectionTrack::Payload,
            bytes: (if index == 0 {
                conduit_todo_plot::TodoCommand::Add {
                    text: "Milk".into(),
                }
            } else {
                conduit_todo_plot::TodoCommand::SetComplete {
                    id: "task-1".into(),
                    complete: index % 2 == 1,
                }
            })
            .encode_info()
            .unwrap(),
        })
        .collect()
}

#[test]
fn internal_preloaded_todo_body_play_delivers_before_complete_with_child_signs() {
    let plan = crate::flow_activation::authored_todo_plan();
    let fragment = &plan.fragments[0];
    let host_id = fragment.host_id.clone();
    let boot_id = fragment.boot_id.clone();
    let partition = BodyPlotPlan {
        plot: ResidentPlot::new(
            plan.source_document_id.clone(),
            plan.checked_plot_id.clone(),
        ),
        plan,
    };
    let inputs = sixty_four_commands();
    let kernel = BodyKernel::prepare(
        &[partition],
        false,
        &conduit_core::ActivePlayId::from("body/play/internal-preloaded-proof"),
        &inputs,
        true,
        true,
    )
    .unwrap();
    kernel.require_supported_execution().unwrap();
    let mut fore = CapturedFore::default();
    let result = kernel.run(
        &mut Vec::new(),
        &mut NoTimer,
        None,
        Some(&mut fore),
        &RunControl::default(),
        &host_id,
        &boot_id,
        None,
        None,
        None,
    );
    assert_eq!(result.terminal, TerminalDisposition::Completed);
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(fore.0.len(), 64);
    let state = conduit_todo_plot::TodoState::decode_info(&fore.0[63].bytes).unwrap();
    assert_eq!(state.items.len(), 1);
    assert_eq!(state.items[0].text, "Milk");
    assert_eq!(result.scan_child_signs.unwrap().len(), 64);
}

#[test]
fn internal_preloaded_todo_body_cancels_after_first_delivery() {
    struct StopOnFirst {
        control: RunControl,
        deliveries: Vec<ExternalForeDelivery>,
    }

    impl BodyForeOutputAdapter for StopOnFirst {
        fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
            self.deliveries.push(output.clone());
            if self.deliveries.len() == 1 {
                self.control
                    .request_stop(crate::RunControlRequestId::new("stop-todo-scan").unwrap())
                    .unwrap();
            }
            Ok(())
        }
    }

    let plan = crate::flow_activation::authored_todo_plan();
    let host_id = plan.fragments[0].host_id.clone();
    let boot_id = plan.fragments[0].boot_id.clone();
    let partition = BodyPlotPlan {
        plot: ResidentPlot::new(
            plan.source_document_id.clone(),
            plan.checked_plot_id.clone(),
        ),
        plan,
    };
    let inputs = sixty_four_commands();
    let kernel = BodyKernel::prepare(
        &[partition],
        false,
        &conduit_core::ActivePlayId::from("body/play/internal-cancellation-proof"),
        &inputs,
        true,
        true,
    )
    .unwrap();
    kernel.require_supported_execution().unwrap();
    let control = RunControl::default();
    let mut fore = StopOnFirst {
        control: control.clone(),
        deliveries: Vec::new(),
    };
    let result = kernel.run(
        &mut Vec::new(),
        &mut NoTimer,
        None,
        Some(&mut fore),
        &control,
        &host_id,
        &boot_id,
        None,
        None,
        None,
    );
    assert!(matches!(
        result.terminal,
        TerminalDisposition::Cancelled {
            reason: CancellationReason::OperatorRequested
        }
    ));
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(fore.deliveries.len(), 1);
    assert!(!result.scan_cancellation_failed);
    assert!(!result.scan_child_signs.unwrap().is_empty());
}

#[test]
fn authored_todo_prepares_typed_body_fore_with_exact_preloaded_gate() {
    let plan = crate::flow_activation::authored_todo_plan();
    assert!(conduit_core::verify_plan(&plan));
    let partition = BodyPlotPlan {
        plot: ResidentPlot::new(
            plan.source_document_id.clone(),
            plan.checked_plot_id.clone(),
        ),
        plan,
    };
    let command = ExternalForeInput {
        front_port_id: conduit_core::port_id("commands"),
        track: conduit_core::ConnectionTrack::Payload,
        bytes: conduit_todo_plot::TodoCommand::Add {
            text: "Milk".into(),
        }
        .encode_info()
        .unwrap(),
    };
    let inputs = vec![command.clone(); 64];
    let over_plan = partition.plan.clone();
    let kernel = BodyKernel::prepare(
        &[partition],
        false,
        &conduit_core::ActivePlayId::from("body/play/prepared-only"),
        &inputs,
        true,
        true,
    )
    .expect("exact Todo Plan, child pool and typed Fore must prepare");
    kernel.require_supported_execution().unwrap();
    let mut over_capacity = inputs.clone();
    over_capacity.push(command);
    assert!(BodyKernel::prepare(
        &[BodyPlotPlan {
            plot: ResidentPlot::new(
                over_plan.source_document_id.clone(),
                over_plan.checked_plot_id.clone(),
            ),
            plan: over_plan,
        }],
        false,
        &conduit_core::ActivePlayId::from("body/play/over-capacity"),
        &over_capacity,
        true,
        true,
    )
    .is_err());
}

#[test]
fn scoped_std_host_runs_public_preloaded_todo_body_with_correlated_child_signs() {
    use crate::body_execution::BodyRunRequest;
    use conduit_body::{Body, BodyPlan};
    use conduit_core::{BootId, HostId, OfferGeneration, SignId};

    let initial = conduit_todo_plot::TodoState::new("Groceries".into()).unwrap();
    let config = crate::StdHostConfig {
        host_id: HostId::from("std-todo-offer-proof"),
        boot_id: BootId::from("std-todo-offer-proof/boot"),
        offer_generation: OfferGeneration(1),
    };
    let mut host = crate::StdHost::new_for_todo_scan(config.clone(), &initial, 64).unwrap();
    let plan = crate::flow_activation::authored_todo_plan_on_host(host.advertisement(), 64);
    assert!(host
        .advertisement()
        .capabilities
        .iter()
        .any(|offer| { offer.capability_id == plan.fragments[0].placements[0].capability_id }));
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        SignId::from("sign/todo-born"),
    )
    .unwrap();
    let wake = body.wake(1, SignId::from("sign/todo-wake")).unwrap().1;
    let body_plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        }],
    )
    .unwrap();
    let inputs = sixty_four_commands();
    let control = RunControl::default();
    let mut fore = CapturedFore::default();
    let report = host
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &control,
                keyboard: None,
            },
            &inputs,
            true,
            &mut fore,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(report.play.plan_id, body_plan.plan_id);
    assert_eq!(report.play.play_sequence, 0);
    assert_eq!(report.terminal, TerminalDisposition::Completed);
    assert!(report.failure.is_none(), "{:?}", report.failure);
    assert_eq!(fore.0.len(), 64);
    let receipts = report.scan_child_signs.unwrap();
    assert_eq!(receipts.len(), 64);
    for (index, receipt) in receipts.iter().enumerate() {
        assert_eq!(receipt.parent_active_play_id, report.play.active_play_id);
        assert_eq!(usize::from(receipt.invocation), index);
        assert!(!receipt.events.is_empty());
    }

    struct StopOnFirst {
        control: RunControl,
        deliveries: usize,
    }
    impl BodyForeOutputAdapter for StopOnFirst {
        fn deliver(&mut self, _: &ExternalForeDelivery) -> Result<(), String> {
            self.deliveries += 1;
            if self.deliveries == 1 {
                self.control
                    .request_stop(crate::RunControlRequestId::new("stop-public-todo").unwrap())
                    .unwrap();
            }
            Ok(())
        }
    }
    let stop = RunControl::default();
    let mut stopping = StopOnFirst {
        control: stop.clone(),
        deliveries: 0,
    };
    let cancelled = host
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &stop,
                keyboard: None,
            },
            &inputs,
            true,
            &mut stopping,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(cancelled.play.play_sequence, 1);
    assert!(matches!(
        cancelled.terminal,
        TerminalDisposition::Cancelled {
            reason: CancellationReason::OperatorRequested
        }
    ));
    assert_eq!(stopping.deliveries, 1);
    assert!(!cancelled.scan_cancellation_failed);
    assert!(cancelled
        .scan_child_signs
        .unwrap()
        .iter()
        .all(|receipt| receipt.parent_active_play_id == cancelled.play.active_play_id));

    let wrong_initial = conduit_todo_plot::TodoState::new("Other".into()).unwrap();
    let mut wrong_host = crate::StdHost::new_for_todo_scan(config, &wrong_initial, 64).unwrap();
    let mut no_fore = CapturedFore::default();
    let refusal = wrong_host
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &control,
                keyboard: None,
            },
            &inputs,
            true,
            &mut no_fore,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap_err();
    assert!(refusal.contains("installed exact capability"), "{refusal}");
    assert!(no_fore.0.is_empty());

    let mut unscoped = crate::StdHost::new_with_config(crate::StdHostConfig {
        host_id: HostId::from("std-todo-offer-proof"),
        boot_id: BootId::from("std-todo-offer-proof/boot"),
        offer_generation: OfferGeneration(1),
    });
    let refusal = unscoped
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &control,
                keyboard: None,
            },
            &inputs,
            true,
            &mut CapturedFore::default(),
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap_err();
    assert!(refusal.contains("unavailable capability"), "{refusal}");

    let mut wrong_generation = crate::StdHost::new_for_todo_scan(
        crate::StdHostConfig {
            host_id: HostId::from("std-todo-offer-proof"),
            boot_id: BootId::from("std-todo-offer-proof/boot"),
            offer_generation: OfferGeneration(2),
        },
        &initial,
        64,
    )
    .unwrap();
    let refusal = wrong_generation
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &control,
                keyboard: None,
            },
            &inputs,
            true,
            &mut CapturedFore::default(),
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap_err();
    assert!(refusal.contains("current host boot and offer"), "{refusal}");
}

#[test]
fn scoped_todo_host_refuses_preloaded_inputs_above_selected_bound_before_play() {
    use crate::body_execution::BodyRunRequest;
    use conduit_body::{Body, BodyPlan};
    use conduit_core::{BootId, HostId, OfferGeneration, SignId};

    let mut host = crate::StdHost::new_for_todo_scan(
        crate::StdHostConfig {
            host_id: HostId::from("std-todo-offer-proof"),
            boot_id: BootId::from("std-todo-offer-proof/boot"),
            offer_generation: OfferGeneration(1),
        },
        &conduit_todo_plot::TodoState::new("Groceries".into()).unwrap(),
        2,
    )
    .unwrap();
    let plan = crate::flow_activation::authored_todo_plan_on_host(host.advertisement(), 2);
    assert!(conduit_core::verify_plan(&plan));
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        SignId::from("sign/bounded-born"),
    )
    .unwrap();
    let wake = body.wake(1, SignId::from("sign/bounded-wake")).unwrap().1;
    let body_plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        }],
    )
    .unwrap();
    let inputs = sixty_four_commands();
    let control = RunControl::default();
    let mut fore = CapturedFore::default();
    let run = |host: &mut crate::StdHost, inputs: &[ExternalForeInput], fore: &mut CapturedFore| {
        host.run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &control,
                keyboard: None,
            },
            inputs,
            true,
            fore,
            &mut Vec::new(),
            &mut NoTimer,
        )
    };
    let refusal = run(&mut host, &inputs[..3], &mut fore).unwrap_err();
    assert!(refusal.contains("selected scan bound"), "{refusal}");
    assert!(fore.0.is_empty());
    let accepted = run(&mut host, &inputs[..2], &mut fore).unwrap();
    assert_eq!(accepted.play.play_sequence, 0);
    assert_eq!(accepted.terminal, TerminalDisposition::Completed);
    assert_eq!(fore.0.len(), 2);
}

#[test]
fn installed_body_preparation_retains_exact_todo_scan_entry() {
    let plan = crate::flow_activation::tests::todo_scan_plan();
    let fragment = &plan.fragments[0];
    let partition = BodyPlotPlan {
        plot: ResidentPlot::new(
            plan.source_document_id.clone(),
            plan.checked_plot_id.clone(),
        ),
        plan: plan.clone(),
    };
    let bound = bind_body_activations(&[partition], &[fragment]).unwrap();
    assert_eq!(bound.len(), 1);
    assert_eq!(bound[0].source_plan_id, plan.plan_id);
    assert_eq!(bound[0].fragment_id, fragment.fragment_id);
    assert_eq!(bound[0].entries, plan.activations);
    assert!(
        conduit_plan_lowering::activation_fragment::verify_lowered_fragment_activations(
            &bound[0], &plan,
        )
    );
    assert!(matches!(
        bound[0].entries[0],
        conduit_core::PlannedActivationEntry::Scan(_)
    ));
    let mut substituted = fragment.clone();
    substituted.fragment_id = conduit_core::FragmentId::from("substituted");
    assert!(bind_body_activations(
        &[BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        }],
        &[&substituted],
    )
    .is_err());
}
