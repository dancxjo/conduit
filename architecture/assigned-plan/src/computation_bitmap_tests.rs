//! Exact inventory matching over the format's full finite configuration domain.
use super::*;
fn profile(requirements: &[AssignedComputationRequirement]) -> AssignedComputationProfile<'_> {
    AssignedComputationProfile {
        maximum_text_bytes: 2,
        maximum_total_text_bytes: 510,
        requirements,
    }
}
fn text(requirement: AssignedComputationRequirement) -> [u8; 21] {
    let mut bytes = [0; 21];
    bytes[..2].copy_from_slice(&requirement.node.to_le_bytes());
    bytes[2..18].copy_from_slice(&requirement.key.0);
    bytes[18] = 3;
    bytes[19..].copy_from_slice(b"ab");
    bytes
}
#[test]
fn full_inventory_preserves_original_storage_and_requires_every_exact_record() {
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
    let mut validator = Validation::new(Some(profile(&requirements))).unwrap();
    // Reverse arrival also crosses every bit/byte boundary.
    for requirement in requirements.iter().rev().copied() {
        assert_eq!(validator.text(&text(requirement)), Ok(true));
    }
    assert_eq!(validator.finish(), Ok(()));
    let mut incomplete = Validation::new(Some(profile(&requirements))).unwrap();
    for requirement in requirements[..254].iter().copied() {
        assert_eq!(incomplete.text(&text(requirement)), Ok(true));
    }
    assert_eq!(
        incomplete.finish(),
        Err(AssignedPlanRefusal::MissingComputation)
    );
}
#[test]
fn excess_duplicate_changed_and_unfunded_records_still_refuse() {
    let requirements: [_; 256] = core::array::from_fn(|i| {
        AssignedComputationRequirement::from_text(i as u16, "program", "ab").unwrap()
    });
    assert!(matches!(
        Validation::new(Some(profile(&requirements))),
        Err(AssignedPlanRefusal::ComputationCapacityExceeded)
    ));
    assert!(matches!(
        Validation::new(Some(profile(&[requirements[0], requirements[0]]))),
        Err(AssignedPlanRefusal::MalformedRecord)
    ));
    let requirements = &requirements[..255];
    let mut validator = Validation::new(Some(profile(requirements))).unwrap();
    let first = text(requirements[0]);
    assert_eq!(validator.text(&first), Ok(true));
    assert_eq!(
        validator.text(&first),
        Err(AssignedPlanRefusal::UnknownComputation)
    );
    for offset in [0, 20] {
        let mut changed = text(requirements[254]);
        changed[offset] ^= 1;
        let mut validator = Validation::new(Some(profile(requirements))).unwrap();
        assert_eq!(
            validator.text(&changed),
            Err(AssignedPlanRefusal::UnknownComputation)
        );
    }
    let mut changed_key = text(requirements[254]);
    changed_key[2..18].copy_from_slice(&AssignedIdentity::from_text("selector").0);
    let mut validator = Validation::new(Some(profile(requirements))).unwrap();
    assert_eq!(
        validator.text(&changed_key),
        Err(AssignedPlanRefusal::UnknownComputation)
    );
    let mut underfunded = profile(requirements);
    underfunded.maximum_total_text_bytes = 1;
    let mut validator = Validation::new(Some(underfunded)).unwrap();
    assert_eq!(
        validator.text(&first),
        Err(AssignedPlanRefusal::ComputationCapacityExceeded)
    );
}
