use super::Parser;
use crate::prelude::*;
use crate::surface_lex::{is_name, is_operation};
use crate::syntax::{PackageRequirementSyntax, PackageSyntax};
use crate::{PlotError, Span, MAXIMUM_PACKAGE_EXPORTS, MAXIMUM_PACKAGE_REQUIREMENTS};

pub(super) fn parse_pack(parser: &mut Parser<'_>) -> Result<PackageSyntax, (PlotError, Span)> {
    let opening = parser.lines[parser.index];
    let (header, header_start) = opening.statement();
    let declaration = header
        .strip_prefix("pack ")
        .and_then(|value| value.strip_suffix('('))
        .map(str::trim)
        .filter(|value| is_operation(value) && value.contains('/'))
        .ok_or_else(|| invalid(parser, "expected 'pack PATH ('", opening))?;
    let path_offset = header_start + header.find(declaration).unwrap_or(0);
    let path = parser.spanned(declaration, path_offset);
    parser.index += 1;
    parser.skip_empty();

    let version_line = parser.lines.get(parser.index).copied().ok_or_else(|| {
        (
            PlotError::MissingBlockEnd,
            parser.span(header_start, header.len()),
        )
    })?;
    let (version_statement, version_start) = version_line.statement();
    let version_literal = version_statement
        .strip_prefix("version")
        .and_then(|value| value.trim_start().strip_prefix('='))
        .map(str::trim)
        .ok_or_else(|| {
            invalid(
                parser,
                "pack header needs version = MAJOR.MINOR.PATCH",
                version_line,
            )
        })?;
    let version = valid_version(version_literal)
        .then_some(version_literal)
        .ok_or_else(|| {
            invalid(
                parser,
                "pack version must be a finite semantic version literal",
                version_line,
            )
        })?;
    let literal_offset = version_start + version_statement.find(version_literal).unwrap_or(0);
    let version = parser.spanned(version, literal_offset);
    parser.index += 1;
    parser.skip_empty();

    let body_open = parser.lines.get(parser.index).copied().ok_or_else(|| {
        (
            PlotError::MissingBlockEnd,
            parser.span(header_start, header.len()),
        )
    })?;
    if body_open.statement().0 != ") {" {
        return Err(invalid(
            parser,
            "expected ') {' after pack header",
            body_open,
        ));
    }
    parser.index += 1;

    let mut exports = Vec::new();
    let mut requirements = Vec::new();
    loop {
        let line = parser.lines.get(parser.index).copied().ok_or_else(|| {
            (
                PlotError::MissingBlockEnd,
                parser.span(header_start, header.len()),
            )
        })?;
        let (statement, start) = line.statement();
        if statement.is_empty() {
            parser.index += 1;
            continue;
        }
        if statement == "}" {
            parser.index += 1;
            return Ok(PackageSyntax {
                path,
                version,
                exports,
                requirements,
                span: parser.span(header_start, line.start + line.text.len()),
            });
        }
        if let Some(name) = statement.strip_prefix("ship ").map(str::trim) {
            if !is_name(name)
                || exports
                    .iter()
                    .any(|item: &crate::syntax::SpannedText| item.text == name)
            {
                return Err(invalid(parser, "pack shipment must be a unique name", line));
            }
            if exports.len() == MAXIMUM_PACKAGE_EXPORTS {
                return Err(invalid(
                    parser,
                    "pack exceeds the finite shipment bound",
                    line,
                ));
            }
            let offset = start + statement.find(name).unwrap_or(0);
            exports.push(parser.spanned(name, offset));
            parser.index += 1;
            continue;
        }
        if let Some(requirement) = statement.strip_prefix("need ").map(str::trim) {
            let (required_path, version_literal) = requirement
                .split_once('=')
                .map(|(path, version)| (path.trim(), version.trim()))
                .ok_or_else(|| invalid(parser, "need takes PATH = VERSION", line))?;
            let version_requirement = valid_requirement(version_literal)
                .then_some(version_literal)
                .ok_or_else(|| invalid(parser, "needed version is invalid", line))?;
            if !is_operation(required_path)
                || !required_path.contains('/')
                || requirements
                    .iter()
                    .any(|item: &PackageRequirementSyntax| item.path.text == required_path)
            {
                return Err(invalid(parser, "required pack path must be unique", line));
            }
            if requirements.len() == MAXIMUM_PACKAGE_REQUIREMENTS {
                return Err(invalid(
                    parser,
                    "pack exceeds the finite requirement bound",
                    line,
                ));
            }
            let path_offset = start + statement.find(required_path).unwrap_or(0);
            let version_offset = start + statement.find(version_literal).unwrap_or(0);
            requirements.push(PackageRequirementSyntax {
                path: parser.spanned(required_path, path_offset),
                version_requirement: parser.spanned(version_requirement, version_offset),
                span: parser.line_span(line),
            });
            parser.index += 1;
            continue;
        }
        return Err(invalid(
            parser,
            "pack body accepts only ship NAME or need PATH = VERSION",
            line,
        ));
    }
}

fn valid_version(value: &str) -> bool {
    let mut parts = value.split('.');
    (0..3).all(|_| parts.next().is_some_and(decimal)) && parts.next().is_none()
}

fn valid_requirement(value: &str) -> bool {
    let version = value.strip_prefix('^').unwrap_or(value);
    let count = version.split('.').count();
    (2..=3).contains(&count) && version.split('.').all(decimal)
}

fn decimal(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
}

fn invalid(
    parser: &Parser<'_>,
    message: &str,
    line: crate::surface_lex::SourceLine<'_>,
) -> (PlotError, Span) {
    (
        PlotError::InvalidSyntax(message.into()),
        parser.line_span(line),
    )
}
