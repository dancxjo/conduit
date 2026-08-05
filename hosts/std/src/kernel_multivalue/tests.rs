use super::{
    advertisement, execute_fragment, execute_fragment_with_options, plan_local,
    profile_catalog, ExecutionOptions, InjectedBoundaryFault,
};
use crate::TimerAdapter;
use conduit_core::{
    bind_evidence, BootId, HostAdvertisement, HostId, OfferGeneration, PlanFragment,
};
use conduit_form::parse;
use conduit_runtime::lowering::{lower_plan_fragment, ExecutionIdentityError};
use std::time::Duration;

#[derive(Default)]
struct VirtualTimer {
    waits: Vec<Duration>,
}

impl TimerAdapter for VirtualTimer {
    fn wait(&mut self, duration: Duration) {
        self.waits.push(duration);
    }
}

fn planned_fixture() -> (HostAdvertisement, PlanFragment) {
    let form = parse(
        include_str!("../../../../examples/kernel-multivalue.form"),
        &profile_catalog(),
    )
    .expect("typed multi-value form parses");
    let host = advertisement(
        HostId::from("std-kernel-multivalue"),
        BootId::from("std-kernel-multivalue-boot"),
        OfferGeneration(1),
    );
    let plan = plan_local(&form, &host).expect("typed multi-value form plans");
    (host, plan.fragments[0].clone())
}

#[test]
fn exact_multi_value_form_plans_and_lowers_all_numeric_tables() {
    let form = parse(
        include_str!("../../../../examples/kernel-multivalue.form"),
        &profile_catalog(),
    )
    .expect("typed multi-value form parses");
    let host = advertisement(
        HostId::from("std-kernel-multivalue"),
        BootId::from("std-kernel-multivalue-boot"),
        OfferGeneration(1),
    );
    let plan = plan_local(&form, &host).expect("typed multi-value form plans");
    let lowered = lower_plan_fragment(&plan.fragments[0]).expect("fragment lowers");
    assert_eq!(lowered.nodes.len(), 6);
    assert_eq!(lowered.cords.len(), 5);
    assert_eq!(lowered.routes.len(), 5);
    assert_eq!(lowered.host_operations.len(), 3);
    assert_eq!(lowered.resources.len(), 3);
    assert_eq!(lowered.cord_value_slots, 5);
    assert_eq!(lowered.cord_value_bytes, 40);
}

#[test]
fn exact_multi_value_form_executes_real_host_operations_through_kernel() {
    let (host, fragment) = planned_fixture();
    let mut output = Vec::with_capacity(65_536);
    let mut timer = VirtualTimer {
        waits: Vec::with_capacity(4),
    };
    let mut evidence_sequence = 0;
    let mut report = execute_fragment(
        &host,
        &fragment,
        0,
        &mut evidence_sequence,
        &mut output,
        &mut timer,
    )
    .expect("multi-value kernel execution completes");

    assert_eq!(timer.waits, vec![Duration::ZERO; 4]);
    assert_eq!(
        report
            .receipts
            .iter()
            .map(|receipt| receipt.tick)
            .collect::<Vec<_>>(),
        [0, 3, 2]
    );
    let output = String::from_utf8(output).expect("output is utf-8");
    assert!(output.contains("tick even 0"));
    assert!(output.contains("tick even 2"));
    assert!(output.contains("tick latest 3"));
    assert!(report.decisions > 0);
    assert!(report.kernel_events > 0);
    assert_eq!(
        report.value_allocation_capacity_before,
        report.value_allocation_capacity_after
    );
    assert_eq!(report.presentation_ids.len(), 3);
    assert_eq!(report.pressure_items, 1);
    assert_eq!(report.pressure_bytes, 8);
    assert_eq!(report.input_closed_events, 5);
    assert!(report.terminal_order_exact);
    assert_eq!(report.identity.plan_id, fragment.plan_id);
    assert_eq!(report.identity.active_play_id, report.active_play_id);
    assert_eq!(report.identity.lengths(), (7, 3, 4));
    assert_eq!(report.post_activation_allocations, 0);
    for observation in &report.observations {
        let evidence = report
            .identity
            .evidence(&observation.evidence_id)
            .expect("host evidence reverses to its kernel identity row");
        assert_eq!(
            evidence.presentation_id.as_ref(),
            observation.presentation_id.as_ref()
        );
        let Some(presentation) = observation.presentation_id.as_ref() else {
            continue;
        };
        let dynamic = report
            .identity
            .presentation(presentation)
            .expect("presentation reverses to one kernel request");
        let request = report
            .identity
            .request(dynamic.node, dynamic.request)
            .expect("presentation request reverses to its host-operation contract");
        assert!(report
            .identity
            .request_for_contract(dynamic.node, &request.contract_id)
            .any(|candidate| candidate == request));
        assert_eq!(
            report
                .identity
                .presentation_for_request(dynamic.node, dynamic.request)
                .map(|identity| &identity.presentation_id),
            Some(presentation)
        );
        assert_eq!(
            report
                .identity
                .evidence_for_presentation(presentation)
                .map(|identity| &identity.evidence_id),
            Some(&observation.evidence_id)
        );
    }
    assert_eq!(report.observations.len(), 4);
    assert_eq!(evidence_sequence, 4);
    let wrong_host_evidence = bind_evidence(
        &HostId::from("wrong-host"),
        &host.boot_id,
        Some(&report.active_play_id),
        99,
    );
    assert_eq!(
        report
            .identity
            .bind_evidence(&wrong_host_evidence, None, None, None),
        Err(ExecutionIdentityError::WrongHost)
    );
}

#[test]
fn stale_completion_identity_fails_closed_before_host_effect() {
    let (host, fragment) = planned_fixture();
    let mut timer = VirtualTimer::default();
    let error = execute_fragment_with_options(
        &host,
        &fragment,
        0,
        &mut 0,
        &mut Vec::new(),
        &mut timer,
        ExecutionOptions {
            fault: InjectedBoundaryFault::StaleCompletion,
            ..ExecutionOptions::default()
        },
    )
    .expect_err("stale request identity must fail closed");
    assert!(error.contains("HostOperationCompletionRejected"), "{error}");
    assert!(timer.waits.is_empty());
}

#[test]
fn cancellation_clears_pending_work_and_terminates_without_effects() {
    let (host, fragment) = planned_fixture();
    let mut timer = VirtualTimer::default();
    let error = execute_fragment_with_options(
        &host,
        &fragment,
        0,
        &mut 0,
        &mut Vec::new(),
        &mut timer,
        ExecutionOptions {
            fault: InjectedBoundaryFault::CancelBeforeDispatch,
            ..ExecutionOptions::default()
        },
    )
    .expect_err("cancelled execution must not report completion");
    assert!(error.contains("kernel was cancelled"), "{error}");
    assert!(timer.waits.is_empty());
}

#[test]
fn evidence_exhaustion_fails_closed_inside_the_installed_scheduler() {
    let (host, fragment) = planned_fixture();
    let mut timer = VirtualTimer::default();
    let error = execute_fragment_with_options(
        &host,
        &fragment,
        0,
        &mut 0,
        &mut Vec::new(),
        &mut timer,
        ExecutionOptions {
            evidence_items: 1,
            ..ExecutionOptions::default()
        },
    )
    .expect_err("evidence budget exhaustion must fail closed");
    assert!(error.contains("ItemCapacityExceeded"), "{error}");
}
