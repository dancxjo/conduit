//! Physical newlines inside declared glyphs belong to their exact payload.
use super::Parser;
use crate::surface_lex::SourceLine;

impl<'a> Parser<'a> {
    pub(super) fn prepare_glyph_lines(&mut self, scope: &crate::GlyphNotationScope) {
        let Some(first) = self.lines.get(self.index) else {
            return;
        };
        let mut lines = self.lines[..self.index].to_vec();
        let mut start = first.start;
        let mut offset = start;
        let mut quote = None;
        let mut escaped = false;
        while offset < self.source.len() {
            if quote.is_none() {
                if let Some(literal) = scope.scan_at(self.source, offset) {
                    offset += literal.consumed_bytes;
                    continue;
                }
            }
            let character = self.source[offset..].chars().next().unwrap();
            if character == '\n' {
                lines.push(glyph_line(self.source, start, offset, scope));
                offset += 1;
                start = offset;
                quote = None;
                escaped = false;
                continue;
            }
            if let Some(active) = quote {
                if character == active && !escaped {
                    quote = None;
                }
                escaped = character == '\\' && !escaped;
                if character != '\\' {
                    escaped = false;
                }
            } else if character == '#' {
                offset = self.source[offset..]
                    .find('\n')
                    .map_or(self.source.len(), |end| offset + end);
                continue;
            } else if matches!(character, '\'' | '"') {
                quote = Some(character);
            }
            offset += character.len_utf8();
        }
        if start < self.source.len() {
            lines.push(glyph_line(self.source, start, self.source.len(), scope));
        }
        self.lines = lines;
    }
}

fn glyph_line<'a>(
    source: &'a str,
    start: usize,
    end: usize,
    scope: &crate::GlyphNotationScope,
) -> SourceLine<'a> {
    let text = &source[start..end];
    SourceLine {
        text,
        start,
        statement_end: crate::surface_lex::comment_start_with_scope(text, scope)
            .unwrap_or(text.len()),
    }
}
