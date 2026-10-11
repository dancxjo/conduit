//! Checked semantic correlation, not physical settlement evidence.
use super::*;
use crate::storage_base::result::PreparedBlockReadResultEncoder;

#[test]
fn two_placements_have_distinct_exact_contracts_without_broadening_block_result() {
    let contract = BlockReadTransactionContract::prepare().unwrap();
    assert_ne!(
        contract.authorize_kind().kind_id,
        contract.settle_kind().kind_id
    );
    assert_eq!(
        contract.authorize_kind().inputs[0],
        contract.block().kind().inputs[0]
    );
    assert_eq!(
        contract.settle_kind().outputs[0],
        contract.block().kind().outputs[0]
    );
    assert_ne!(
        contract.authorize_kind().outputs[0].value_kind,
        contract.block().kind().outputs[0].value_kind
    );
}

#[test]
fn settlement_candidate_requires_full_original_request_and_ordinal() {
    let contract = BlockReadTransactionContract::prepare().unwrap();
    let mut encoder = PreparedBlockReadResultEncoder::new(contract.block()).unwrap();
    let request = BlockReadRequest::checked(512, 1, 9).unwrap();
    let bytes = encoder.observed(request, 4, &[0x80; 512], 0).unwrap();
    validate_candidate(bytes, request, 4).unwrap();
    assert_eq!(
        validate_candidate(bytes, request, 3),
        Err(StorageTransactionRefusal::Candidate)
    );
    assert_eq!(
        validate_candidate(bytes, BlockReadRequest::checked(512, 1, 10).unwrap(), 4),
        Err(StorageTransactionRefusal::Candidate)
    );
}

#[test]
fn partial_extent_stays_incomplete_and_cannot_become_a_completed_block() {
    let contract = BlockReadTransactionContract::prepare().unwrap();
    let mut encoder = PreparedBlockReadResultEncoder::new(contract.block()).unwrap();
    let request = BlockReadRequest::checked(512, 1, 0).unwrap();
    let bytes = encoder.observed(request, 0, &[1; 17], 495).unwrap();
    validate_candidate(bytes, request, 0).unwrap();
    assert_eq!(
        validate_canonical_structured_value(bytes)
            .unwrap()
            .variant_tag()
            .unwrap(),
        "incomplete"
    );
    assert!(encoder.observed(request, 0, &[1; 17], 0).is_err());
}

#[path = "fixture.rs"]
mod fixture;
#[test]
fn selected_dual_scopes_reject_exact_identity_substitution_before_authorization() {
    for field in 0..8 {
        let result = fixture::owner(|_, settle| match field {
            0 => settle.claim.boot_id = "foreign/boot".into(),
            1 => settle.claim.plan_id = "foreign/plan".into(),
            2 => settle.claim.active_play_id = "foreign/play".into(),
            3 => settle.claim.base_provider_generation += 1,
            4 => {
                settle.claim.resource_generation_id =
                    ResourceGenerationId("foreign/resource".into())
            }
            5 => settle.claim.envelope_id = "foreign/envelope".into(),
            6 => settle.claim.subject_kind = AUTHORIZE_KIND.into(),
            _ => settle.claim.operation_contract_id = AUTHORIZE_CALL.into(),
        });
        assert!(result.is_err());
    }
}
#[test]
fn short_contradictory_candidate_retains_storage_possession_and_original_receipt() {
    let mut owner = fixture::owner(|_, _| {}).unwrap();
    let node = owner.authorize_node();
    let (submission, _) = owner
        .authorize(node, HostCallId(0), RequestId(0), &fixture::input(0))
        .unwrap();
    assert!(matches!(
        owner.authorize(node, HostCallId(0), RequestId(1), &fixture::input(0)),
        Err(StorageTransactionRefusal::Pressure)
    ));
    let contract = BlockReadTransactionContract::prepare().unwrap();
    let mut encoder = PreparedBlockReadResultEncoder::new(contract.block()).unwrap();
    // Extent itself is valid, but its wrong ordinal cannot settle this lease.
    let candidate = encoder
        .observed(submission.request(), 1, &[0x80; 17], 495)
        .unwrap();
    let receipt = unsafe { StorageSettlementReceipt::acknowledged(submission) };
    let Err((error, receipt)) = owner.settle(
        owner.settle_node(),
        HostCallId(0),
        RequestId(0),
        candidate,
        receipt,
    ) else {
        panic!("wrong original ordinal accepted");
    };
    assert_eq!(error, StorageTransactionRefusal::Candidate);
    assert!(owner.pending.is_some());
    assert_eq!(
        owner
            .authorize
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        1
    );
    let candidate = encoder
        .observed(receipt.0.request(), 0, &[0x80; 17], 495)
        .unwrap();
    assert!(
        owner
            .settle(
                owner.settle_node(),
                HostCallId(0),
                RequestId(0),
                candidate,
                receipt
            )
            .is_ok()
    );
    assert!(owner.pending.is_none());
}
#[test]
fn cancellation_retains_physical_submission_until_exact_stop_receipt() {
    let mut owner = fixture::owner(|_, _| {}).unwrap();
    let (submission, _) = owner
        .authorize(
            owner.authorize_node(),
            HostCallId(0),
            RequestId(0),
            &fixture::input(0),
        )
        .unwrap();
    owner.request_cancel().unwrap();
    assert!(owner.pending.is_some());
    let mut foreign = StorageCancellationReceipt(StorageAuthorizationSubmission {
        lease: submission.lease.clone(),
        request: submission.request,
        ordinal: submission.ordinal,
        preparation: [0; 32],
    });
    foreign = match owner.retire_cancelled(foreign) {
        Err((StorageTransactionRefusal::Stale, receipt)) => receipt,
        _ => panic!("forged preparation released lease"),
    };
    assert!(owner.pending.is_some());
    drop(foreign);
    let receipt = unsafe { StorageCancellationReceipt::stopped(submission) };
    assert!(owner.retire_cancelled(receipt).is_ok());
    assert!(owner.pending.is_none());
    assert!(matches!(
        owner.authorize(
            owner.authorize_node(),
            HostCallId(0),
            RequestId(1),
            &fixture::input(0)
        ),
        Err(StorageTransactionRefusal::Terminal)
    ));
}
