//! A provider-owned error cannot inflate selected retained result backing.
use super::*;
#[test]
fn adapter_refusal_uses_bounded_result_and_escaped_bytes_keep_charge() {
    struct Refusal {
        error: Option<String>,
        bytes: Option<crate::ExternalForeBytes>,
    }
    impl BodyForeOutputAdapter for Refusal {
        fn deliver(&mut self, output: &crate::ExternalForeDelivery) -> Result<(), String> {
            self.bytes = Some(output.bytes.clone());
            Err(self.error.take().unwrap())
        }
    }
    let (mut host, wake, plan) = fixture(64 * 1024 * 1024);
    let observed = host.kernel_resources.image_memory_observer();
    let mut adapter = Refusal {
        error: Some("provider-error".repeat(1024)),
        bytes: None,
    };
    let control = RunControl::default();
    let mut probe = None;
    let result = host
        .run_selected_image_body_plan_with_output_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &mut Vec::new(),
            &mut crate::ThreadTimer,
            Some(&mut adapter),
            |_, _| {
                probe = Some(crate::allocation_probe::begin());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(probe.unwrap().finish(), 0);
    assert!(matches!(
        result.report().terminal,
        TerminalDisposition::Failed { .. }
    ));
    assert!(result.report().failure.as_ref().unwrap().capacity() <= 512);
    assert!(result.report().fore_pending_output.is_some());
    assert!(host.kernel_resources.is_idle());
    drop(result);
    assert!(observed.load(Ordering::Acquire) > 0);
    drop(adapter.bytes.take());
    assert_eq!(observed.load(Ordering::Acquire), 0);
}
