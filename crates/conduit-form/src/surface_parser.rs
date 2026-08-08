use crate::surface_lex::{
    delimiters_are_balanced, is_name, is_operation, is_reference, location, split_declaration,
    split_top_level, split_top_level_once, top_level_positions, SourceLine,
};
use crate::syntax::{
    Argument, BackStatement, Cord, CordStage, Expression, FormFace, FormSyntax, Invocation,
    LocalValue, NamedCell, RuntimePort, RuntimePortDirection, ShorthandPair, SpannedText,
    StartupParameter, SyntaxDocument,
};
use crate::{
    diagnostic, eof_span, tokenize_losslessly, FormError, Span, MAXIMUM_FORM_SOURCE_BYTES,
};

pub(crate) fn parse_surface(source: &str) -> SyntaxDocument {
    if source.len() > MAXIMUM_FORM_SOURCE_BYTES {
        return SyntaxDocument::new(
            String::new(),
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
                vec![diagnostic(FormError::TokenLimitExceeded, span)],
            );
        }
    };
    match Parser::new(source).parse_document() {
        Ok(forms) => SyntaxDocument::new(source.to_string(), tokens, forms, Vec::new()),
        Err((error, span)) => SyntaxDocument::new(
            source.to_string(),
            tokens,
            Vec::new(),
            vec![diagnostic(error, span)],
        ),
    }
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

    fn parse_document(mut self) -> Result<Vec<FormSyntax>, (FormError, Span)> {
        let mut forms = Vec::new();
        self.skip_empty();
        while self.index < self.lines.len() {
            forms.push(self.parse_form()?);
            self.skip_empty();
        }
        if forms.is_empty() {
            return Err((FormError::IncompleteForm, eof_span(self.source)));
        }
        Ok(forms)
    }

    fn parse_form(&mut self) -> Result<FormSyntax, (FormError, Span)> {
        let header_line = self.lines[self.index];
        let (header, header_start) = header_line.trimmed();
        let rest = header.strip_prefix("form ").ok_or_else(|| {
            (
                FormError::InvalidSyntax("expected 'form NAME' definition".into()),
                self.span(header_start, header_start + header.len()),
            )
        })?;
        let boundary = rest.find(['(', '{']).ok_or_else(|| {
            (
                FormError::InvalidSyntax("expected form face or back".into()),
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
        let mut face = FormFace::default();
        if marker == '(' {
            if !rest[boundary + 1..].trim().is_empty() {
                return Err((
                    FormError::InvalidSyntax(
                        "face declarations must follow '(' on their own lines".into(),
                    ),
                    self.line_span(header_line),
                ));
            }
            let open = header_start + "form ".len() + boundary;
            self.index += 1;
            face = self.parse_face(open)?;
        } else {
            if !rest[boundary..].trim().starts_with('{') || rest[boundary + 1..].trim() != "" {
                return Err((
                    FormError::InvalidSyntax("expected '{' to open form back".into()),
                    self.line_span(header_line),
                ));
            }
            self.index += 1;
        }
        let back = self.parse_back()?;
        let close = self.lines[self.index - 1];
        Ok(FormSyntax {
            name,
            face,
            back,
            span: self.span(form_start, close.start + close.text.len()),
        })
    }

    fn parse_face(&mut self, open: usize) -> Result<FormFace, (FormError, Span)> {
        let mut face = FormFace::default();
        while self.index < self.lines.len() {
            let line = self.lines[self.index];
            let (text, start) = line.trimmed();
            if text == ") {" || text == "){" {
                face.span = Some(self.span(open, start + text.find(')').unwrap() + 1));
                self.index += 1;
                return Ok(face);
            }
            if text.is_empty() || text.starts_with('#') {
                self.index += 1;
                continue;
            }
            if text.contains('>') {
                self.parse_face_runtime(text, start, &mut face)?;
            } else {
                face.startup_parameters
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

    fn parse_face_runtime(
        &self,
        text: &str,
        start: usize,
        face: &mut FormFace,
    ) -> Result<(), (FormError, Span)> {
        let arrows = top_level_positions(text, '>');
        if arrows.len() != 1 {
            return Err((
                FormError::InvalidSyntax("malformed face arrows".into()),
                self.span(start, start + text.len()),
            ));
        }
        let arrow = arrows[0];
        let left = text[..arrow].trim();
        let right = text[arrow + 1..].trim();
        match (left.is_empty(), right.is_empty()) {
            (true, false) => face.runtime_ports.push(self.runtime_port(
                right,
                text,
                start,
                RuntimePortDirection::Input,
            )?),
            (false, true) => face.runtime_ports.push(self.runtime_port(
                left,
                text,
                start,
                RuntimePortDirection::Output,
            )?),
            (false, false) => {
                if face.shorthand.is_some() {
                    return Err((
                        FormError::InvalidSyntax("more than one shorthand face pair".into()),
                        self.span(start, start + text.len()),
                    ));
                }
                let input = self.runtime_port(left, text, start, RuntimePortDirection::Input)?;
                let output = self.runtime_port(right, text, start, RuntimePortDirection::Output)?;
                face.shorthand = Some(ShorthandPair {
                    input_port: input.name.clone(),
                    output_port: output.name.clone(),
                    span: self.span(start, start + text.len()),
                });
                face.runtime_ports.extend([input, output]);
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
        Ok(RuntimePort {
            name: self.spanned_at(name, line, start),
            value_type: self.spanned_at(value_type, line, start),
            direction,
            span: self.span(
                start + line.find(declaration).unwrap(),
                start + line.find(declaration).unwrap() + declaration.len(),
            ),
        })
    }

    fn parse_back(&mut self) -> Result<Vec<BackStatement>, (FormError, Span)> {
        let mut statements = Vec::new();
        while self.index < self.lines.len() {
            let line = self.lines[self.index];
            let (text, start) = line.trimmed();
            if text == "}" {
                self.index += 1;
                return Ok(statements);
            }
            if text.is_empty() || text.starts_with('#') {
                self.index += 1;
                continue;
            }
            if text == "..." {
                self.index += 1;
                continue;
            }
            if !top_level_positions(text, '{').is_empty()
                || !top_level_positions(text, '}').is_empty()
            {
                return Err((
                    FormError::InvalidSyntax("a cell invocation cannot have a form back".into()),
                    self.line_span(line),
                ));
            }
            statements.push(self.parse_statement(text, start)?);
            self.index += 1;
        }
        Err((FormError::MissingBlockEnd, eof_span(self.source)))
    }

    fn parse_statement(
        &self,
        text: &str,
        start: usize,
    ) -> Result<BackStatement, (FormError, Span)> {
        if !top_level_positions(text, '>').is_empty() {
            return self.parse_cord(text, start).map(BackStatement::Cord);
        }
        if let Some(colon) = top_level_positions(text, ':').first().copied() {
            let name = text[..colon].trim();
            let invoked = text[colon + 1..].trim();
            if name.is_empty() || invoked.is_empty() {
                return Err((
                    FormError::InvalidSyntax("missing cell operation after ':'".into()),
                    self.span(start, start + text.len()),
                ));
            }
            let invocation = self.parse_invocation(invoked, start + text.find(invoked).unwrap())?;
            return Ok(BackStatement::NamedCell(NamedCell {
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
        let mut stages = Vec::new();
        let mut search = 0;
        for part in parts {
            let part = part.trim();
            let relative = text[search..].find(part).unwrap() + search;
            let part_start = start + relative;
            search = relative + part.len();
            if part.contains('/') || part.contains('(') {
                stages.push(CordStage::InlineCell(
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
        Ok(Cord {
            stages,
            span: self.span(start, start + text.len()),
        })
    }

    fn parse_invocation(&self, text: &str, start: usize) -> Result<Invocation, (FormError, Span)> {
        let (operation, arguments, end) = if let Some(open) = text.find('(') {
            if !text.ends_with(')') {
                return Err(self.invalid_statement(text, start));
            }
            (&text[..open], &text[open + 1..text.len() - 1], text.len())
        } else {
            (text, "", text.len())
        };
        let operation = operation.trim();
        if operation.is_empty() || !is_operation(operation) {
            return Err((
                FormError::InvalidSyntax("invalid operation reference".into()),
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
            operation: self.spanned(operation, start + text.find(operation).unwrap()),
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
            || !top_level_positions(value, '{').is_empty()
            || !top_level_positions(value, '}').is_empty()
        {
            return Err(self.invalid_statement(container, start));
        }
        let offset = start + container.find(value).unwrap();
        Ok(Expression {
            text: value.to_string(),
            span: self.span(offset, offset + value.len()),
        })
    }

    fn skip_empty(&mut self) {
        while self.index < self.lines.len() {
            let (text, _) = self.lines[self.index].trimmed();
            if !text.is_empty() && !text.starts_with('#') {
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
        let (text, start) = line.trimmed();
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
