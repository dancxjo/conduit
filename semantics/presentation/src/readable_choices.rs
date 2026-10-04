//! A bounded Mask-neutral view of exact finite text choices.

use alloc::vec::Vec;
use conduit_core::{CheckedValueContract, ValueConstraint};

use crate::UTF8_TEXT_VALUE_KIND;

/// Borrow small control-free UTF-8 choices from a positive canonical text membership.
/// The contract remains authoritative; each Mask chooses its own wording.
pub fn readable_finite_text_choices(contract: &CheckedValueContract) -> Option<Vec<&str>> {
    if contract.value_kind.as_str() != UTF8_TEXT_VALUE_KIND {
        return None;
    }
    let [ValueConstraint::CanonicalMembership {
        members,
        negated: false,
    }] = contract.constraints.as_slice()
    else {
        return None;
    };
    if members.is_empty() || members.len() > 8 {
        return None;
    }
    let mut choices = Vec::with_capacity(members.len());
    let mut bytes = 0usize;
    for member in members {
        let value = core::str::from_utf8(member).ok()?;
        if value.is_empty() || value.chars().any(char::is_control) {
            return None;
        }
        bytes = bytes.checked_add(value.len())?;
        if bytes > 128 {
            return None;
        }
        choices.push(value);
    }
    Some(choices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::kind_id;

    fn contract(members: Vec<Vec<u8>>, negated: bool) -> CheckedValueContract {
        CheckedValueContract::new(
            kind_id(UTF8_TEXT_VALUE_KIND),
            256,
            alloc::vec![ValueConstraint::CanonicalMembership { members, negated }],
        )
        .unwrap()
    }

    #[test]
    fn finite_control_free_text_choices_are_borrowed_without_changing_contract() {
        let current = contract(
            ["1000", "2000", "250", "500"]
                .map(|value| value.as_bytes().to_vec())
                .into(),
            false,
        );
        assert_eq!(
            readable_finite_text_choices(&current),
            Some(alloc::vec!["1000", "2000", "250", "500"])
        );
        assert!(current.validate(b"500").is_ok());
        assert!(current.validate(b"750").is_err());
    }

    #[test]
    fn large_unsafe_and_negative_membership_keep_exact_generic_contract() {
        let many = contract(
            (0u8..9).map(|value| alloc::vec![b'0' + value]).collect(),
            false,
        );
        assert_eq!(readable_finite_text_choices(&many), None);
        assert_eq!(
            readable_finite_text_choices(&contract(alloc::vec![b"line\nbreak".to_vec()], false)),
            None
        );
        assert_eq!(
            readable_finite_text_choices(&contract(alloc::vec![b"one".to_vec()], true)),
            None
        );
        assert_eq!(
            readable_finite_text_choices(&contract(alloc::vec![alloc::vec![b'x'; 129]], false)),
            None
        );
    }
}
