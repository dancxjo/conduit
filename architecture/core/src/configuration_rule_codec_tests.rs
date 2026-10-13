//! Existing compact configuration rules keep their wire identities when names are added.
use crate::KindConfigurationRule;
use alloc::vec;

#[test]
fn construction_label_rule_does_not_reassign_existing_compact_rule_tags() {
    for (rule, bytes) in [
        (
            KindConfigurationRule::TextBytes { maximum: 32 },
            vec![5, 32],
        ),
        (
            KindConfigurationRule::TextOneOf { values: vec![] },
            vec![6, 0],
        ),
        (KindConfigurationRule::Quantity, vec![8]),
        (KindConfigurationRule::Unit, vec![9]),
        (KindConfigurationRule::TemperatureDifference, vec![10]),
        (
            KindConfigurationRule::DataLabel { maximum: 32 },
            vec![11, 32],
        ),
    ] {
        assert_eq!(postcard::to_allocvec(&rule).unwrap(), bytes);
        assert_eq!(
            postcard::from_bytes::<KindConfigurationRule>(&bytes).unwrap(),
            rule
        );
    }
}
