//! One fixed metadata declaration, not a user-defined parsing production.
use super::Parser;
use crate::{Expression, GlyphNotationSyntax, PlotError, Span};

const MAXIMUM_DECLARATION_BYTES: usize = 64 * 1024;

pub(super) fn parse(parser: &mut Parser<'_>) -> Result<GlyphNotationSyntax, (PlotError, Span)> {
    let opening = parser.lines[parser.index];
    let (header, start) = opening.statement();
    let declaration = header.strip_prefix("glyph notation ").unwrap();
    let (name, initial) = declaration.split_once('=').ok_or_else(|| {
        invalid(
            parser,
            "expected 'glyph notation NAME = { ... }'",
            start,
            header.len(),
        )
    })?;
    let name = name.trim();
    if !crate::surface_lex::is_name(name) || !initial.trim_start().starts_with('{') {
        return Err(invalid(
            parser,
            "glyph notation needs a name and a metadata record",
            start,
            header.len(),
        ));
    }
    let name_start = start + "glyph notation ".len() + declaration.find(name).unwrap();
    let value_start =
        start + header.find('=').unwrap() + 1 + initial.len() - initial.trim_start().len();
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    let mut end = None;
    // Scan only this finite declaration. Quoted braces stay payload bytes;
    // the ordinary structured parser validates record/collection syntax.
    for (offset, character) in parser.source[value_start..].char_indices() {
        if offset >= MAXIMUM_DECLARATION_BYTES {
            return Err(invalid(
                parser,
                "glyph notation metadata byte limit exceeded",
                value_start,
                offset,
            ));
        }
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
            continue;
        }
        match character {
            '"' => quoted = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(value_start + offset + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end.ok_or((
        PlotError::MissingBlockEnd,
        parser.span(value_start, parser.source.len()),
    ))?;
    let text = &parser.source[value_start..end];
    let syntax = crate::structured_expression::parse(parser.source, text, value_start)
        .map_err(|(message, span)| (PlotError::InvalidSyntax(message), span))?;
    while parser.index < parser.lines.len() && parser.lines[parser.index].start < end {
        let line = parser.lines[parser.index];
        let line_end = line.start + line.text.len();
        if end <= line_end
            && !parser.source[end..line_end].trim().is_empty()
            && !parser.source[end..line_end].trim_start().starts_with('#')
        {
            return Err(invalid(
                parser,
                "unexpected input after glyph notation metadata",
                end,
                line_end - end,
            ));
        }
        parser.index += 1;
    }
    Ok(GlyphNotationSyntax {
        name: parser.spanned(name, name_start),
        metadata: Expression {
            text: text.into(),
            syntax,
            span: parser.span(value_start, end),
        },
        span: parser.span(start, end),
    })
}

fn invalid(parser: &Parser<'_>, message: &str, start: usize, length: usize) -> (PlotError, Span) {
    (
        PlotError::InvalidSyntax(message.into()),
        parser.span(start, start + length),
    )
}
