//! Exact private envelope path; escaped ASCII keys cannot bypass the Plan bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Scope {
    Other,
    Root,
    Show,
    Mask,
    Plan,
}
impl Scope {
    pub(super) fn child(self, key: &[u8]) -> Self {
        match self {
            Self::Root if field(key, b"show") => Self::Show,
            Self::Show if field(key, b"planned_mask") => Self::Mask,
            Self::Mask if field(key, b"plan") => Self::Plan,
            _ => Self::Other,
        }
    }
}
fn field(key: &[u8], expected: &[u8]) -> bool {
    let Some(mut key) = key.strip_prefix(b"\"").and_then(|k| k.strip_suffix(b"\"")) else {
        return false;
    };
    for expected in expected {
        let Some((&byte, tail)) = key.split_first() else {
            return false;
        };
        if byte == b'\\' {
            let Some(hex) = tail.strip_prefix(b"u").and_then(|k| k.get(..4)) else {
                return false;
            };
            let Ok(hex) = core::str::from_utf8(hex) else {
                return false;
            };
            if u16::from_str_radix(hex, 16).ok() != Some(u16::from(*expected)) {
                return false;
            }
            key = &tail[5..];
        } else {
            if byte != *expected {
                return false;
            }
            key = tail;
        }
    }
    key.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escaped_ascii_names_match_exactly_without_allocating() {
        for k in [
            br#""plan""#.as_slice(),
            br#""pl\u0061n""#,
            br#""\u0070\u006Can""#,
        ] {
            assert_eq!(Scope::Mask.child(k), Scope::Plan);
        }
        for k in [br#""plans""#.as_slice(), br#""pl\u0161n""#, br#""pl\\an""#] {
            assert_eq!(Scope::Mask.child(k), Scope::Other);
        }
    }
}
