//! Exact admitted semantic ownership; fixtures do not prove native transport.
use super::*;
mod fixture;
#[test]
fn forged_handle_and_wrong_authority_lifetimes_refuse_before_submission() {
    let input = fixture::input(0);
    for field in 0..7 {
        let mut owner = fixture::owner(7);
        match field {
            0 => owner.claim.boot_id = conduit_core::BootId::from("boot/foreign"),
            1 => owner.claim.plan_id = conduit_core::PlanId::from("plan/foreign"),
            2 => owner.claim.active_play_id = conduit_core::ActivePlayId::from("play/foreign"),
            3 => owner.claim.base_provider_generation += 1,
            4 => {
                owner.claim.resource_generation_id =
                    conduit_core::ResourceGenerationId("resource/foreign".into())
            }
            5 => {
                owner.claim.envelope_id =
                    conduit_core::CapabilityEnvelopeId::from("envelope/foreign")
            }
            _ => {
                let mut foreign = fixture::table(8);
                let mut request = fixture::request();
                request.scope.plan_id = owner.claim.plan_id.clone();
                request.scope.active_play_id = owner.claim.active_play_id.clone();
                owner.handle = foreign.issue(request).unwrap();
            }
        }
        assert!(matches!(
            owner.begin(owner.node, HostCallId(0), RequestId(0), &input),
            Err(BlockReadOwnerRefusal::Capability(_))
        ));
        assert!(owner.pending.is_none());
        assert_eq!(owner.next_request, 0);
    }
}
#[test]
fn one_pending_request_retains_exact_geometry_until_verified_completion() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let input = fixture::input(19);
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &input)
        .unwrap();
    assert_eq!(submission.request().lba, 19);
    assert_eq!(
        owner.begin(node, HostCallId(0), RequestId(1), &input).err(),
        Some(BlockReadOwnerRefusal::Pressure)
    );
    assert!(
        unsafe {
            owner.finish_acknowledged(
                &submission,
                BlockReadObservation::Read {
                    bytes: &[0; 17],
                    residue: 0,
                },
            )
        }
        .is_err()
    );
    assert!(owner.pending.is_some());
    let result = unsafe {
        owner.finish_acknowledged(
            &submission,
            BlockReadObservation::Read {
                bytes: &[0; 17],
                residue: 495,
            },
        )
    }
    .unwrap();
    assert_eq!(
        conduit_core::validate_canonical_structured_value(result)
            .unwrap()
            .variant_tag()
            .unwrap(),
        "incomplete"
    );
    assert_eq!(
        owner.begin(node, HostCallId(0), RequestId(0), &input).err(),
        Some(BlockReadOwnerRefusal::StaleRequest)
    );
    assert_eq!(
        unsafe { owner.discard_acknowledged(&submission) },
        Err(BlockReadOwnerRefusal::StaleRequest)
    );
}
#[test]
fn wrong_call_and_selected_output_envelope_refuse() {
    let mut owner = fixture::owner(7);
    let input = fixture::input(0);
    assert_eq!(
        owner
            .begin(owner.node, HostCallId(1), RequestId(0), &input)
            .err(),
        Some(BlockReadOwnerRefusal::WrongBinding)
    );
    let (fragment, mut lowered, active, placement) = fixture::selected();
    lowered.host_calls[0].binding.maximum_output_bytes += 1;
    assert_eq!(
        fixture::bind(
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement
            },
            7
        )
        .err(),
        Some(BlockReadOwnerRefusal::WrongBinding)
    );
}
#[test]
fn cancellation_keeps_pending_receipt_until_ack_and_late_output_never_publishes() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let input = fixture::input(0);
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &input)
        .unwrap();
    owner.revoke(node, HostCallId(0)).unwrap();
    assert!(owner.pending.is_some());
    assert!(matches!(
        owner.begin(node, HostCallId(0), RequestId(1), &input),
        Err(BlockReadOwnerRefusal::Capability(
            BaseCapabilityRefusal::Revoked
        ))
    ));
    // Unsafe acknowledgement models actual subordinate stop, not software revoke.
    assert!(
        unsafe {
            owner.finish_acknowledged(
                &submission,
                BlockReadObservation::Read {
                    bytes: &[0; 512],
                    residue: 0,
                },
            )
        }
        .is_err()
    );
    assert!(owner.pending.is_none());
    assert_eq!(
        unsafe { owner.discard_acknowledged(&submission) },
        Err(BlockReadOwnerRefusal::StaleRequest)
    );
}
#[test]
fn finite_reuse_does_not_allocate_during_play_and_budget_exhaustion_is_distinct() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let input = fixture::input(0);
    let allocations = crate::test_allocations::allocations(|| {
        for ordinal in 0..16 {
            let submission = owner
                .begin(node, HostCallId(0), RequestId(ordinal), &input)
                .unwrap();
            unsafe {
                owner.finish_acknowledged(
                    &submission,
                    BlockReadObservation::Read {
                        bytes: &[0xa5; 512],
                        residue: 0,
                    },
                )
            }
            .unwrap();
        }
    });
    assert_eq!(allocations, 0);
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        16
    );
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(16), &input)
            .err(),
        Some(BlockReadOwnerRefusal::Capability(
            BaseCapabilityRefusal::Exhausted
        ))
    );
}
#[test]
fn acknowledged_terminal_failure_forbids_another_effect() {
    for disposition in [
        BlockReadDisposition::Stalled,
        BlockReadDisposition::ProviderLost,
        BlockReadDisposition::Unsupported,
        BlockReadDisposition::Timeout,
        BlockReadDisposition::CommandFailed,
    ] {
        let mut owner = fixture::owner(7);
        let input = fixture::input(0);
        let submission = owner
            .begin(owner.node, HostCallId(0), RequestId(0), &input)
            .unwrap();
        unsafe {
            owner.finish_acknowledged(&submission, BlockReadObservation::Disposition(disposition))
        }
        .unwrap();
        assert_eq!(
            owner
                .begin(owner.node, HostCallId(0), RequestId(1), &input)
                .err(),
            Some(BlockReadOwnerRefusal::Terminal)
        );
    }
}
