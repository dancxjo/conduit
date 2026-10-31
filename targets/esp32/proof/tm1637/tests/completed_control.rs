//! CPU-only proof of actual NativePlay dispatch and retained native custody.
use super::*;
use crate::{
    allocation_probe,
    support::{fixture, immediate},
};
#[test]
fn actual_attached_completion_frees_only_kernel_and_keeps_selected_native_custody() {
    let (mut play, state, mut issuer) = fixture();
    let expected = play.active().unwrap().clone();
    let ((), memory) = allocation_probe::observe(|| immediate(play.run()));
    assert_eq!(memory.allocations + memory.reallocations, 0);
    assert_eq!(
        memory.deallocated_bytes,
        core::mem::size_of::<PreparedNetworkKernel>()
    );
    println!("completed_kernel_bytes={} completed_owner_inline_bytes={} completed_option_inline_bytes={} native_play_bytes={} sign_backing_bytes={}",core::mem::size_of::<PreparedNetworkKernel>(),core::mem::size_of::<CompletedControl>(),core::mem::size_of::<Option<CompletedControl>>(),core::mem::size_of::<NativePlay>(),n::CONTROL_SIGN_BACKING_BYTES);
    assert!(play.prepared.is_none() && play.completed.is_some());
    assert_eq!(play.status(), "attached");
    assert!(play.installed());
    assert_eq!(play.active(), Some(&expected));
    assert!(!play.permits_unselected_configuration());
    assert!(play.station_services_available());
    assert!(play.source.is_some() && play.join.is_some());
    assert!(play.join.as_ref().unwrap().cleanup_required());
    assert!(!play.source.as_ref().unwrap().retirement_ready());
    assert!(!play.join.as_ref().unwrap().retirement_ready());
    assert_eq!(state.calls.get(), [1, 1, 0]);
    let signs = play
        .completed
        .as_ref()
        .unwrap()
        .signs
        .events()
        .collect::<Vec<_>>();
    assert_eq!(
        play.completed.as_ref().unwrap().signs.allocation_capacity(),
        128
    );
    assert!(play.take_retired().is_err());
    immediate(play.run());
    assert_eq!(state.calls.get(), [1, 1, 0]);
    immediate(play.restore_for_retirement()).unwrap();
    let retired = play.take_retired().unwrap();
    assert_eq!(retired.evidence().events().collect::<Vec<_>>(), signs);
    assert!(retired
        .evidence()
        .events()
        .all(|event| !matches!(event.kind, k::KernelEventKind::RunCancelled)));
    assert!(!retired.evidence_is_committed());
    assert!(issuer.finish_network_play(&retired).is_err());
    assert_eq!(state.calls.get(), [1, 1, 1]);
    assert!(!state.link.get());
    let active = retired.active();
    let mut encoded = [0; n::recovery_evidence::MAXIMUM_BYTES];
    n::recovery_evidence::encode(
        n::recovery_evidence::Scope {
            host: active.host_id.as_str(),
            boot: active.boot_id.as_str(),
            plan: active.plan_id.as_str(),
            active_play: active.active_play_id.as_str(),
            provider_generation: retired.provider_generation,
            play_sequence: active.play_sequence,
            configuration_revision: retired.configuration_revision,
        },
        retired.evidence().events(),
        retired.evidence(),
        &mut encoded,
    )
    .unwrap();
    assert!(play.permits_unselected_configuration());
}
#[test]
fn live_kernel_refusal_retains_identical_box_and_original_signs() {
    let (mut play, state, _) = fixture();
    let pointer = &**play.prepared.as_ref().unwrap() as *const PreparedNetworkKernel;
    let signs = play
        .prepared
        .as_ref()
        .unwrap()
        .kernel
        .signs()
        .events()
        .collect::<Vec<_>>();
    // Descriptive successful phase/receipt cannot replace the actual gate.
    play.phase = "attached";
    play.receipt_observed = true;
    let (retired, memory) = allocation_probe::observe(|| play.retire_completed_kernel());
    assert!(!retired);
    assert!(play.completed.is_none());
    assert_eq!(&**play.prepared.as_ref().unwrap() as *const _, pointer);
    assert_eq!(
        memory.allocations + memory.reallocations + memory.deallocations,
        0
    );
    assert_eq!(
        play.prepared
            .as_ref()
            .unwrap()
            .kernel
            .signs()
            .events()
            .collect::<Vec<_>>(),
        signs
    );
    assert_eq!(state.calls.get(), [0, 0, 0]);
}
#[test]
fn completed_cleanup_failure_keeps_exact_signs_and_native_owners_for_retry() {
    let (mut play, state, _) = fixture();
    immediate(play.run());
    state.cleanup_failure.set(true);
    let signs = play
        .completed
        .as_ref()
        .unwrap()
        .signs
        .events()
        .collect::<Vec<_>>();
    assert!(immediate(play.restore_for_retirement()).is_err());
    assert_eq!(play.status(), "cleanup-failed");
    assert!(play.take_retired().is_err());
    assert!(play.source.is_some() && play.join.is_some() && play.active.is_some());
    assert_eq!(
        play.completed
            .as_ref()
            .unwrap()
            .signs
            .events()
            .collect::<Vec<_>>(),
        signs
    );
    state.cleanup_failure.set(false);
    immediate(play.restore_for_retirement()).unwrap();
    let mut retired = play.take_retired().unwrap();
    assert_eq!(retired.evidence().events().collect::<Vec<_>>(), signs);
    assert!(retired.take_owners().is_none());
    assert_eq!(state.profile_updates.get(), 0);
}
#[test]
fn failed_join_preserves_original_cancellation_and_full_retirement_path() {
    let (mut play, state, _) = fixture();
    state.join_failure.set(true);
    immediate(play.run());
    assert!(play.completed.is_none() && play.prepared.is_some());
    assert_eq!(play.status(), "access-point-restored");
    let retired = play.take_retired().unwrap();
    assert!(retired
        .evidence()
        .events()
        .any(|event| matches!(event.kind, k::KernelEventKind::RunCancelled)));
    assert_eq!(state.calls.get(), [1, 1, 1]);
}

#[test]
fn attachment_sign_refuses_different_bytes_than_the_actual_completed_join() {
    let (mut play, state, _) = fixture();
    // Drive the actual first two calls, but deliberately corrupt the Cord's
    // Join result after the selected owner has retained its original output.
    // This remains a fully bounded, structurally valid attachment value.
    for ordinal in 0..2 {
        let kernel = &mut play.prepared.as_mut().unwrap().kernel;
        let request = loop {
            if let Some(request) = kernel.next_host_request() {
                break request;
            }
            kernel.step().unwrap();
        };
        let input = kernel.host_value(request.input.value).unwrap();
        let mut buffer = [0; 167];
        let length = if ordinal == 0 {
            play.source
                .as_mut()
                .unwrap()
                .invoke(
                    request.node,
                    request.call,
                    request.request,
                    input,
                    &mut buffer,
                )
                .unwrap()
        } else {
            let bytes = immediate(play.join.as_mut().unwrap().invoke(
                request.node,
                request.call,
                request.request,
                input,
            ))
            .unwrap();
            let info = conduit_net::decode_network_attachment(bytes).unwrap();
            conduit_net::encode_network_attachment(
                conduit_net::NetworkAttachmentInfo {
                    attachment_id: "fixture/wrong-cord-value",
                    host_id: info.host_id,
                    boot_id: info.boot_id,
                    resource_pool_id: info.resource_pool_id,
                    generation: info.generation,
                },
                &mut buffer,
            )
            .unwrap()
        };
        let value = kernel.store_host_value(&buffer[..length]).unwrap();
        kernel
            .complete_host_call(
                request.node,
                request.request,
                k::HostCallOutcome {
                    disposition: k::HostCallDisposition::Completed,
                    output: Some(k::BoundedValueRef::new(value, 167).unwrap()),
                    failure: None,
                },
            )
            .unwrap();
    }
    assert!(play.join.as_ref().unwrap().completed_output().is_some());
    let (_, memory) = allocation_probe::observe(|| immediate(play.run()));
    assert_eq!(memory.allocations + memory.reallocations, 0);
    assert!(!play.receipt_received());
    assert!(play.completed.is_none());
    assert_eq!(play.failure.unwrap().code, k::FailureCode::HostCallDenied);
    assert_eq!(play.status(), "access-point-restored");
    assert_eq!(state.calls.get(), [1, 1, 1]);
    assert!(play.take_retired().is_ok());
}
