use super::*;
use crate::state_delay::StateError;

#[test]
fn a_larger_profile_continues_committed_state_and_generation() {
    let mut source = StateDelay::<1>::externally_continued(3, 1, &[0]).unwrap();
    source.offer_next(&[7]).unwrap();
    source.commit().unwrap();
    let (mut destination, evidence) = match source.try_transfer::<4>(9, 4) {
        Ok(transferred) => transferred,
        Err(_) => panic!("admitted transfer refused"),
    };
    assert_eq!(destination.current(), &[7]);
    assert_eq!(destination.generation(), 1);
    assert_eq!(
        evidence,
        StateTransferEvidence {
            source_slot: 3,
            destination_slot: 9,
            generation: 1,
            value_bytes: 1,
            source_capacity: 1,
            destination_capacity: 4,
        }
    );
    destination.offer_next(&[7, 8]).unwrap();
    let committed = destination.commit().unwrap();
    assert_eq!(committed.state, 9);
    assert_eq!(destination.current(), &[7, 8]);
    assert_eq!(destination.generation(), 2);
}

#[test]
fn insufficient_capacity_returns_the_unchanged_source() {
    let source = StateDelay::<4>::externally_continued(3, 4, &[1, 2]).unwrap();
    let refused = match source.try_transfer::<1>(9, 1) {
        Err(refused) => refused,
        Ok(_) => panic!("oversized State transferred"),
    };
    assert_eq!(
        refused.reason,
        StateTransferError::InsufficientDestinationCapacity
    );
    assert_eq!(refused.source.current(), &[1, 2]);
    assert_eq!(refused.source.generation(), 0);
}

#[test]
fn pending_candidate_is_not_silently_dropped() {
    let mut source = StateDelay::<1>::externally_continued(3, 1, &[1]).unwrap();
    source.offer_next(&[2]).unwrap();
    let mut refused = match source.try_transfer::<4>(9, 4) {
        Err(refused) => refused,
        Ok(_) => panic!("pending candidate silently transferred"),
    };
    assert_eq!(refused.reason, StateTransferError::CandidatePending);
    assert_eq!(refused.source.current(), &[1]);
    refused.source.commit().unwrap();
    assert_eq!(refused.source.current(), &[2]);
}

#[test]
fn transfer_does_not_renew_transition_or_identity_capacity() {
    let mut source = StateDelay::<1>::new(3, 1, 1, &[0]).unwrap();
    source.offer_next(&[1]).unwrap();
    source.commit().unwrap();
    let (mut destination, _) = match source.try_transfer::<2>(9, 2) {
        Ok(transferred) => transferred,
        Err(_) => panic!("transfer refused"),
    };
    destination.offer_next(&[2]).unwrap();
    assert_eq!(
        destination.commit(),
        Err(StateError::TransitionLimitReached)
    );
    assert_eq!(destination.current(), &[1]);
    let mut source = StateDelay::<1>::externally_continued(3, 1, &[8]).unwrap();
    source.generation = u64::MAX;
    let (mut destination, _) = match source.try_transfer::<2>(9, 2) {
        Ok(transferred) => transferred,
        Err(_) => panic!("transfer refused"),
    };
    assert_eq!(
        destination.offer_next(&[9]),
        Err(StateError::IdentityCapacityExhausted)
    );
    assert_eq!(destination.current(), &[8]);
}

#[test]
fn destination_bounds_must_fit_the_concrete_profile() {
    for capacity in [0, 3] {
        let source = StateDelay::<1>::externally_continued(3, 1, &[0]).unwrap();
        match source.try_transfer::<2>(9, capacity) {
            Err(refused) => {
                assert_eq!(refused.reason, StateTransferError::InvalidDestinationBounds);
                assert_eq!(refused.source.current(), &[0]);
            }
            Ok(_) => panic!("invalid profile admitted"),
        }
    }
}
