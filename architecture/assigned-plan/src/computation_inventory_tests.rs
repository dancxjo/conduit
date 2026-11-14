//! Full existing record-count domain with unchanged physical validation state.
use super::*;
fn profile(requirements: &[AssignedComputationRequirement]) -> AssignedComputationProfile<'_> {
    AssignedComputationProfile {
        maximum_text_bytes: 2,
        maximum_total_text_bytes: 510,
        requirements,
    }
}
fn resident(requirement: AssignedComputationRequirement) -> [u8; 55] {
    let mut bytes = [0; 55];
    bytes[..2].copy_from_slice(&requirement.node.to_le_bytes());
    bytes[2..18].copy_from_slice(&requirement.key.0);
    bytes[18] = 4;
    bytes[19..23].copy_from_slice(&requirement.text_bytes.to_le_bytes());
    bytes[23..].copy_from_slice(&requirement.text_digest);
    bytes
}
#[test]
fn full_configuration_domain_uses_original_storage_and_exact_inventory() {
    let requirements: [_; MAXIMUM_ASSIGNED_COMPUTATIONS] = core::array::from_fn(|i| {
        AssignedComputationRequirement::from_text(
            i as u16,
            if i % 2 == 0 { "program" } else { "selector" },
            "ab",
        )
        .unwrap()
    });
    #[allow(dead_code)]
    struct Previous<'a> {
        profile: Option<AssignedComputationProfile<'a>>,
        seen: [bool; 32],
        total: u32,
    }
    assert_eq!(SEEN_BYTES, 32);
    assert_eq!(
        core::mem::size_of::<Validation<'_>>(),
        core::mem::size_of::<Previous<'_>>()
    );
    let mut validation = Validation::new(Some(profile(&requirements))).unwrap();
    for (i, requirement) in requirements.iter().copied().enumerate() {
        if i % 2 == 0 {
            assert_eq!(validation.resident(&resident(requirement)), Ok(true));
        } else {
            let mut bytes = [0; 21];
            bytes[..2].copy_from_slice(&requirement.node.to_le_bytes());
            bytes[2..18].copy_from_slice(&requirement.key.0);
            bytes[18] = 3;
            bytes[19..].copy_from_slice(b"ab");
            assert_eq!(validation.text(&bytes), Ok(true));
        }
    }
    assert_eq!(validation.finish(), Ok(()));
    let mut incomplete = Validation::new(Some(profile(&requirements))).unwrap();
    for requirement in requirements[..254].iter().copied() {
        assert_eq!(incomplete.resident(&resident(requirement)), Ok(true));
    }
    assert_eq!(
        incomplete.finish(),
        Err(AssignedPlanRefusal::MissingComputation)
    );
}
#[test]
fn excess_duplicate_changed_and_unfunded_computations_still_refuse() {
    let requirements: [_; 256] = core::array::from_fn(|i| {
        AssignedComputationRequirement::from_text(i as u16, "program", "ab").unwrap()
    });
    assert!(matches!(
        Validation::new(Some(profile(&requirements))),
        Err(AssignedPlanRefusal::ComputationCapacityExceeded)
    ));
    let duplicate = [requirements[0], requirements[0]];
    assert!(matches!(
        Validation::new(Some(profile(&duplicate))),
        Err(AssignedPlanRefusal::MalformedRecord)
    ));
    let requirements = &requirements[..255];
    let mut validation = Validation::new(Some(profile(requirements))).unwrap();
    let first = resident(requirements[0]);
    assert_eq!(validation.resident(&first), Ok(true));
    assert_eq!(
        validation.resident(&first),
        Err(AssignedPlanRefusal::UnknownComputation)
    );
    for offset in [0, 2, 23, 54] {
        let mut changed = resident(requirements[254]);
        changed[offset] ^= 1;
        let mut validation = Validation::new(Some(profile(requirements))).unwrap();
        assert_eq!(
            validation.resident(&changed),
            Err(AssignedPlanRefusal::UnknownComputation)
        );
    }
    let mut small = profile(requirements);
    small.maximum_total_text_bytes = 1;
    let mut validation = Validation::new(Some(small)).unwrap();
    assert_eq!(
        validation.resident(&first),
        Err(AssignedPlanRefusal::ComputationCapacityExceeded)
    );
}
