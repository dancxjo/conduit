use crate::prelude::*;
use crate::surface_lex::{
    delimiters_are_balanced, is_name, is_operation, is_reference, location, split_declaration,
    split_top_level, split_top_level_once, top_level_positions, SourceLine,
};
use crate::syntax::{
    Argument, BackStatement, ConstructionRole, ConstructionSyntax, Cord, CordStage, Expression,
    FormCompletionPolicy, FormFront, FormSyntax, Invocation, LocalValue, MatchedRoute,
    MatchedRouteArm, MatchedRoutePattern, NamedGear, RuntimePort, RuntimePortDirection,
    RuntimePortTemporal, ShorthandPair, SpannedText, StartupParameter, SyntaxDocument,
};
use crate::{
    diagnostic, eof_span, tokenize_losslessly, FormError, Span, MAXIMUM_FORM_SOURCE_BYTES,
};

mod construction;
mod shared_pool;
use construction::parse_construction;
use shared_pool::parse_pool_declaration;

pub(crate) fn parse_surface(source: &str) -> SyntaxDocument {
    if source.len() > MAXIMUM_FORM_SOURCE_BYTES {
        return SyntaxDocument::new(
            String::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
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
                Vec::new(),
                vec![diagnostic(FormError::TokenLimitExceeded, span)],
            );
        }
    };
    match Parser::new(source).parse_document() {
        Ok((forms, constructions)) => {
            SyntaxDocument::new(source.to_string(), tokens, forms, constructions, Vec::new())
        }
        Err((error, span)) => SyntaxDocument::new(
            source.to_string(),
            tokens,
            Vec::new(),
            Vec::new(),
            vec![diagnostic(error, span)],
        ),
    }
}

fn parse_port_type(value_type: &str) -> Option<(&str, RuntimePortTemporal)> {
    let (value_type, temporal) = if let Some(value_type) = value_type.strip_prefix('$') {
        (value_type, RuntimePortTemporal::Current)
    } else if let Some(value_type) = value_type.strip_suffix("...|") {
        (value_type, RuntimePortTemporal::Flow { closes: true })
    } else if let Some(value_type) = value_type.strip_suffix("...") {
        (value_type, RuntimePortTemporal::Flow { closes: false })
    } else {
        (value_type, RuntimePortTemporal::Value)
    };
    (!value_type.is_empty()
        && !value_type.starts_with('$')
        && !value_type.ends_with("...")
        && !value_type.ends_with("...|"))
    .then_some((value_type, temporal))
}

struct Parser<'a> {
    source: &'a str,
    lines: Vec<SourceLine<'a>>,
    index: usize,
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

    fn parse_document(
        mut self,
    ) -> Result<(Vec<FormSyntax>, Vec<ConstructionSyntax>), (FormError, Span)> {
        let mut forms = Vec::new();
        let mut constructions = Vec::new();
        self.skip_empty();
        while self.index < self.lines.len() {
            let (text, _) = self.lines[self.index].statement();
            if text.starts_with("form ") {
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
            } else {
                return Err((
                    FormError::InvalidSyntax(
                        "expected 'form NAME', 'host NAME', or 'body NAME' definition".into(),
                    ),
                    self.line_span(self.lines[self.index]),
                ));
            }
            self.skip_empty();
        }
        if forms.is_empty() && constructions.is_empty() {
            return Err((FormError::IncompleteForm, eof_span(self.source)));
        }
        Ok((forms, constructions))
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
            front = self.parse_front(open)?;
        } else {
            if !rest[boundary..].trim().starts_with('{') || rest[boundary + 1..].trim() != "" {
                return Err((
                    FormError::InvalidSyntax("expected '{' to open form back".into()),
                    self.line_span(header_line),
                ));
            }
            self.index += 1;
        }
        let (back, completion) = self.parse_back()?;
        let close = self.lines[self.index - 1];
        Ok(FormSyntax {
            name,
            front,
            completion,
            back,
            span: self.span(form_start, close.start + close.text.len()),
        })
    }

    fn parse_front(&mut self, open: usize) -> Result<FormFront, (FormError, Span)> {
        let mut front = FormFront::default();
        while self.index < self.lines.len() {
            let line = self.lines[self.index];
            let (text, start) = line.statement();
            if text == ") {" || text == "){" {
                front.span = Some(self.span(open, start + text.find(')').unwrap() + 1));
                self.index += 1;
                return Ok(front);
            }
            if text.is_empty() || text.starts_with('#') {
                self.index += 1;
                continue;
            }
            if text.contains('>') {
                self.parse_front_runtime(text, start, &mut front)?;
            } else {
                front
                    .startup_parameters
                    .push(self.parse_startup(text, start)?);
            }
            self.index += 1;
        }
        Err((FormError::IncompleteForm, eof_span(self.source)))
    }

    fn parse_startup(
        &self,
        text: &str,
        start: usize,
    ) -> Result<StartupParameter, (FormError, Span)> {
        let (left, default) = split_top_level_once(text, '=');
        let (name, value_type) =
            split_declaration(left).ok_or_else(|| self.invalid_statement(text, start))?;
        let span = self.span(start, start + text.len());
        Ok(StartupParameter {
            name: self.spanned_at(name, text, start),
            value_type: self.spanned_at(value_type, text, start),
            default: default
                .map(|value| self.expression_at(value, text, start))
                .transpose()?,
            span,
        })
    }

    fn parse_front_runtime(
        &self,
        text: &str,
        start: usize,
        front: &mut FormFront,
    ) -> Result<(), (FormError, Span)> {
        let arrows = top_level_positions(text, '>');
        if arrows.len() != 1 {
            return Err((
                FormError::InvalidSyntax("malformed front arrows".into()),
                self.span(start, start + text.len()),
            ));
        }
        let arrow = arrows[0];
        let left = text[..arrow].trim();
        let right = text[arrow + 1..].trim();
        match (left.is_empty(), right.is_empty()) {
            (true, false) => front.runtime_ports.push(self.runtime_port(
                right,
                text,
                start,
                RuntimePortDirection::Input,
            )?),
            (false, true) => front.runtime_ports.push(self.runtime_port(
                left,
                text,
                start,
                RuntimePortDirection::Output,
            )?),
            (false, false) => {
                if front.shorthand.is_some() {
                    return Err((
                        FormError::InvalidSyntax("more than one shorthand front pair".into()),
                        self.span(start, start + text.len()),
                    ));
                }
                let input = self.runtime_port(left, text, start, RuntimePortDirection::Input)?;
                let output = self.runtime_port(right, text, start, RuntimePortDirection::Output)?;
                front.shorthand = Some(ShorthandPair {
                    input_port: input.name.clone(),
                    output_port: output.name.clone(),
                    span: self.span(start, start + text.len()),
                });
                front.runtime_ports.extend([input, output]);
            }
            (true, true) => return Err(self.invalid_statement(text, start)),
        }
        Ok(())
    }

    fn runtime_port(
        &self,
        declaration: &str,
        line: &str,
        start: usize,
        direction: RuntimePortDirection,
    ) -> Result<RuntimePort, (FormError, Span)> {
        let (name, value_type) =
            split_declaration(declaration).ok_or_else(|| self.invalid_statement(line, start))?;
        let (value_type, temporal) =
            parse_port_type(value_type).ok_or_else(|| self.invalid_statement(line, start))?;
        Ok(RuntimePort {
            name: self.spanned_at(name, line, start),
            value_type: self.spanned_at(value_type, line, start),
            direction,
            temporal,
            span: self.span(
                start + line.find(declaration).unwrap(),
                start + line.find(declaration).unwrap() + declaration.len(),
            ),
        })
    }

    fn parse_back(
        &mut self,
    ) -> Result<(Vec<BackStatement>, FormCompletionPolicy), (FormError, Span)> {
        let mut statements = Vec::new();
        let mut completion = FormCompletionPolicy::Live;
        while self.index < self.lines.len() {
            let line = self.lines[self.index];
            let (text, start) = line.statement();
            if text == "}" {
                self.index += 1;
                return Ok((statements, completion));
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
                if completion == FormCompletionPolicy::SemanticCompletion {
                    return Err((
                        FormError::InvalidSyntax(
                            "a form may contain only one semantic full stop".into(),
                        ),
                        self.span(start, start + text.len()),
                    ));
                }
                completion = FormCompletionPolicy::SemanticCompletion;
                self.index += 1;
                continue;
            }
            if text == "complete" {
                return Err((
                    FormError::InvalidSyntax(
                        "'complete' is not Conduitese; use a standalone '.' full stop".into(),
                    ),
                    self.span(start, start + text.len()),
                ));
            }
            if let Some(source) = text.strip_suffix("> ? {").map(str::trim) {
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
            let Some(split) = top_level_positions(text, '>').first().copied() else {
                return Err((
                    FormError::InvalidSyntax(
                        "matched routing track requires PATTERN > ROUTE".into(),
                    ),
                    self.line_span(line),
                ));
            };
            let pattern_text = text[..split].trim();
            let tail = text[split + 1..].trim();
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
                        expected: Expression {
                            text: expected.to_string(),
                            syntax,
                            span: self.span(expected_offset, expected_offset + expected.len()),
                        },
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
        if !top_level_positions(text, '>').is_empty() {
            return self.parse_cord(text, start).map(BackStatement::Cord);
        }
        if let Some(colon) = top_level_positions(text, ':').first().copied() {
            let name = text[..colon].trim();
            let invoked = text[colon + 1..].trim();
            if name.is_empty() || invoked.is_empty() {
                return Err((
                    FormError::InvalidSyntax("missing Gear Kind after ':'".into()),
                    self.span(start, start + text.len()),
                ));
            }
            let invocation = self.parse_invocation(invoked, start + text.find(invoked).unwrap())?;
            return Ok(BackStatement::NamedGear(NamedGear {
                name: self.spanned_at(name, text, start),
                invocation,
                span: self.span(start, start + text.len()),
            }));
        }
        if let Some(equal) = top_level_positions(text, '=').first().copied() {
            let name = text[..equal].trim();
            let value = text[equal + 1..].trim();
            if !is_name(name) || value.is_empty() {
                return Err(self.invalid_statement(text, start));
            }
            return Ok(BackStatement::LocalValue(LocalValue {
                name: self.spanned_at(name, text, start),
                value: self.expression_at(value, text, start)?,
                span: self.span(start, start + text.len()),
            }));
        }
        Err(self.invalid_statement(text, start))
    }

    fn parse_cord(&self, text: &str, start: usize) -> Result<Cord, (FormError, Span)> {
        let parts = split_top_level(text, '>');
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
        let parts = split_top_level(text, '>');
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
            if let Some(selector) = crate::structured_selector::parse(self.source, part, part_start)
            {
                let selector = selector
                    .map_err(|(message, span)| (FormError::InvalidSyntax(message), span))?;
                stages.push(CordStage::StructuredSelector(selector));
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
        if gear.is_empty() || !is_operation(gear) {
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
        if value.is_empty()
            || !delimiters_are_balanced(value)
            || !top_level_positions(value, '>').is_empty()
            || !top_level_positions(value, '=').is_empty()
        {
            return Err(self.invalid_statement(container, start));
        }
        let offset = start + container.find(value).unwrap();
        let syntax = crate::structured_expression::parse(self.source, value, offset)
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
