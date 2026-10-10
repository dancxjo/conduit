//! Fixed expression entrance for an explicitly imported literal family.
use super::*;
use crate::{TypedGlyphLiteralSyntax, TypedLiteralScanRefusal};
use sha2::{Digest, Sha256};

impl Parser<'_> {
    pub(super) fn notation(&mut self) -> Result<Option<ExpressionSyntax>, (String, Span)> {
        let Some(scope) = self.scope else {
            return Ok(None);
        };
        let start = self.offset;
        let tail = &self.text[start..];
        let length = tail
            .chars()
            .take_while(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '-'))
            .map(char::len_utf8)
            .sum::<usize>();
        let alias = &tail[..length];
        let Some(binding) = scope.binding(alias) else {
            return Ok(None);
        };
        let scanned = scope.scan_literal(alias, tail).map_err(|refusal| {
            let message = match refusal {
                TypedLiteralScanRefusal::UndeclaredDelimiter =>
                    "bound glyph notation requires an adjacent declared delimiter; use its qualified constructor for an ordinary expression",
                TypedLiteralScanRefusal::PayloadLimit =>
                    "typed glyph payload exceeds the declared finite byte bound",
                TypedLiteralScanRefusal::Unterminated =>
                    "unterminated typed glyph literal",
                TypedLiteralScanRefusal::InvalidPatternOrFlags =>
                    "invalid typed glyph delimiter, escape or flags; arithmetic ambiguity requires the qualified constructor",
                TypedLiteralScanRefusal::InvalidFamily | TypedLiteralScanRefusal::UnboundPrefix =>
                    "typed glyph family is not valid in this lexical scope",
            };
            (message.into(), self.from(start, start + length))
        })?;
        let identity = binding
            .family
            .identity_bytes()
            .map_err(|message| (message, self.from(start, start + length)))?;
        let source_document_id = self
            .source_document_id
            .get_or_insert_with(|| {
                conduit_core::SourceDocumentId::from(crate::hash_string(&format!(
                    "canonical-source:{}",
                    self.source
                )))
            })
            .clone();
        let literal = TypedGlyphLiteralSyntax {
            source_document_id,
            alias: SpannedText {
                text: alias.into(),
                span: self.from(start, start + length),
            },
            delimiter: scanned.branch.delimiter,
            family_identity: Sha256::digest(&identity).into(),
            authored: SpannedText {
                text: scanned.authored.into(),
                span: self.from(start, start + scanned.consumed_bytes),
            },
            raw_payload: SpannedText {
                text: scanned.raw_payload.into(),
                span: self.from(
                    start + scanned.payload_bytes.start,
                    start + scanned.payload_bytes.end,
                ),
            },
            payload: scanned.payload.into(),
            case_insensitive: scanned.case_insensitive,
            anchored_start: scanned.anchored_start,
            anchored_end: scanned.anchored_end,
        };
        self.offset += scanned.consumed_bytes;
        Ok(Some(ExpressionSyntax::TypedGlyphLiteral(Box::new(literal))))
    }
}

#[cfg(test)]
mod tests;
