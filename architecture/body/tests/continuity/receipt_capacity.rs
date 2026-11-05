//! Reproduce the observed hot-history pressure without fabricating receipt rows.
//! This proves atomic refusal; archive/retirement acceptance belongs to #6764.
use super::*;
use conduit_body::{MAX_ADMISSION_RECEIPTS, MAX_MEMBERSHIP_EVENTS};

#[test]
fn repeated_signed_returns_fill_receipts_without_consuming_new_parts() {
    let (mut manager, mut membership, key) = admitted();
    let part = membership.parts[0].part_id.clone();
    let first = manager.receipts[0].clone();
    let mut archived_events = Vec::new();
    for index in 1..=MAX_ADMISSION_RECEIPTS {
        if membership.events.len() + 2 > MAX_MEMBERSHIP_EVENTS {
            // Preserve the exact returned prefix; no event grant is enlarged.
            archived_events.push(membership.checkpoint_events().unwrap());
        }
        let current = membership.parts[0]
            .current
            .as_ref()
            .unwrap()
            .boot_id
            .clone();
        membership
            .observe_offline(
                &membership.body_id.clone(),
                membership.revision,
                &part,
                &current,
                format!("sign/receipt-pressure/offline/{index}").into(),
            )
            .unwrap();
        let now = 10_000 + index as u64 * 1000;
        let host = advertisement(&format!("boot/receipt-pressure/{index}"), index as u64 + 1);
        let challenge = manager
            .begin_return(&membership, &part, &host, [index as u8; 32], now, now + 500)
            .unwrap();
        let proof = return_proof(&challenge, &key);
        let before_manager = manager.clone();
        let before_membership = membership.clone();
        let result = manager.complete_return(
            &mut membership,
            &host,
            &proof,
            now + 1,
            format!("sign/receipt-pressure/returned/{index}").into(),
        );
        if index == MAX_ADMISSION_RECEIPTS {
            assert_eq!(result, Err(AdmissionRefusal::ReceiptCapacityExhausted));
            assert_eq!(manager, before_manager);
            assert_eq!(membership, before_membership);
        } else {
            let credential = result.unwrap();
            assert_eq!(credential.part_id, part);
            assert_eq!(credential.host_id, host.host_id);
            assert_eq!(credential.boot_id, host.boot_id);
        }
        assert_eq!(membership.parts.len(), 1);
        assert_eq!(manager.storage_usage().continuity_keys.len, 1);
        assert_eq!(manager.receipts[0], first);
    }
    assert!(!archived_events.is_empty());
    assert_eq!(manager.receipts.len(), MAX_ADMISSION_RECEIPTS);
}
