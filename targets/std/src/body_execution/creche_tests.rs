//! Shipped Source through the production Body runner with fixture effects.
//! This proves software routing and custody, not native rendering or manufacture.
use super::*;
use crate::hosted_creche::test_support::{planned, Adapter};
use conduit_body_make::deployment_runtime::{CrecheDisposition, CrecheOutcome};

struct Timer;
impl TimerAdapter for Timer {
    fn wait(&mut self, _: std::time::Duration) {}
}
struct Outputs(Vec<ExternalForeDelivery>);
impl BodyForeOutputAdapter for Outputs {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        if self.0.len() == self.0.capacity() {
            return Err("fixture output bound".into());
        }
        self.0.push(output.clone());
        Ok(())
    }
}

fn run(adapter: &mut Adapter, control: &RunControl) -> (BodyRunReport, Outputs) {
    let (advertisement, plan, _, wake) = planned(adapter);
    let mut host = StdHost::from_advertisement(advertisement).unwrap();
    let mut outputs = Outputs(Vec::with_capacity(1));
    let report = host
        .run_body_plan_with_creche_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control,
                keyboard: None,
            },
            adapter,
            &mut outputs,
            &mut Vec::with_capacity(2048),
            &mut Timer,
            |play, wake| {
                assert_eq!(play.body_id, wake.body_id);
                Ok(())
            },
        )
        .unwrap();
    (report, outputs)
}

#[test]
fn shipped_plot_uses_installed_host_call_and_exact_acknowledged_outcome() {
    let mut adapter = Adapter::new();
    let (report, outputs) = run(&mut adapter, &RunControl::default());
    assert_eq!(report.terminal, TerminalDisposition::Completed);
    assert!(report.failure.is_none());
    assert!(report.cleanup_failure.is_none());
    assert_eq!(report.requests.len(), 1);
    assert_eq!(outputs.0.len(), 1);
    let outcome = CrecheOutcome::decode(&outputs.0[0].bytes).unwrap();
    assert_eq!(outcome.request, adapter.request.unwrap());
    assert_eq!(outcome.disposition, CrecheDisposition::Refused);
    assert_eq!((adapter.begins, adapter.polls, adapter.drains), (1, 1, 1));
    assert_eq!(report.fore_deliveries, outputs.0);
}

#[test]
fn corrupted_receipt_cannot_escape_as_a_prepared_outcome() {
    let mut adapter = Adapter::new();
    adapter.corrupt = true;
    let (report, outputs) = run(&mut adapter, &RunControl::default());
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Failed { .. }
    ));
    assert!(report.failure.as_ref().unwrap().contains("custody"));
    assert!(outputs.0.is_empty());
    assert_eq!((adapter.begins, adapter.polls, adapter.drains), (1, 1, 1));
}

#[test]
fn operator_stop_drains_pending_effect_before_kernel_cancellation() {
    let control = RunControl::default();
    let mut adapter = Adapter::new();
    adapter.pending = true;
    adapter.stop_on_begin = Some(control.clone());
    let (report, outputs) = run(&mut adapter, &control);
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    assert!(report.cleanup_failure.is_none());
    assert!(outputs.0.is_empty());
    assert_eq!((adapter.begins, adapter.drains), (1, 1));
}

#[test]
fn failed_platform_drain_preserves_failure_and_never_produces_an_outcome() {
    let control = RunControl::default();
    let mut adapter = Adapter::new();
    adapter.pending = true;
    adapter.stop_on_begin = Some(control.clone());
    adapter.fail_drain = true;
    let (report, outputs) = run(&mut adapter, &control);
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Failed { .. }
    ));
    assert!(report
        .cleanup_failure
        .as_ref()
        .unwrap()
        .contains("drain failure"));
    assert!(outputs.0.is_empty());
    assert_eq!(adapter.begins, 1);
}

#[test]
fn refused_start_drains_preparation_without_invoking_effect() {
    let mut adapter = Adapter::new();
    let (advertisement, plan, _, wake) = planned(&adapter);
    let mut host = StdHost::from_advertisement(advertisement).unwrap();
    let mut outputs = Outputs(Vec::with_capacity(1));
    let error = host
        .run_body_plan_with_creche_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &mut adapter,
            &mut outputs,
            &mut Vec::with_capacity(2048),
            &mut Timer,
            |_, _| Err("fixture start refusal".into()),
        )
        .unwrap_err();
    assert!(error.contains("start refusal"));
    assert_eq!((adapter.begins, adapter.drains), (0, 1));
    assert!(outputs.0.is_empty());
}

#[test]
fn ordinary_body_entry_refuses_an_unattached_encounter_before_play() {
    let adapter = Adapter::new();
    let (advertisement, plan, _, wake) = planned(&adapter);
    let mut host = StdHost::from_advertisement(advertisement).unwrap();
    let mut outputs = Outputs(Vec::with_capacity(1));
    let input = ExternalForeInput {
        front_port_id: conduit_core::port_id("request"),
        track: conduit_core::ConnectionTrack::Payload,
        bytes: vec![0; 32],
    };
    let error = host
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &[input],
            false,
            &mut outputs,
            &mut Vec::with_capacity(2048),
            &mut Timer,
        )
        .unwrap_err();
    assert!(error.contains("Creche"));
    assert!(outputs.0.is_empty());
}

#[test]
fn uncertain_creche_retirement_keeps_charge_and_refuses_next_actual_admission() {
    let control = RunControl::default();
    let mut adapter = Adapter::new();
    adapter.pending = true;
    adapter.stop_on_begin = Some(control.clone());
    adapter.fail_drain = true;
    let (advertisement, plan, _, wake) = planned(&adapter);
    let mut host = StdHost::from_advertisement(advertisement.clone()).unwrap();
    let mut outputs = Outputs(Vec::with_capacity(1));
    let report = host
        .run_body_plan_with_creche_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &mut adapter,
            &mut outputs,
            &mut Vec::new(),
            &mut Timer,
            |_, _| Ok(()),
        )
        .unwrap();
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Failed { .. }
    ));
    assert!(report.failure.is_some());
    assert!(report
        .cleanup_failure
        .as_ref()
        .unwrap()
        .contains("quarantined"));
    assert!(outputs.0.is_empty());
    assert!(!host.kernel_resources.is_idle());
    assert!(host
        .kernel_resources
        .observe_offers(&advertisement, &[], &[])
        .unwrap_err()
        .contains("quarantined"));
    assert!(host
        .kernel_resources
        .prepare_and_reserve_plans(&advertisement, &[(&plan.plots[0].plan, false)])
        .unwrap_err()
        .contains("quarantined"));
    let mut next = Adapter::new();
    let mut started = false;
    let refusal = host
        .run_body_plan_with_creche_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &mut next,
            &mut outputs,
            &mut Vec::new(),
            &mut Timer,
            |_, _| {
                started = true;
                Ok(())
            },
        )
        .unwrap_err();
    assert!(refusal.contains("quarantined"));
    assert!(!started);
    assert_eq!(next.begins, 0);
    assert!(outputs.0.is_empty());
    // A later adapter state change is not a release/re-admission witness.
    adapter.fail_drain = false;
    assert!(host
        .kernel_resources
        .observe_offers(&advertisement, &[], &[])
        .is_err());
}

#[test]
fn failed_preparation_drain_quarantines_before_any_execution_charge() {
    let mut adapter = Adapter::new();
    let (advertisement, plan, _, wake) = planned(&adapter);
    let mut host = StdHost::from_advertisement(advertisement).unwrap();
    adapter.limits.maximum_polls = 0;
    adapter.fail_drain = true;
    let mut outputs = Outputs(Vec::new());
    let error = host
        .run_body_plan_with_creche_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &mut adapter,
            &mut outputs,
            &mut Vec::new(),
            &mut Timer,
            |_, _| panic!("preparation refusal must precede Play"),
        )
        .unwrap_err();
    assert!(error.contains("preparation drain failed"));
    assert_eq!(adapter.begins, 0);
    assert!(outputs.0.is_empty());
    assert!(host
        .kernel_resources
        .require_available()
        .unwrap_err()
        .contains("quarantined"));
    let mut next = Adapter::new();
    let error = host
        .run_body_plan_with_creche_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &mut next,
            &mut outputs,
            &mut Vec::new(),
            &mut Timer,
            |_, _| panic!("quarantined Host must not start"),
        )
        .unwrap_err();
    assert!(error.contains("quarantined"));
    assert_eq!((next.begins, next.drains), (0, 0));
}
