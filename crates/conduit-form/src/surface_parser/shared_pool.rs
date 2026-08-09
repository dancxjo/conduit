use super::Parser;
use crate::surface_lex::{is_name, top_level_positions};
use crate::syntax::{Argument, PoolDeclaration};
use crate::{FormError, Span};

pub(super) fn parse_pool_declaration(
    parser: &Parser<'_>,
    declaration: &str,
    line: &str,
    start: usize,
) -> Result<PoolDeclaration, (FormError, Span)> {
    let Some(colon) = top_level_positions(declaration, ':').first().copied() else {
        return Err(parser.invalid_statement(line, start));
    };
    let name = declaration[..colon].trim();
    let member = declaration[colon + 1..].trim();
    if !is_name(name) {
        return Err(parser.invalid_statement(line, start));
    }
    let invocation_start = start + line.find(member).unwrap_or(0);
    let invocation = parser.parse_invocation(member, invocation_start)?;
    let [Argument::Named {
        name: size_name,
        value,
        ..
    }] = invocation.arguments.as_slice()
    else {
        return Err((
            FormError::InvalidSyntax(
                "pool declaration requires exactly one named 'size' bound".into(),
            ),
            parser.span(start, start + line.len()),
        ));
    };
    let maximum_members = value.text.parse::<u16>().ok().filter(|size| *size > 0);
    if size_name.text != "size" || maximum_members.is_none() {
        return Err((
            FormError::InvalidSyntax(
                "pool size must be one positive integer representable as u16".into(),
            ),
            value.span,
        ));
    }
    Ok(PoolDeclaration {
        name: parser.spanned_at(name, line, start),
        member_form: invocation.kind,
        maximum_members: maximum_members.expect("positive pool size was checked"),
        span: parser.span(start, start + line.len()),
    })
}
