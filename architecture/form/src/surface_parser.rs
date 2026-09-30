use crate::prelude::*;
use crate::surface_lex::{
    delimiters_are_balanced, is_name, is_reference, is_source_import_path, location,
    split_top_level, split_top_level_token, top_level_positions, top_level_token_positions,
    SourceLine,
};
use crate::syntax::{
    Argument, BackStatement, ConstructionRole, ConstructionSyntax, Cord, CordStage, Expression,
    FormCompletionPolicy, FormFront, FormSyntax, Invocation, LocalValue, MatchedRoute,
    MatchedRouteArm, MatchedRoutePattern, NamedGear, RetainedDuration, RetainedValue,
    RuntimePortDirection, RuntimePortTemporal, SpannedText, SyntaxDocument, TypeSyntax,
    UseDeclaration,
};
use crate::{
    diagnostic, eof_span, tokenize_losslessly, FormError, Span, MAXIMUM_FORM_SOURCE_BYTES,
    MAXIMUM_USE_DECLARATIONS,
};

mod construction;
pub(crate) mod front;
mod pack;
mod shared_pool;
mod type_declaration;
use construction::parse_construction;
use front::{canonical_default_bound, parse_finite_bound, parse_port_type};
use pack::parse_pack;
use shared_pool::parse_pool_declaration;

pub(crate) fn parse_surface(source: &str) -> SyntaxDocument {
    if source.len() > MAXIMUM_FORM_SOURCE_BYTES {
        return SyntaxDocument::new(
            String::new(),
            Vec::new(),
            Vec::new(),
            true,
            (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
            vec![diagnostic(
                FormError::SourceLimitExceeded,
                crate::whole_source_span(source),
            )],
        );
    }
    let tokens = match tokenize_losslessly(source) {
        Ok(tokens) => tokens,
        Err(span) => {
            return SyntaxDocument::new(
                source.to_string(),
                Vec::new(),
                Vec::new(),
                true,
                (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
                vec![diagnostic(FormError::TokenLimitExceeded, span)],
            );
        }
    };
    match Parser::new(source).parse_document() {
        Ok(parsed) => SyntaxDocument::new(
            source.to_string(),
            tokens,
            parsed.uses,
            parsed.standard_glyphs,
            (
                parsed.types,
                parsed.forms,
                parsed.constructions,
                parsed.packages,
            ),
            Vec::new(),
        ),
        Err((error, span)) => SyntaxDocument::new(
            source.to_string(),
            tokens,
            Vec::new(),
            true,
            (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
            vec![diagnostic(error, span)],
        ),
    }
}

struct Parser<'a> {
    source: &'a str,
    lines: Vec<SourceLine<'a>>,
    index: usize,
}

struct ParsedSurface {
    uses: Vec<UseDeclaration>,
    standard_glyphs: bool,
    types: Vec<TypeSyntax>,
    forms: Vec<FormSyntax>,
    constructions: Vec<ConstructionSyntax>,
    packages: Vec<crate::syntax::PackageSyntax>,
}

struct ParsedBack {
    statements: Vec<BackStatement>,
    local_forms: Vec<FormSyntax>,
    completion: FormCompletionPolicy,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        let mut lines = Vec::new();
        let mut offset = 0;
        for raw in source.split_inclusive('\n') {
            let text = raw.strip_suffix('\n').unwrap_or(raw);
            lines.push(SourceLine {
                text,
                start: offset,
            });
            offset += raw.len();
        }
        Self {
            source,
            lines,
            index: 0,
        }
    }

    fn parse_document(mut self) -> Result<ParsedSurface, (FormError, Span)> {
        let mut uses = Vec::new();
        let mut standard_glyphs = true;
        let mut types = Vec::new();
        let mut forms = Vec::new();
        let mut constructions = Vec::new();
        let mut packages = Vec::new();
        self.skip_empty();
        while self.index < self.lines.len() {
            let (text, start) = self.lines[self.index].statement();
            if text == "sans glyphs" {
                if !standard_glyphs {
                    return Err((
                        FormError::InvalidSyntax("duplicate 'sans glyphs' header".into()),
                        self.line_span(self.lines[self.index]),
                    ));
                }
                standard_glyphs = false;
                self.index += 1;
                self.skip_empty();
                continue;
            }
            let Some(import) = text.strip_prefix("with ") else {
                break;
            };
            uses.extend(self.parse_use(import, text, start)?);
            if uses.len() > MAXIMUM_USE_DECLARATIONS {
                return Err((
                    FormError::InvalidSyntax(alloc::format!(
                        "source exceeds the {MAXIMUM_USE_DECLARATIONS}-import bound"
                    )),
                    self.line_span(self.lines[self.index]),
                ));
            }
            self.index += 1;
            self.skip_empty();
        }
        while self.index < self.lines.len() {
            let (text, _) = self.lines[self.index].statement();
            if text.starts_with("type ") {
                types.push(self.parse_type_declaration()?);
            } else if text.starts_with("form ") {
                forms.push(self.parse_form()?);
            } else if text.starts_with("host ") {
                constructions.push(parse_construction(
                    &mut self,
                    ConstructionRole::Host,
                    "host",
                )?);
            } else if text.starts_with("body ") {
                constructions.push(parse_construction(
                    &mut self,
                    ConstructionRole::Body,
                    "body",
                )?);
            } else if text.starts_with("pack ") {
                packages.push(parse_pack(&mut self)?);
            } else {
                return Err((
                    FormError::InvalidSyntax(
                        "expected 'type NAME', 'form NAME', 'host NAME', 'body NAME', or 'pack PATH' definition"
                            .into(),
                    ),
                    self.line_span(self.lines[self.index]),
                ));
            }
            self.skip_empty();
        }
        if types.is_empty() && forms.is_empty() && constructions.is_empty() && packages.is_empty() {
            return Err((FormError::IncompleteForm, eof_span(self.source)));
        }
        if !packages.is_empty()
            && (!types.is_empty()
                || !forms.is_empty()
                || !constructions.is_empty()
                || !uses.is_empty()
                || packages.len() != 1)
        {
            return Err((
                FormError::InvalidSyntax(
                    "pack.conduit contains exactly one pack declaration and no form, host, body, or with declarations".into(),
                ),
                packages[0].span,
            ));
        }
        Ok(ParsedSurface {
            uses,
            standard_glyphs,
            types,
            forms,
            constructions,
            packages,
        })
    }

    fn parse_use(
        &self,
        import: &str,
        line: &str,
        start: usize,
    ) -> Result<Vec<UseDeclaration>, (FormError, Span)> {
        let import = import.trim();
        if import.is_empty() {
            return Err(self.invalid_statement(line, start));
        }
        if let Some(open) = import.find("/{") {
            let prefix = &import[..open];
            let members = import[open + 2..].strip_suffix('}').ok_or_else(|| {
                (
                    FormError::InvalidSyntax("grouped with requires a final '}'".into()),
                    self.line_span(self.lines[self.index]),
                )
            })?;
            if !is_source_import_path(prefix) || members.trim().is_empty() {
                return Err(self.invalid_statement(line, start));
            }
            let mut declarations = Vec::new();
            for member in split_top_level(members, ',') {
                let member = member.trim();
                if !is_name(member) {
                    return Err(self.invalid_statement(line, start));
                }
                let path = alloc::format!("{prefix}/{member}");
                let member_offset = start + line.find(member).unwrap_or(0);
                declarations.push(UseDeclaration {
                    path,
                    path_span: self.span(member_offset, member_offset + member.len()),
                    alias: self.spanned(member, member_offset),
                    span: self.line_span(self.lines[self.index]),
                });
            }
            return Ok(declarations);
        }
        let (path, alias) = import
            .split_once(" as ")
            .map_or((import, None), |(path, alias)| {
                (path.trim(), Some(alias.trim()))
            });
        if !is_source_import_path(path)
            || alias.is_some_and(|alias| !crate::surface_lex::is_gear_name(alias))
        {
            return Err(self.invalid_statement(line, start));
        }
        let alias = alias.unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path));
        let path_offset = start + line.find(path).unwrap_or(0);
        let alias_offset = start + line.rfind(alias).unwrap_or(0);
        Ok(vec![UseDeclaration {
            path: path.to_string(),
            path_span: self.span(path_offset, path_offset + path.len()),
            alias: self.spanned(alias, alias_offset),
            span: self.line_span(self.lines[self.index]),
        }])
    }

    fn parse_form(&mut self) -> Result<FormSyntax, (FormError, Span)> {
        let header_line = self.lines[self.index];
        let (header, header_start) = header_line.statement();
        let rest = header.strip_prefix("form ").ok_or_else(|| {
            (
                FormError::InvalidSyntax("expected 'form NAME' definition".into()),
                self.span(header_start, header_start + header.len()),
            )
        })?;
        let boundary = rest.find(['(', '{']).ok_or_else(|| {
            (
                FormError::InvalidSyntax("expected form front or back".into()),
                self.span(header_start, header_start + header.len()),
            )
        })?;
        let name_text = rest[..boundary].trim();
        if name_text.is_empty() {
            return Err((
                FormError::InvalidSyntax("form name must not be empty".into()),
                self.span(header_start, header_start + header.len()),
            ));
        }
        let name_offset =
            header_start + "form ".len() + rest[..boundary].find(name_text).unwrap_or(0);
        let name = self.spanned(name_text, name_offset);
        let form_start = header_start;
        let marker = rest.as_bytes()[boundary] as char;
        let mut front = FormFront::default();
        if marker == '(' {
            if !rest[boundary + 1..].trim().is_empty() {
                return Err((
                    FormError::InvalidSyntax(
                        "front declarations must follow '(' on their own lines".into(),
                    ),
                    self.line_span(header_line),
                ));
            }
            let open = header_start + "form ".len() + boundary;
            self.index += 1;
            let (parsed_front, expression_body) = self.parse_front(open)?;
            front = parsed_front;
            if let Some(expression) = expression_body {
                let close = self.lines[self.index - 1];
                return Ok(FormSyntax {
                    name,
                    back: vec![expression_body_cord(
                        &front,
                        expression,
                        self.line_span(close),
                    )?],
                    front,
                    completion: FormCompletionPolicy::Live,
                    local_forms: Vec::new(),
                    span: self.span(form_start, close.start + close.text.len()),
                });
            }
        } else {
            if !rest[boundary..].trim().starts_with('{') || rest[boundary + 1..].trim() != "" {
                return Err((
                    FormError::InvalidSyntax("expected '{' to open form back".into()),
                    self.line_span(header_line),
                ));
            }
            self.index += 1;
        }
        let ParsedBack {
            statements: back,
            local_forms,
            completion,
        } = self.parse_back()?;
        let close = self.lines[self.index - 1];
        Ok(FormSyntax {
            name,
            front,
            completion,
            local_forms,
            back,
            span: self.span(form_start, close.start + close.text.len()),
        })
    }

    fn parse_back(&mut self) -> Result<ParsedBack, (FormError, Span)> {
        let mut statements = Vec::new();
        let mut local_forms = Vec::new();
        while self.index < self.lines.len() {
            let line = self.lines[self.index];
            let (text, start) = line.statement();
            if text == "}" || text == "}." {
                let completion = if text == "}." {
                    FormCompletionPolicy::SemanticCompletion
                } else {
                    FormCompletionPolicy::Live
                };
                self.index += 1;
                return Ok(ParsedBack {
                    statements,
                    local_forms,
                    completion,
                });
            }
            if text.is_empty() || text.starts_with('#') {
                self.index += 1;
                continue;
            }
            if text == "..." {
                self.index += 1;
                continue;
            }
            if text == "." {
                return Err((
                    FormError::InvalidSyntax(
                        "a semantic full stop belongs after the form body; use '}' followed immediately by '.' ('}.')"
                            .into(),
                    ),
                    self.span(start, start + text.len()),
                ));
            }
            if text == "complete" {
                return Err((
                    FormError::InvalidSyntax(
                        "'complete' is not Conduitese; use a trailing full stop after the form body ('}.')"
                            .into(),
                    ),
                    self.span(start, start + text.len()),
                ));
            }
            if text.starts_with("form ") {
                local_forms.push(self.parse_form()?);
                continue;
            }
            if let Some(source) = text.strip_suffix(">> ? {").map(str::trim) {
                statements.push(BackStatement::MatchedRoute(
                    self.parse_matched_route(source, text, start)?,
                ));
                continue;
            }
            statements.push(self.parse_statement(text, start)?);
            self.index += 1;
        }
        Err((FormError::MissingBlockEnd, eof_span(self.source)))
    }

    fn parse_matched_route(
        &mut self,
        source: &str,
        opening: &str,
        start: usize,
    ) -> Result<MatchedRoute, (FormError, Span)> {
        if !is_reference(source) {
            return Err((
                FormError::InvalidSyntax(
                    "matched routing source must be one exact reference".into(),
                ),
                self.span(start, start + opening.len()),
            ));
        }
        let route_source = self.spanned_at(source, opening, start);
        let route_start = start;
        self.index += 1;
        let mut arms = Vec::new();
        while self.index < self.lines.len() {
            let line = self.lines[self.index];
            let (text, arm_start) = line.statement();
            if text == "}" {
                if arms.is_empty() {
                    return Err((
                        FormError::InvalidSyntax(
                            "matched routing requires at least one track".into(),
                        ),
                        self.line_span(line),
                    ));
                }
                self.index += 1;
                return Ok(MatchedRoute {
                    source: route_source,
                    arms,
                    span: self.span(route_start, line.start + line.text.len()),
                });
            }
            if text.is_empty() || text.starts_with('#') {
                self.index += 1;
                continue;
            }
            let Some(split) = top_level_token_positions(text, ">>").first().copied() else {
                return Err((
                    FormError::InvalidSyntax(
                        "matched routing track requires PATTERN >> ROUTE".into(),
                    ),
                    self.line_span(line),
                ));
            };
            let pattern_text = text[..split].trim();
            let tail = text[split + 2..].trim();
            if tail.is_empty() {
                return Err(self.invalid_statement(text, arm_start));
            }
            let pattern_offset = arm_start + text.find(pattern_text).unwrap();
            let pattern_span = self.span(pattern_offset, pattern_offset + pattern_text.len());
            let pattern = if pattern_text == "_" {
                MatchedRoutePattern::Otherwise(pattern_span)
            } else {
                let body = pattern_text
                    .strip_prefix('[')
                    .and_then(|body| body.strip_suffix(']'))
                    .ok_or_else(|| {
                        (
                            FormError::InvalidSyntax(
                                "matched routing pattern requires [Type.tag], [Type.field == value], or _"
                                    .into(),
                            ),
                            pattern_span,
                        )
                    })?;
                let equals = top_level_positions(body, '=');
                if equals.len() == 2 && equals[1] == equals[0] + 1 {
                    let left = body[..equals[0]].trim();
                    let expected = body[equals[1] + 1..].trim();
                    let (value_type, field) = left.rsplit_once('.').ok_or_else(|| {
                        (
                            FormError::InvalidSyntax(
                                "guard route pattern requires [Type.field == value]".into(),
                            ),
                            pattern_span,
                        )
                    })?;
                    let value_type = value_type.trim();
                    let field = field.trim();
                    if !is_name(value_type) || !is_name(field) || expected.is_empty() {
                        return Err((
                            FormError::InvalidSyntax(
                                "guard route type and field must be canonical names and name one value"
                                    .into(),
                            ),
                            pattern_span,
                        ));
                    }
                    let body_offset = pattern_offset + 1;
                    let expected_offset = body_offset + body.rfind(expected).unwrap();
                    let syntax =
                        crate::structured_expression::parse(self.source, expected, expected_offset)
                            .map_err(|(message, span)| (FormError::InvalidSyntax(message), span))?;
                    MatchedRoutePattern::Guard {
                        value_type: self
                            .spanned(value_type, body_offset + body.find(value_type).unwrap()),
                        field: self.spanned(field, body_offset + body.find(field).unwrap()),
                        expected: Box::new(Expression {
                            text: expected.to_string(),
                            syntax,
                            span: self.span(expected_offset, expected_offset + expected.len()),
                        }),
                        span: pattern_span,
                    }
                } else {
                    let (value_type, tag) = body.rsplit_once('.').ok_or_else(|| {
                        (
                            FormError::InvalidSyntax(
                                "variant route pattern requires [Type.tag]".into(),
                            ),
                            pattern_span,
                        )
                    })?;
                    let value_type = value_type.trim();
                    let tag = tag.trim();
                    if !is_name(value_type) || !is_name(tag) {
                        return Err((
                            FormError::InvalidSyntax(
                                "variant route type and tag must be canonical names".into(),
                            ),
                            pattern_span,
                        ));
                    }
                    let body_offset = pattern_offset + 1;
                    MatchedRoutePattern::Variant {
                        value_type: self
                            .spanned(value_type, body_offset + body.find(value_type).unwrap()),
                        tag: self.spanned(tag, body_offset + body.rfind(tag).unwrap()),
                        span: pattern_span,
                    }
                }
            };
            let tail_start = arm_start + text.find(tail).unwrap();
            let stages = self.parse_cord_stages(tail, tail_start)?;
            arms.push(MatchedRouteArm {
                pattern,
                stages,
                span: self.line_span(line),
            });
            self.index += 1;
        }
        Err((FormError::MissingBlockEnd, eof_span(self.source)))
    }

    fn parse_statement(
        &self,
        text: &str,
        start: usize,
    ) -> Result<BackStatement, (FormError, Span)> {
        if let Some(declaration) = text.strip_prefix("pool ") {
            return parse_pool_declaration(self, declaration, text, start).map(BackStatement::Pool);
        }
        if let Some(cord) = self.parse_unary_glyph_cord(text, start) {
            return cord.map(BackStatement::Cord);
        }
        if has_top_level_cord(text) {
            return self.parse_cord(text, start).map(BackStatement::Cord);
        }
        if let Some(equal) = top_level_assignment(text) {
            let name = text[..equal].trim();
            let value = text[equal + 1..].trim();
            if !is_name(name) || value.is_empty() || top_level_assignment(value).is_some() {
                return Err(self.invalid_statement(text, start));
            }
            return Ok(BackStatement::LocalValue(LocalValue {
                name: self.spanned_at(name, text, start),
                value: self.expression_at(value, text, start)?,
                span: self.span(start, start + text.len()),
            }));
        }
        if has_legacy_top_level_cord(text) {
            return Err((
                FormError::InvalidSyntax("'>' is not a Conduitese cord; use '>>'".into()),
                self.span(start, start + text.len()),
            ));
        }
        if let Some(colon) = top_level_positions(text, ':').first().copied() {
            let name = text[..colon].trim();
            let invoked = text[colon + 1..].trim();
            if !crate::surface_lex::is_gear_name(name) || invoked.is_empty() {
                return Err((
                    FormError::InvalidSyntax("missing Gear Kind after ':'".into()),
                    self.span(start, start + text.len()),
                ));
            }
            let invoked_start = start + text.find(invoked).unwrap();
            if let Some(retained) = invoked.strip_prefix("keep ") {
                let retained =
                    self.parse_retained_value(retained, invoked_start + "keep ".len())?;
                return Ok(BackStatement::NamedGear(NamedGear {
                    name: self.spanned_at(name, text, start),
                    invocation: Invocation {
                        kind: self.spanned("state/latest", invoked_start),
                        arguments: Vec::new(),
                        span: self.span(invoked_start, start + text.len()),
                    },
                    retained: Some(Box::new(retained)),
                    span: self.span(start, start + text.len()),
                }));
            }
            let invocation = self.parse_invocation(invoked, invoked_start)?;
            return Ok(BackStatement::NamedGear(NamedGear {
                name: self.spanned_at(name, text, start),
                invocation,
                retained: None,
                span: self.span(start, start + text.len()),
            }));
        }
        Err(self.invalid_statement(text, start))
    }

    /// The fixed unary glyph grammar is `source glyph target`. It is exactly
    /// sugar for `source >> glyph >> target`; the glyph contributes no
    /// precedence, fixity, hidden operand, or alternate parsing production.
    fn parse_unary_glyph_cord(
        &self,
        text: &str,
        start: usize,
    ) -> Option<Result<Cord, (FormError, Span)>> {
        let parts = text.split_whitespace().collect::<Vec<_>>();
        if parts.len() != 3 || !crate::surface_lex::is_glyph(parts[1]) {
            return None;
        }
        if !is_reference(parts[0]) || !is_reference(parts[2]) {
            return Some(Err(self.invalid_statement(text, start)));
        }
        let source_offset = text.find(parts[0]).unwrap_or(0);
        let glyph_offset = text[source_offset + parts[0].len()..]
            .find(parts[1])
            .map_or(0, |offset| source_offset + parts[0].len() + offset);
        let target_offset = text[glyph_offset + parts[1].len()..]
            .find(parts[2])
            .map_or(0, |offset| glyph_offset + parts[1].len() + offset);
        Some(Ok(Cord {
            stages: vec![
                CordStage::Reference(self.spanned(parts[0], start + source_offset)),
                CordStage::Glyph(self.spanned(parts[1], start + glyph_offset)),
                CordStage::Reference(self.spanned(parts[2], start + target_offset)),
            ],
            span: self.span(start, start + text.len()),
        }))
    }

    fn parse_retained_value(
        &self,
        source: &str,
        start: usize,
    ) -> Result<RetainedValue, (FormError, Span)> {
        let durations = [
            (" for this step", RetainedDuration::Step),
            (" for this play", RetainedDuration::Play),
            (" for this wake", RetainedDuration::Wake),
            (" for this boot", RetainedDuration::Boot),
            (" for this body", RetainedDuration::Body),
            (" for life", RetainedDuration::Body),
        ];
        let (value, duration) = durations
            .iter()
            .find_map(|(suffix, duration)| {
                source
                    .strip_suffix(suffix)
                    .map(|value| (value.trim(), *duration))
            })
            .unwrap_or((source.trim(), RetainedDuration::Step));
        let parts = split_top_level_token(value, "<=");
        let (value, explicit_bound) = match parts.as_slice() {
            [value] => (value.trim(), None),
            [value, bound] => (
                value.trim(),
                Some(
                    parse_finite_bound(bound)
                        .ok_or_else(|| self.invalid_statement(source, start))?,
                ),
            ),
            _ => return Err(self.invalid_statement(source, start)),
        };
        let (value_type, initial) = if let Some(open) = value.find('(') {
            if !value.ends_with(')') {
                return Err(self.invalid_statement(source, start));
            }
            let initial = value[open + 1..value.len() - 1].trim();
            if initial.is_empty() {
                return Err(self.invalid_statement(source, start));
            }
            (
                &value[..open],
                Some(self.expression_at(initial, value, start)?),
            )
        } else {
            (value, None)
        };
        let (value_type, temporal) = parse_port_type(value_type.trim())
            .ok_or_else(|| self.invalid_statement(source, start))?;
        let optional = match temporal {
            RuntimePortTemporal::Value => false,
            RuntimePortTemporal::OptionalValue => true,
            _ => return Err(self.invalid_statement(source, start)),
        };
        Ok(RetainedValue {
            value_type: self.spanned(value_type, start + source.find(value_type).unwrap_or(0)),
            optional,
            maximum_bytes: explicit_bound.or_else(|| canonical_default_bound(value_type)),
            initial,
            duration,
        })
    }

    fn parse_cord(&self, text: &str, start: usize) -> Result<Cord, (FormError, Span)> {
        let parts = split_top_level_token(text, ">>");
        if parts.len() < 2 || parts.iter().any(|part| part.trim().is_empty()) {
            return Err(self.invalid_statement(text, start));
        }
        let stages = self.parse_cord_stages(text, start)?;
        Ok(Cord {
            stages,
            span: self.span(start, start + text.len()),
        })
    }

    fn parse_cord_stages(
        &self,
        text: &str,
        start: usize,
    ) -> Result<Vec<CordStage>, (FormError, Span)> {
        let parts = split_top_level_token(text, ">>");
        if parts.iter().any(|part| part.trim().is_empty()) {
            return Err(self.invalid_statement(text, start));
        }
        let mut stages = Vec::new();
        let mut search = 0;
        for part in parts {
            let part = part.trim();
            let relative = text[search..].find(part).unwrap() + search;
            let part_start = start + relative;
            search = relative + part.len();
            if let Some(expression) = part
                .strip_prefix("when(")
                .and_then(|expression| expression.strip_suffix(')'))
            {
                let expression_start = part_start + "when(".len();
                stages.push(CordStage::When(self.expression_at(
                    expression,
                    expression,
                    expression_start,
                )?));
            } else if let Some((endpoint, terminal)) = parse_terminal_projection(part) {
                stages.push(CordStage::TerminalProjection {
                    endpoint: self.spanned(endpoint, part_start),
                    terminal,
                    span: self.span(part_start, part_start + part.len()),
                });
            } else if let Some(gear) = part.strip_suffix('~').filter(|gear| is_reference(gear)) {
                stages.push(CordStage::Cancellation {
                    gear: self.spanned(gear, part_start),
                    span: self.span(part_start, part_start + part.len()),
                });
            } else if let Some(selector) =
                crate::structured_selector::parse(self.source, part, part_start)
            {
                let selector = selector
                    .map_err(|(message, span)| (FormError::InvalidSyntax(message), span))?;
                stages.push(CordStage::StructuredSelector(selector));
            } else if part.starts_with('(') && part.ends_with(')') {
                stages.push(CordStage::PureExpression(
                    self.expression_at(part, part, part_start)?,
                ));
            } else if let Some(relational) = self.parse_relational_glyph(part, part_start)? {
                stages.push(relational);
            } else if crate::surface_lex::is_glyph(part) {
                stages.push(CordStage::Glyph(self.spanned(part, part_start)));
            } else if part.contains('/') || part.contains('(') {
                stages.push(CordStage::InlineGear(
                    self.parse_invocation(part, part_start)?,
                ));
            } else if part.starts_with('"') && part.ends_with('"') {
                stages.push(CordStage::Literal(
                    self.expression_at(part, part, part_start)?,
                ));
            } else if is_reference(part) {
                stages.push(CordStage::Reference(self.spanned(part, part_start)));
            } else {
                return Err((
                    FormError::InvalidSyntax("an expression cannot appear as a graph stage".into()),
                    self.span(part_start, part_start + part.len()),
                ));
            }
        }
        Ok(stages)
    }

    fn parse_relational_glyph(
        &self,
        text: &str,
        start: usize,
    ) -> Result<Option<CordStage>, (FormError, Span)> {
        let parts = text.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 3 || parts.len() % 2 == 0 {
            return Ok(None);
        }
        let glyph = parts[1];
        if !crate::surface_lex::is_glyph(glyph) {
            return Ok(None);
        }
        if parts
            .iter()
            .skip(1)
            .step_by(2)
            .any(|candidate| *candidate != glyph)
        {
            return Err((
                FormError::InvalidSyntax("mixed adjacent glyphs require explicit grouping".into()),
                self.span(start, start + text.len()),
            ));
        }
        if parts
            .iter()
            .step_by(2)
            .any(|operand| !is_reference(operand))
        {
            return Err(self.invalid_statement(text, start));
        }
        let mut search = 0;
        let mut operands = Vec::new();
        for operand in parts.iter().step_by(2) {
            let offset = text[search..].find(operand).unwrap_or(0) + search;
            operands.push(self.spanned(operand, start + offset));
            search = offset + operand.len();
        }
        let glyph_offset = text.find(glyph).unwrap_or(0);
        Ok(Some(CordStage::RelationalGlyph {
            operands,
            glyph: self.spanned(glyph, start + glyph_offset),
            span: self.span(start, start + text.len()),
        }))
    }

    fn parse_invocation(&self, text: &str, start: usize) -> Result<Invocation, (FormError, Span)> {
        if !top_level_positions(text, '{').is_empty() {
            return Err((
                FormError::InvalidSyntax("a gear invocation cannot have a form back".into()),
                self.span(start, start + text.len()),
            ));
        }
        let (gear, arguments, end) = if let Some(open) = text.find('(') {
            if !text.ends_with(')') {
                return Err(self.invalid_statement(text, start));
            }
            (&text[..open], &text[open + 1..text.len() - 1], text.len())
        } else {
            (text, "", text.len())
        };
        let gear = gear.trim();
        if gear.is_empty() || !crate::surface_lex::is_gear_name(gear) {
            return Err((
                FormError::InvalidSyntax("invalid gear reference".into()),
                self.span(start, start + text.len()),
            ));
        }
        let mut parsed = Vec::new();
        let mut saw_named = false;
        for argument in split_top_level(arguments, ',') {
            let argument = argument.trim();
            if argument.is_empty() {
                if !arguments.trim().is_empty() {
                    return Err((
                        FormError::InvalidSyntax("empty invocation argument".into()),
                        self.span(start, start + text.len()),
                    ));
                }
                continue;
            }
            let argument_start = start + text.find(argument).unwrap();
            if let Some(equal) = top_level_positions(argument, '=').first().copied() {
                saw_named = true;
                let name = argument[..equal].trim();
                let value = argument[equal + 1..].trim();
                if !is_name(name) || value.is_empty() {
                    return Err(self.invalid_statement(argument, argument_start));
                }
                parsed.push(Argument::Named {
                    name: self.spanned_at(name, argument, argument_start),
                    value: self.expression_at(value, argument, argument_start)?,
                    span: self.span(argument_start, argument_start + argument.len()),
                });
            } else {
                if saw_named {
                    return Err((
                        FormError::InvalidSyntax(
                            "positional argument cannot follow a named argument".into(),
                        ),
                        self.span(argument_start, argument_start + argument.len()),
                    ));
                }
                parsed.push(Argument::Positional(self.expression_at(
                    argument,
                    argument,
                    argument_start,
                )?));
            }
        }
        Ok(Invocation {
            kind: self.spanned(gear, start + text.find(gear).unwrap()),
            arguments: parsed,
            span: self.span(start, start + end),
        })
    }

    fn expression_at(
        &self,
        value: &str,
        container: &str,
        start: usize,
    ) -> Result<Expression, (FormError, Span)> {
        let value = value.trim();
        if value.is_empty() || !delimiters_are_balanced(value) {
            return Err(self.invalid_statement(container, start));
        }
        let offset = start + container.find(value).unwrap();
        let syntax = crate::pure_expression::parse(self.source, value, offset)
            .map_err(|(message, span)| (FormError::InvalidSyntax(message), span))?;
        Ok(Expression {
            text: value.to_string(),
            syntax,
            span: self.span(offset, offset + value.len()),
        })
    }

    fn skip_empty(&mut self) {
        while self.index < self.lines.len() {
            let (text, _) = self.lines[self.index].statement();
            if !text.is_empty() {
                break;
            }
            self.index += 1;
        }
    }

    fn invalid_statement(&self, text: &str, start: usize) -> (FormError, Span) {
        (
            FormError::InvalidSyntax(text.to_string()),
            self.span(start, start + text.len()),
        )
    }

    fn spanned_at(&self, text: &str, container: &str, start: usize) -> SpannedText {
        self.spanned(text, start + container.find(text).unwrap())
    }

    fn spanned(&self, text: &str, start: usize) -> SpannedText {
        SpannedText {
            text: text.to_string(),
            span: self.span(start, start + text.len()),
        }
    }

    fn line_span(&self, line: SourceLine<'_>) -> Span {
        let (text, start) = line.statement();
        self.span(start, start + text.len())
    }

    fn span(&self, start: usize, end: usize) -> Span {
        let (line, column) = location(self.source, start);
        let (end_line, end_column) = location(self.source, end);
        Span {
            start,
            end,
            line,
            column,
            end_line,
            end_column,
        }
    }
}

fn has_top_level_cord(text: &str) -> bool {
    top_level_token_positions(text, ">>")
        .into_iter()
        .any(|position| !text[..position].ends_with('>') && !text[position + 2..].starts_with('>'))
}

fn expression_body_cord(
    front: &FormFront,
    expression: Expression,
    span: Span,
) -> Result<BackStatement, (FormError, Span)> {
    let inputs = front
        .runtime_ports
        .iter()
        .filter(|port| port.direction == RuntimePortDirection::Input)
        .collect::<Vec<_>>();
    let outputs = front
        .runtime_ports
        .iter()
        .filter(|port| port.direction == RuntimePortDirection::Output)
        .collect::<Vec<_>>();
    if inputs.len() != 1 || outputs.len() != 1 || inputs[0].temporal != outputs[0].temporal {
        return Err((
            FormError::InvalidSyntax(
                "expression-bodied form requires exactly one runtime input and one runtime output with the same temporal contract; write an ordinary explicit Form back"
                    .into(),
            ),
            span,
        ));
    }
    Ok(BackStatement::Cord(Cord {
        stages: vec![
            CordStage::Reference(inputs[0].name.clone()),
            CordStage::PureExpression(expression),
            CordStage::Reference(outputs[0].name.clone()),
        ],
        span,
    }))
}

fn has_legacy_top_level_cord(text: &str) -> bool {
    top_level_positions(text, '>').into_iter().any(|position| {
        !text[..position]
            .chars()
            .next_back()
            .is_some_and(|character| matches!(character, '>' | '='))
            && !text[position + 1..].starts_with(['>', '='])
    })
}

fn top_level_assignment(text: &str) -> Option<usize> {
    top_level_positions(text, '=').into_iter().find(|position| {
        !text[..*position]
            .chars()
            .next_back()
            .is_some_and(|character| matches!(character, '<' | '>' | '!' | '='))
            && !text[*position + 1..].starts_with('=')
    })
}

fn parse_terminal_projection(part: &str) -> Option<(&str, crate::TerminalProjection)> {
    let (endpoint, terminal) = if let Some(endpoint) = part.strip_suffix('|') {
        (endpoint, crate::TerminalProjection::NormalClose)
    } else {
        (part.strip_suffix('!')?, crate::TerminalProjection::Abnormal)
    };
    is_reference(endpoint).then_some((endpoint, terminal))
}
