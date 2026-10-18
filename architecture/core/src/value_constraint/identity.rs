//! Versioned semantic identity encoding for checked value contracts.
use super::*;

impl CheckedValueContract {
    /// Canonical, versioned bytes used wherever this checked contract enters a
    /// larger semantic identity. Authored spelling and Host representation are
    /// deliberately absent.
    pub fn identity_bytes(&self) -> Vec<u8> {
        let mut canonical = b"conduit.value-contract@3\0".to_vec();
        push_bytes(&mut canonical, self.value_kind.as_str().as_bytes());
        push_u32(&mut canonical, self.maximum_bytes);
        push_u32(&mut canonical, self.constraints.len() as u32);
        for constraint in &self.constraints {
            constraint.push_identity(&mut canonical);
        }
        canonical
    }
}

impl ValueConstraint {
    fn push_identity(&self, canonical: &mut Vec<u8>) {
        match self {
            Self::ByteLength { minimum, maximum } => {
                canonical.push(0);
                push_u32(canonical, *minimum);
                push_u32(canonical, *maximum);
            }
            Self::UnsignedRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                canonical.push(1);
                push_optional(canonical, minimum.as_ref(), |out, value| {
                    out.extend_from_slice(&value.to_le_bytes());
                });
                push_optional(canonical, maximum.as_ref(), |out, value| {
                    out.extend_from_slice(&value.to_le_bytes());
                });
                canonical.push(*minimum_endpoint as u8);
                canonical.push(*maximum_endpoint as u8);
            }
            Self::SignedRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                canonical.push(2);
                push_optional(canonical, minimum.as_ref(), |out, value| {
                    out.extend_from_slice(&value.to_le_bytes());
                });
                push_optional(canonical, maximum.as_ref(), |out, value| {
                    out.extend_from_slice(&value.to_le_bytes());
                });
                canonical.push(*minimum_endpoint as u8);
                canonical.push(*maximum_endpoint as u8);
            }
            Self::FixedIntegerRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                canonical.push(6);
                push_optional(canonical, minimum.as_ref(), |out, value| {
                    push_bytes(out, value)
                });
                push_optional(canonical, maximum.as_ref(), |out, value| {
                    push_bytes(out, value)
                });
                canonical.push(*minimum_endpoint as u8);
                canonical.push(*maximum_endpoint as u8);
            }
            Self::QuantityRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                canonical.push(3);
                push_optional(canonical, minimum.as_ref(), |out, value| {
                    out.extend_from_slice(&value.encode());
                });
                push_optional(canonical, maximum.as_ref(), |out, value| {
                    out.extend_from_slice(&value.encode());
                });
                canonical.push(*minimum_endpoint as u8);
                canonical.push(*maximum_endpoint as u8);
            }
            Self::FloatFinite => canonical.push(7),
            Self::FloatRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } => {
                canonical.push(8);
                push_optional(canonical, minimum.as_ref(), |out, value| {
                    push_bytes(out, value)
                });
                push_optional(canonical, maximum.as_ref(), |out, value| {
                    push_bytes(out, value)
                });
                canonical.push(*minimum_endpoint as u8);
                canonical.push(*maximum_endpoint as u8);
            }
            Self::CanonicalMembership { members, negated } => {
                canonical.push(4);
                canonical.push(u8::from(*negated));
                push_u32(canonical, members.len() as u32);
                for member in members {
                    push_bytes(canonical, member);
                }
            }
            Self::TextPattern {
                pattern,
                anchored_start,
                anchored_end,
                negated,
            } => {
                canonical.push(5);
                canonical.push(u8::from(*anchored_start));
                canonical.push(u8::from(*anchored_end));
                canonical.push(u8::from(*negated));
                push_u32(canonical, u32::from(pattern.start_state));
                push_u32(canonical, pattern.maximum_input_characters);
                push_u32(canonical, pattern.maximum_match_steps);
                push_u32(canonical, pattern.states.len() as u32);
                for state in &pattern.states {
                    canonical.push(u8::from(state.accepting));
                    push_u32(canonical, state.transitions.len() as u32);
                    for transition in &state.transitions {
                        push_u32(canonical, transition.first_scalar);
                        push_u32(canonical, transition.last_scalar);
                        push_u32(canonical, u32::from(transition.target_state));
                    }
                }
            }
        }
    }
}

fn push_u32(canonical: &mut Vec<u8>, value: u32) {
    canonical.extend_from_slice(&value.to_le_bytes());
}

fn push_bytes(canonical: &mut Vec<u8>, value: &[u8]) {
    push_u32(canonical, value.len() as u32);
    canonical.extend_from_slice(value);
}

fn push_optional<T>(
    canonical: &mut Vec<u8>,
    value: Option<&T>,
    push: impl FnOnce(&mut Vec<u8>, &T),
) {
    canonical.push(u8::from(value.is_some()));
    if let Some(value) = value {
        push(canonical, value);
    }
}
