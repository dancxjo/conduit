//! Scanning against an already resolved family, without domain admission.
use super::*;
use core::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedLiteralScanRefusal {
    InvalidFamily,
    UnboundPrefix,
    UndeclaredDelimiter,
    Unterminated,
    PayloadLimit,
    InvalidPatternOrFlags,
}

/// Borrowed authored bytes and the exact declared branch. The payload remains
/// an unchecked candidate until its ordinary owner constructor admits it.
#[derive(Debug)]
pub struct ScannedTypedLiteral<'a> {
    pub branch: &'a TypedLiteralBranch,
    pub authored: &'a str,
    pub raw_payload: &'a str,
    pub payload: &'a str,
    /// Byte offsets relative to the supplied source entrance.
    pub payload_bytes: Range<usize>,
    pub consumed_bytes: usize,
    pub case_insensitive: bool,
    pub anchored_start: bool,
    pub anchored_end: bool,
}

impl TypedLiteralFamily {
    /// The caller must resolve `alias` in lexical scope first. This method
    /// cannot import a family, choose by expected Type, or reserve a prefix.
    /// Only reviewed declared pairs are scanned; no Unicode normalization or
    /// escape decoding occurs here. Remaining input belongs to outer grammar.
    pub fn scan_literal<'a>(
        &'a self,
        alias: &str,
        source: &'a str,
    ) -> Result<ScannedTypedLiteral<'a>, TypedLiteralScanRefusal> {
        use TypedLiteralScanRefusal as R;
        self.validate_metadata().map_err(|_| R::InvalidFamily)?;
        if alias.len() > MAXIMUM_IDENTITY_BYTES || !crate::surface_lex::is_name(alias) {
            return Err(R::UnboundPrefix);
        }
        let body = source.strip_prefix(alias).ok_or(R::UnboundPrefix)?;
        let branch = self
            .branches
            .iter()
            .find(|branch| body.starts_with(branch.delimiter.pair().0))
            .ok_or(R::UndeclaredDelimiter)?;
        let (opener, closer) = branch.delimiter.pair();
        let maximum_close = opener.len_utf8() + branch.maximum_payload_bytes;
        // Bound scanner work before delegating to the existing pattern policy.
        // Two extra bytes suffice to recognize the only accepted flag, `i`,
        // and a second flag that makes it invalid.
        let mut end = body.len().min(maximum_close + closer.len_utf8() + 2);
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        let bounded = &body[..end];
        let (raw_payload, payload, consumed, insensitive, start, finish) =
            match branch.lexical_policy {
                TypedLiteralLexicalPolicy::RawUnicode => {
                    let mut escaped = false;
                    let mut close = None;
                    for (offset, character) in bounded.char_indices().skip(1) {
                        if offset > maximum_close {
                            break;
                        }
                        if escaped {
                            escaped = false;
                        } else if character == '\\' {
                            escaped = true;
                        } else if character == closer {
                            close = Some(offset);
                            break;
                        }
                    }
                    let close = close.ok_or(if body.len() > maximum_close {
                        R::PayloadLimit
                    } else {
                        R::Unterminated
                    })?;
                    let payload = &body[opener.len_utf8()..close];
                    (
                        payload,
                        payload,
                        close + closer.len_utf8(),
                        false,
                        false,
                        false,
                    )
                }
                TypedLiteralLexicalPolicy::PortablePattern => {
                    let (payload, insensitive, start, finish, consumed) =
                        crate::surface_parser::front::pattern::delimited_pattern(
                            bounded, opener, closer,
                        )
                        .ok_or(if end < body.len() {
                            R::PayloadLimit
                        } else {
                            R::InvalidPatternOrFlags
                        })?;
                    let flags = usize::from(insensitive);
                    let close = consumed - flags - closer.len_utf8();
                    let raw = &body[opener.len_utf8()..close];
                    (raw, payload, consumed, insensitive, start, finish)
                }
            };
        if raw_payload.len() > branch.maximum_payload_bytes {
            return Err(R::PayloadLimit);
        }
        if body[consumed..]
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic())
        {
            return Err(R::InvalidPatternOrFlags);
        }
        let payload_start = alias.len() + opener.len_utf8();
        let consumed_bytes = alias.len() + consumed;
        Ok(ScannedTypedLiteral {
            branch,
            authored: &source[..consumed_bytes],
            raw_payload,
            payload,
            payload_bytes: payload_start..payload_start + raw_payload.len(),
            consumed_bytes,
            case_insensitive: insensitive,
            anchored_start: start,
            anchored_end: finish,
        })
    }
}

#[cfg(test)]
mod tests;
