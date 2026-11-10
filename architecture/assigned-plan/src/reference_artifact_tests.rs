use super::*;
use std::vec::Vec;
fn payload(node: u16, key: &str, text: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&node.to_le_bytes());
    bytes.extend_from_slice(&AssignedIdentity::from_text(key).0);
    bytes.push(3);
    bytes.extend_from_slice(text.as_bytes());
    bytes
}
#[test]
fn reference_text_requires_exact_trusted_inventory_and_finite_funding() {
    let text = "ab".repeat(122);
    let pin = AssignedComputationRequirement::from_text(1, "reference", &text).unwrap();
    let pins = [pin];
    let profile = AssignedComputationProfile {
        maximum_text_bytes: 244,
        maximum_total_text_bytes: 244,
        requirements: &pins,
    };
    let bytes = payload(1, "reference", &text);
    assert_eq!(
        Validation::new(None).unwrap().text(&bytes),
        Err(AssignedPlanRefusal::UnknownComputation)
    );
    let mut valid = Validation::new(Some(profile)).unwrap();
    assert_eq!(valid.text(&bytes), Ok(true));
    assert!(valid.finish().is_ok());
    let mut duplicate = Validation::new(Some(profile)).unwrap();
    duplicate.text(&bytes).unwrap();
    assert!(duplicate.text(&bytes).is_err());
    assert_eq!(
        Validation::new(Some(profile)).unwrap().finish(),
        Err(AssignedPlanRefusal::MissingComputation)
    );
    assert_eq!(
        Validation::new(Some(profile))
            .unwrap()
            .text(&payload(1, "reference", &"cd".repeat(122))),
        Err(AssignedPlanRefusal::UnknownComputation)
    );
    assert_eq!(
        Validation::new(Some(profile))
            .unwrap()
            .text(&payload(2, "reference", &text)),
        Err(AssignedPlanRefusal::UnknownComputation)
    );
    for narrowed in [
        AssignedComputationProfile {
            maximum_text_bytes: 243,
            ..profile
        },
        AssignedComputationProfile {
            maximum_total_text_bytes: 243,
            ..profile
        },
    ] {
        assert_eq!(
            Validation::new(Some(narrowed)).unwrap().text(&bytes),
            Err(AssignedPlanRefusal::ComputationCapacityExceeded)
        );
    }
}
#[test]
fn reference_ceiling_does_not_enlarge_ordinary_text_or_skip_aggregate_accounting() {
    let text = "ab".repeat(122);
    let pins = [
        AssignedComputationRequirement::from_text(1, "reference", &text).unwrap(),
        AssignedComputationRequirement::from_text(2, "reference", &text).unwrap(),
    ];
    let profile = AssignedComputationProfile {
        maximum_text_bytes: 1024,
        maximum_total_text_bytes: 487,
        requirements: &pins,
    };
    let mut checked = Validation::new(Some(profile)).unwrap();
    assert!(checked.text(&payload(1, "reference", &text)).unwrap());
    assert_eq!(
        checked.text(&payload(2, "reference", &text)),
        Err(AssignedPlanRefusal::ComputationCapacityExceeded)
    );
    assert_eq!(
        Validation::new(Some(profile))
            .unwrap()
            .text(&payload(1, "arbitrary", &"x".repeat(129))),
        Ok(false)
    );
    assert_eq!(
        Validation::new(None)
            .unwrap()
            .text(&payload(1, "arbitrary", &"x".repeat(128))),
        Ok(true)
    );
    assert!(AssignedComputationRequirement::from_text(1, "reference", &"ab".repeat(513)).is_err());
    assert!(AssignedComputationRequirement::from_text(1, "reference", "AB").is_err());
}
