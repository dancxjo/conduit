//! A bounded Mask-neutral view of exact finite text choices.

use alloc::vec::Vec;
use conduit_core::{CheckedValueContract, ValueConstraint};

use crate::UTF8_TEXT_VALUE_KIND;

/// Borrow small, conservatively legible UTF-8 choices from positive membership.
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
        if value.is_empty() || !value.chars().all(plain_choice_scalar) {
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

// Do not inline format, bidi, zero-width, or combining characters as though
// they were plainly visible options. A broader contract keeps its generic Face
// clause; the checked membership itself is never changed by this display hint.
fn plain_choice_scalar(ch: char) -> bool {
    !matches!(ch, '\u{115f}' | '\u{1160}' | '\u{3164}' | '\u{ffa0}')
        && (ch == ' ' || ch.is_ascii_graphic() || ch.is_alphanumeric())
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

    #[test]
    fn formatting_and_invisible_scalars_keep_the_generic_clause() {
        for value in [
            "left\u{202e}right",
            "a\u{200d}b",
            "a\u{0301}",
            "a\u{00ad}b",
            "a\u{3164}b",
        ] {
            assert_eq!(
                readable_finite_text_choices(&contract(
                    alloc::vec![value.as_bytes().to_vec()],
                    false
                )),
                None,
                "{value:?}"
            );
        }
        let legible = contract(
            ["Café", "١٠٠٠", "東京"]
                .map(|value| value.as_bytes().to_vec())
                .into(),
            false,
        );
        assert_eq!(
            readable_finite_text_choices(&legible),
            Some(alloc::vec!["Café", "١٠٠٠", "東京"])
        );
    }
}
