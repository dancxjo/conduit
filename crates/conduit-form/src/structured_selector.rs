use alloc::string::String;

use crate::surface_lex::{is_name, location, split_top_level, split_top_level_once};
use crate::{Span, SpannedText, StructuredSelectorSyntax};

pub(crate) fn parse(
    source: &str,
    text: &str,
    start: usize,
) -> Option<Result<StructuredSelectorSyntax, (String, Span)>> {
    let (operation, body) = ["project", "index", "select"]
        .into_iter()
        .find_map(|operation| {
            text.strip_prefix(operation)
                .and_then(|rest| rest.strip_prefix('('))
                .and_then(|rest| rest.strip_suffix(')'))
                .map(|body| (operation, body))
        })?;
    Some(match operation {
        "project" => parse_project(source, text, body, start),
        "index" => parse_index(source, text, body, start),
        "select" => parse_select(source, text, body, start),
        _ => unreachable!("selector operation comes from a closed set"),
    })
}

fn parse_project(
    source: &str,
    text: &str,
    body: &str,
    start: usize,
) -> Result<StructuredSelectorSyntax, (String, Span)> {
    let (value_type, field) = body
        .rsplit_once('.')
        .ok_or_else(|| error(source, start, text, "project selector requires Type.field"))?;
    let (value_type, field) = checked_pair(source, text, start, value_type, field)?;
    Ok(StructuredSelectorSyntax::Field {
        value_type,
        field,
        span: source_span(source, start, start + text.len()),
    })
}

fn parse_index(
    source: &str,
    text: &str,
    body: &str,
    start: usize,
) -> Result<StructuredSelectorSyntax, (String, Span)> {
    let open = body
        .rfind('[')
        .ok_or_else(|| error(source, start, text, "index selector requires Type[INDEX]"))?;
    let value_type = body[..open].trim();
    let index = body[open + 1..]
        .strip_suffix(']')
        .map(str::trim)
        .ok_or_else(|| error(source, start, text, "index selector requires Type[INDEX]"))?;
    if !is_name(value_type) || index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(error(
            source,
            start,
            text,
            "index selector requires a type name and decimal fixed index",
        ));
    }
    Ok(StructuredSelectorSyntax::Index {
        value_type: spanned(source, text, start, value_type),
        index: spanned(source, text, start, index),
        span: source_span(source, start, start + text.len()),
    })
}

fn parse_select(
    source: &str,
    text: &str,
    body: &str,
    start: usize,
) -> Result<StructuredSelectorSyntax, (String, Span)> {
    let parts = split_top_level(body, ',');
    if parts.len() != 2 {
        return Err(error(
            source,
            start,
            text,
            "variant selector requires Type.tag, unmatched=drop|refuse",
        ));
    }
    let (value_type, tag) = parts[0]
        .trim()
        .rsplit_once('.')
        .ok_or_else(|| error(source, start, text, "variant selector requires Type.tag"))?;
    let (value_type, tag) = checked_pair(source, text, start, value_type, tag)?;
    let (name, unmatched) = split_top_level_once(parts[1].trim(), '=');
    let unmatched = unmatched.map(str::trim).ok_or_else(|| {
        error(
            source,
            start,
            text,
            "variant selector requires unmatched=drop|refuse",
        )
    })?;
    if name.trim() != "unmatched" || !matches!(unmatched, "drop" | "refuse") {
        return Err(error(
            source,
            start,
            text,
            "variant selector unmatched disposition must be drop or refuse",
        ));
    }
    Ok(StructuredSelectorSyntax::Variant {
        value_type,
        tag,
        unmatched: spanned(source, text, start, unmatched),
        span: source_span(source, start, start + text.len()),
    })
}

fn checked_pair(
    source: &str,
    container: &str,
    start: usize,
    value_type: &str,
    member: &str,
) -> Result<(SpannedText, SpannedText), (String, Span)> {
    let value_type = value_type.trim();
    let member = member.trim();
    if !is_name(value_type) || !is_name(member) {
        return Err(error(
            source,
            start,
            container,
            "selector type and member names must be canonical names",
        ));
    }
    Ok((
        spanned(source, container, start, value_type),
        spanned(source, container, start, member),
    ))
}

fn spanned(source: &str, container: &str, start: usize, text: &str) -> SpannedText {
    let relative = container.find(text).unwrap_or(0);
    SpannedText {
        text: text.into(),
        span: source_span(source, start + relative, start + relative + text.len()),
    }
}

fn error(source: &str, start: usize, text: &str, message: &str) -> (String, Span) {
    (
        message.into(),
        source_span(source, start, start + text.len()),
    )
}

fn source_span(source: &str, start: usize, end: usize) -> Span {
    let (line, column) = location(source, start);
    let (end_line, end_column) = location(source, end);
    Span {
        start,
        end,
        line,
        column,
        end_line,
        end_column,
    }
}
