//! Ordinary installed execution, exact old custody and no implicit effect retries.
#[path = "replan_common/conformance_costs.rs"]
mod fixed_costs;
#[path = "replan_common/expression.rs"]
mod fixture;
use conduit_core::ExactNumericWork;
use conduit_planner::resource_cost::*;
use conduit_std_host::*;
use fixed_costs::with_request;
use fixture::*;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}
#[test]
fn persistent_benefit_enters_ordinary_fresh_play_and_preserves_the_old_report() {
    let mut case = case();
    let old = old_run(&mut case);
    let before = format!("{old:?}");
    let mut gate = ReluctantReplanGate::default();
    let mut outputs = Outputs::default();
    let result = with_request(
        &mut case,
        &old,
        &inputs(7),
        [100; 3],
        1201,
        |host, request| {
            host.run_reluctant_replacement_to(
                &mut gate,
                request,
                &mut outputs,
                &mut Vec::new(),
                &mut NoTimer,
                &mut work(),
            )
        },
    )
    .unwrap();
    let new = result.replacement_report.unwrap();
    assert!(result.decision.recommends_replacement());
    assert_ne!(
        new.kernel.as_ref().unwrap().active_play_id,
        old.kernel.as_ref().unwrap().active_play_id
    );
    assert_eq!(
        new.kernel.as_ref().unwrap().active_play.plan_id,
        case.candidate.plan_id
    );
    assert_eq!(outputs.0.len(), 1);
    assert_eq!(&*outputs.0[0].bytes, &231u64.to_le_bytes());
    assert_eq!(format!("{old:?}"), before);
    assert!(conduit_core::verify_plan(&case.previous));
    assert!(conduit_core::verify_plan(&case.candidate));
}
#[test]
fn noisy_model_keeps_the_current_plan_without_calling_the_adapter() {
    let mut case = case();
    let old = old_run(&mut case);
    let mut gate = ReluctantReplanGate::default();
    let mut outputs = Outputs::default();
    let result = with_request(
        &mut case,
        &old,
        &inputs(7),
        [100, 10, 100],
        1201,
        |host, request| {
            host.run_reluctant_replacement_to(
                &mut gate,
                request,
                &mut outputs,
                &mut Vec::new(),
                &mut NoTimer,
                &mut work(),
            )
        },
    )
    .unwrap();
    assert!(result.replacement_report.is_none());
    assert_eq!(
        result.decision.keep_current,
        Some(KeepCurrentPlanReason::NetBenefitNotPersistent)
    );
    assert!(outputs.0.is_empty());
}
#[test]
fn changed_request_and_uncertain_old_outcome_refuse_before_consuming_the_gate() {
    let mut case = case();
    let old = old_run(&mut case);
    let mut gate = ReluctantReplanGate::default();
    let mut outputs = Outputs::default();
    let error = with_request(
        &mut case,
        &old,
        &inputs(8),
        [100; 3],
        1201,
        |host, request| {
            host.run_reluctant_replacement_to(
                &mut gate,
                request,
                &mut outputs,
                &mut Vec::new(),
                &mut NoTimer,
                &mut work(),
            )
        },
    )
    .unwrap_err();
    assert!(matches!(
        error.reason,
        StdReluctantReplanRefusal::RequestChanged
    ));
    assert!(error.decision.is_none());
    let mut uncertain = old.clone();
    uncertain
        .observations
        .retain(|o| !matches!(o.kind, conduit_core::ObservationKind::PlanTerminal { .. }));
    let error = with_request(
        &mut case,
        &uncertain,
        &inputs(7),
        [100; 3],
        1201,
        |host, request| {
            host.run_reluctant_replacement_to(
                &mut gate,
                request,
                &mut outputs,
                &mut Vec::new(),
                &mut NoTimer,
                &mut work(),
            )
        },
    )
    .unwrap_err();
    assert!(matches!(
        error.reason,
        StdReluctantReplanRefusal::PreviousOutcomeUnavailable
    ));
    assert!(error.decision.is_none());
    assert!(outputs.0.is_empty());
    assert!(with_request(
        &mut case,
        &old,
        &inputs(7),
        [100; 3],
        1201,
        |host, request| host.run_reluctant_replacement_to(
            &mut gate,
            request,
            &mut outputs,
            &mut Vec::new(),
            &mut NoTimer,
            &mut work()
        )
    )
    .unwrap()
    .replacement_report
    .is_some());
}
#[test]
fn current_offer_loss_cannot_use_the_snapshot_as_authority() {
    let mut case = case();
    let old = old_run(&mut case);
    let mut gate = ReluctantReplanGate::default();
    let mut outputs = Outputs::default();
    let error = with_request(
        &mut case,
        &old,
        &inputs(7),
        [100; 3],
        1201,
        |host, request| {
            let mut changed = host.advertisement().clone();
            changed.offer_generation.0 += 1;
            changed.capabilities.retain(|c| {
                c.implementation.implementation_id.as_str()
                    != conduit_std_offers::PURE_SCALAR_TAPE_STD_IMPLEMENTATION
            });
            *host = StdHost::from_advertisement(changed).unwrap();
            host.run_reluctant_replacement_to(
                &mut gate,
                request,
                &mut outputs,
                &mut Vec::new(),
                &mut NoTimer,
                &mut work(),
            )
        },
    )
    .unwrap_err();
    assert!(matches!(
        error.reason,
        StdReluctantReplanRefusal::CurrentOfferChanged
    ));
    assert!(error.decision.is_none());
    assert!(outputs.0.is_empty());
}
struct AcceptThenFail {
    calls: usize,
    retained: Vec<ExternalForeDelivery>,
}
impl ExternalForeOutputAdapter for AcceptThenFail {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        self.calls += 1;
        self.retained.push(value);
        Err("external acknowledgement unknown".into())
    }
}
#[test]
fn uncertain_new_delivery_retains_the_recommendation_and_never_automatically_replays() {
    let mut case = case();
    let old = old_run(&mut case);
    let before = format!("{old:?}");
    let mut gate = ReluctantReplanGate::default();
    let mut output = AcceptThenFail {
        calls: 0,
        retained: Vec::with_capacity(1),
    };
    let error = with_request(
        &mut case,
        &old,
        &inputs(7),
        [100; 3],
        1201,
        |host, request| {
            host.run_reluctant_replacement_to(
                &mut gate,
                request,
                &mut output,
                &mut Vec::new(),
                &mut NoTimer,
                &mut work(),
            )
        },
    )
    .unwrap_err();
    assert!(
        matches!(&error.reason,StdReluctantReplanRefusal::Execution(text) if text.contains("external acknowledgement unknown"))
    );
    assert!(error.decision.unwrap().recommends_replacement());
    assert_eq!(output.calls, 1);
    assert_eq!(&*output.retained[0].bytes, &231u64.to_le_bytes());
    let next = with_request(
        &mut case,
        &old,
        &inputs(7),
        [100; 3],
        1202,
        |host, request| {
            host.run_reluctant_replacement_to(
                &mut gate,
                request,
                &mut output,
                &mut Vec::new(),
                &mut NoTimer,
                &mut work(),
            )
        },
    )
    .unwrap();
    assert!(next.replacement_report.is_none());
    assert_eq!(
        next.decision.keep_current,
        Some(KeepCurrentPlanReason::AlreadyRecommended)
    );
    assert_eq!(output.calls, 1);
    assert_eq!(format!("{old:?}"), before);
}

#[test]
fn foreign_duplicate_and_oversized_terminal_evidence_refuse_before_execution() {
    let mut case = case();
    let old = old_run(&mut case);
    let mut gate = ReluctantReplanGate::default();
    let mut output = Outputs::default();
    let mut foreign = old.clone();
    foreign.kernel.as_mut().unwrap().active_play.plan_id = case.candidate.plan_id.clone();
    let mut duplicate = old.clone();
    let terminal = duplicate
        .observations
        .iter()
        .find(|o| matches!(o.kind, conduit_core::ObservationKind::PlanTerminal { .. }))
        .unwrap()
        .clone();
    duplicate.observations.push(terminal);
    let mut oversized = old.clone();
    oversized.observations.resize(
        STD_REPLAN_PREVIOUS_EVIDENCE_CAPACITY + 1,
        old.observations[0].clone(),
    );
    let mut incomplete = old.clone();
    let k = incomplete.kernel.as_mut().unwrap();
    k.kernel_sign.clear();
    k.kernel_events = 0;
    let mut wrong_map = old.clone();
    wrong_map.kernel.as_mut().unwrap().identity.plan_id = case.candidate.plan_id.clone();
    for invalid in [&foreign, &duplicate, &oversized, &incomplete, &wrong_map] {
        let error = with_request(
            &mut case,
            invalid,
            &inputs(7),
            [100; 3],
            1201,
            |host, request| {
                host.run_reluctant_replacement_to(
                    &mut gate,
                    request,
                    &mut output,
                    &mut std::io::sink(),
                    &mut NoTimer,
                    &mut work(),
                )
            },
        )
        .unwrap_err();
        assert!(matches!(
            error.reason,
            StdReluctantReplanRefusal::PreviousOutcomeUnavailable
        ));
        assert!(error.decision.is_none());
        assert!(output.0.is_empty());
    }
    assert!(with_request(
        &mut case,
        &old,
        &inputs(7),
        [100; 3],
        1201,
        |host, request| host.run_reluctant_replacement_to(
            &mut gate,
            request,
            &mut output,
            &mut std::io::sink(),
            &mut NoTimer,
            &mut work()
        )
    )
    .unwrap()
    .replacement_report
    .is_some());
}

#[test]
fn direct_local_back_uses_the_same_request_and_ordinary_kernel_boundary() {
    let config = StdHostConfig {
        host_id: "host/replan-local-conformance".into(),
        boot_id: "boot/replan-local-conformance".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    };
    let mut case = case_with_profile(config, true);
    let old = old_run(&mut case);
    let mut gate = ReluctantReplanGate::default();
    let mut output = Outputs::default();
    let result = with_request(
        &mut case,
        &old,
        &inputs(7),
        [100; 3],
        1201,
        |host, request| {
            host.run_reluctant_replacement_to(
                &mut gate,
                request,
                &mut output,
                &mut std::io::sink(),
                &mut NoTimer,
                &mut work(),
            )
        },
    )
    .unwrap();
    let report = result.replacement_report.unwrap();
    let kernel = report.kernel.unwrap();
    assert_eq!(
        case.candidate.fragments[0].placements[0]
            .implementation_id
            .as_str(),
        conduit_std_offers::PURE_EXPRESSION_LOCAL_IMPLEMENTATION
    );
    assert!(!kernel
        .kernel_sign
        .iter()
        .any(|e| e.kind == conduit_kernel::KernelEventKind::HostCallRequested));
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(&*output.0[0].bytes, &231u64.to_le_bytes());
}
