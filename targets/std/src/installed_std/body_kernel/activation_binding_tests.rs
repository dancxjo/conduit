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
    assert!(kernel.require_supported_execution().is_err());
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
    assert!(kernel.require_supported_execution().is_err());
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
fn authored_todo_prepares_typed_body_fore_but_cannot_start_play() {
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
    assert_eq!(
        kernel.require_supported_execution().unwrap_err(),
        "Body activation coordinator is not installed"
    );
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
