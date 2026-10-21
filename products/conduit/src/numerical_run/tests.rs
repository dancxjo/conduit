use super::*;

fn prepared() -> (StdHost, Plan) {
    let source = crate::plot_source::parse(include_str!(
        "../../../../plots/quantity-projection/main.conduit"
    ))
    .unwrap();
    let checked = source.check().unwrap();
    let authored = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "quantity-projection/main",
        source.authoring_catalog(),
    )
    .unwrap();
    prepare(&source, &checked, &authored).unwrap()
}

#[test]
fn empty_numerical_fore_does_not_report_unexecuted_reservations() {
    let (host, plan) = prepared();
    let report = NumericalFore::from_plan(host, plan)
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(report.terminal, TerminalDisposition::Completed);
    assert!(report.each_child_signs.unwrap().is_empty());
}

#[test]
fn malformed_input_does_not_spend_a_value_and_stop_retains_accepted_evidence() {
    let (host, plan) = prepared();
    let mut execution = NumericalFore::from_plan(host, plan).unwrap();
    assert!(execution.execute_encoded(&[0; 3]).is_err());
    execution.execute(Quantity::new(3, Unit::Meter)).unwrap();
    let report = execution.stop().unwrap();
    assert_ne!(report.terminal, TerminalDisposition::Completed);
    let receipts = report.each_child_signs.unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].invocation, 0);
    assert!(!receipts[0].remote_lifecycle.is_empty());
    assert!(!report.each_cancellation_failed);
}

#[test]
fn producer_pressure_and_close_do_not_admit_an_extra_value() {
    use conduit_std_host::{BodyLiveForeAdmission, BodyLiveForeQueue, RunControl};
    let (_, plan) = prepared();
    let control = RunControl::default();
    let queue = BodyLiveForeQueue::for_near_each_plan(&plan, control, 1).unwrap();
    let bytes = Quantity::new(3, Unit::Meter).encode();
    assert_eq!(
        queue.submit(&bytes).unwrap(),
        BodyLiveForeAdmission::Accepted { sequence: 0 }
    );
    assert_eq!(queue.submit(&bytes).unwrap(), BodyLiveForeAdmission::Full);
    queue.close().unwrap();
    assert!(queue.submit(&bytes).is_err());
}

#[test]
fn substituted_plan_and_replacement_host_cannot_start_the_numerical_owner() {
    let (host, mut plan) = prepared();
    plan.plan_id = "foreign-plan".into();
    assert!(NumericalFore::from_plan(host, plan).is_err());
    let (_, plan) = prepared();
    let replacement = StdHost::new_with_composition(
        StdHostConfig {
            host_id: plan.fragments[0].host_id.clone(),
            boot_id: "replacement-boot".into(),
            offer_generation: plan.fragments[0].offer_generation,
        },
        StdHostComposition::minimal(),
    );
    assert!(NumericalFore::from_plan(replacement, plan).is_err());
}

#[test]
fn numerical_child_keeps_pending_output_until_the_exact_ack_and_cancels_without_replay() {
    use conduit_composite::{
        BoundedActivationAdmission as Admission, BoundedActivationState as State,
    };
    use conduit_std_host::flow_activation::{
        install_reusable_planned_activation, standard_child_registry, StdActivationHost,
    };
    let (_, plan) = prepared();
    let PlannedActivationEntry::Unary(planned) = &plan.activations[0] else {
        panic!()
    };
    let fragment = &plan.fragments[0];
    let mut host = StdActivationHost::new(
        PreparationHostIdentity {
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            offer_generation: fragment.offer_generation,
        },
        standard_child_registry().unwrap(),
    );
    let mut receipts = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    let mut each = install_reusable_planned_activation(
        &plan,
        &mut receipts,
        &planned.activation_id,
        &mut host,
    )
    .unwrap()
    .into_unary()
    .unwrap();
    assert!(install_reusable_planned_activation(
        &plan,
        &mut receipts,
        &planned.activation_id,
        &mut host
    )
    .is_err());
    each.bind_parent_play(&ActivePlayId::from("numerical/acceptance-control"))
        .unwrap();
    let input = ValuePayload {
        value_kind: QUANTITY_INFO_ID.into(),
        encoded: Quantity::new(3, Unit::Meter).encode().to_vec(),
    };
    assert_eq!(
        each.activate(0, &input).unwrap(),
        Admission::Accepted { sequence: 0 }
    );
    for _ in 0..8 {
        if each.output().unwrap().is_some() {
            break;
        }
        each.step().unwrap();
    }
    let (sequence, output) = each.output().unwrap().unwrap();
    assert_eq!(sequence, 0);
    let pending = output.clone();
    assert!(each.complete_output(1).is_err());
    assert_eq!(
        each.activate(1, &input).unwrap(),
        Admission::Full { sequence: 1 }
    );
    each.step().unwrap();
    assert_eq!(each.output().unwrap().unwrap().1, &pending);
    each.complete_output(0).unwrap();
    for _ in 0..8 {
        if each.step().unwrap() == &(State::Succeeded { sequence: 0 }) {
            break;
        }
    }
    assert!(each.activate(0, &input).is_err());
    assert_eq!(
        each.activate(1, &input).unwrap(),
        Admission::Accepted { sequence: 1 }
    );
    each.cancel().unwrap();
    assert!(each.activate(2, &input).is_err());
    let evidence = each.child_sign_receipts().unwrap();
    assert_eq!(evidence.len(), 2);
    assert_ne!(
        evidence[0].child_active_play_id,
        evidence[1].child_active_play_id
    );
}

#[test]
fn numerical_physical_refusal_keeps_failed_child_evidence_without_an_accepted_output() {
    let (host, plan) = prepared();
    let mut execution = NumericalFore::from_plan(host, plan).unwrap();
    assert!(execution
        .execute(Quantity::parse_plot_literal("3s").unwrap())
        .is_err());
    let report = execution.finish().unwrap();
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Failed { .. }
    ));
    assert!(report.kernel_failure.is_some());
    assert!(report.fore_deliveries.is_empty());
    let receipts = report.each_child_signs.unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].invocation, 0);
    assert!(!receipts[0].events.is_empty());
    assert!(!receipts[0].remote_lifecycle.is_empty());
}

#[test]
fn refused_external_fore_delivery_is_not_reported_as_accepted_or_replayed() {
    use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
    use conduit_std_host::{
        body_execution::{BodyForeOutputAdapter, BodyRunRequest},
        BodyLiveForeQueue, ExternalForeDelivery, RunControl, ThreadTimer,
    };
    struct Refuse;
    impl BodyForeOutputAdapter for Refuse {
        fn deliver(&mut self, _: &ExternalForeDelivery) -> Result<(), String> {
            Err("consumer refused delivery".into())
        }
    }
    let (mut host, plan) = prepared();
    std::thread::Builder::new()
        .stack_size(4 * 1024 * 1024)
        .spawn(move || {
            let body = Body::born(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
                1,
                "numerical/refused-fore-born".into(),
            )
            .unwrap();
            let wake = body
                .wake(1, "numerical/refused-fore-wake".into())
                .unwrap()
                .1;
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
            let control = RunControl::default();
            let queue =
                BodyLiveForeQueue::for_near_each_plan(&body_plan.plots[0].plan, control.clone(), 1)
                    .unwrap();
            queue
                .submit(&Quantity::new(3, Unit::Meter).encode())
                .unwrap();
            queue.close().unwrap();
            let report = host
                .run_body_plan_with_live_fore_to(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &body_plan,
                        control: &control,
                        keyboard: None,
                    },
                    &queue,
                    &mut Refuse,
                    &mut Vec::new(),
                    &mut ThreadTimer,
                )
                .unwrap();
            assert!(matches!(
                report.terminal,
                TerminalDisposition::Failed { .. }
            ));
            assert!(report.fore_deliveries.is_empty());
            assert!(report
                .failure
                .as_deref()
                .unwrap()
                .contains("consumer refused delivery"));
            let children = report.each_child_signs.unwrap();
            assert_eq!(children.len(), 1);
            assert_eq!(children[0].invocation, 0);
            assert!(!children[0].events.is_empty());
        })
        .unwrap()
        .join()
        .unwrap();
}
