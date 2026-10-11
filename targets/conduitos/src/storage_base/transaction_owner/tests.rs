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
