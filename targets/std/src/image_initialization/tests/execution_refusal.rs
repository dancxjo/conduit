use super::*;
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};

/// Any adapter entry is observable work, including reading the timer basis.
/// The selected scalar-only IMAGE must refuse before entering the adapter.
struct UntouchedTimer;
impl crate::TimerAdapter for UntouchedTimer {
    fn observe_kernel_phase(&mut self, _phase: crate::KernelTimingPhase) {
        panic!("refused Play entered timer phase observation");
    }
    fn wait(&mut self, _duration: std::time::Duration) {
        panic!("refused Play waited on the timer");
    }
    fn monotonic_observation(
        &mut self,
        _host_id: &HostId,
        _boot_id: &BootId,
    ) -> Option<MonotonicInstant> {
        panic!("refused Play sampled the timer basis");
    }
    fn monotonic_now_ms(&mut self) -> Option<u64> {
        panic!("refused Play sampled timer milliseconds");
    }
    fn monotonic_now_micros(&mut self) -> Option<u64> {
        panic!("refused Play sampled timer microseconds");
    }
    fn wait_until_monotonic_ms(&mut self, _deadline_ms: u64) -> bool {
        panic!("refused Play waited on a timer deadline");
    }
}

#[test]
fn selected_timer_public_body_runner_refuses_before_started_or_any_execution_effect() {
    let selected = image(SOURCE);
    let mut current = host(&selected, "boot/execution-refusal").unwrap();
    let constituent = plan(&current);
    let resident = ResidentPlot::new(
        constituent.source_document_id.clone(),
        constituent.checked_plot_id.clone(),
    );
    let body = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        SignId::from("sign/image-refusal-born"),
    )
    .unwrap();
    let (_awake, wake) = body
        .wake(1, SignId::from("sign/image-refusal-wake"))
        .unwrap();
    let body_plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: resident,
            plan: constituent,
        }],
    )
    .unwrap();
    // Establish that this is a valid canonical proposal, not an invalid Body
    // or fabricated Plan used to trigger an unrelated early rejection.
    let prospective = current.prospective_body_play(&wake, &body_plan).unwrap();
    let sequence = current.next_kernel_play_sequence;
    let sign_sequence = current.next_kernel_sign_sequence;
    let resources = format!("{:?}", current.kernel_resources);
    let mut output = b"retained output".to_vec();
    let before_output = output.clone();
    let control = crate::RunControl::default();
    let mut started = false;
    let error = current
        .run_body_plan_to_with_start(
            crate::body_execution::BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &control,
                keyboard: None,
            },
            &mut output,
            &mut UntouchedTimer,
            |_, _| {
                started = true;
                Ok(())
            },
        )
        .unwrap_err();
    // The scalar checks precede this refusal; a scalar shortage has its own
    // error and must not accidentally make this missing-receipt test pass.
    assert!(
        error.contains("complete ordinary managed-memory accounting"),
        "{error}"
    );
    assert!(!started);
    assert_eq!(output, before_output);
    assert_eq!(current.next_kernel_play_sequence, sequence);
    assert_eq!(current.next_kernel_sign_sequence, sign_sequence);
    assert_eq!(format!("{:?}", current.kernel_resources), resources);
    assert!(current.kernel_resources.is_idle());
    assert_eq!(
        current.prospective_body_play(&wake, &body_plan).unwrap(),
        prospective
    );
}
