
use super::*;

pub fn parse_document(source: &str, catalog: &ProfileCatalog) -> FormDocument {
    if source.len() > MAXIMUM_FORM_SOURCE_BYTES {
        let error = FormError::SourceLimitExceeded;
        return FormDocument {
            source: String::new(),
            tokens: Vec::new(),
            checked_form: None,
            diagnostics: vec![diagnostic(error, whole_source_span(source))],
        };
    }

    let tokens = match tokenize_losslessly(source) {
        Ok(tokens) => tokens,
        Err(span) => {
            let error = FormError::TokenLimitExceeded;
            return FormDocument {
                source: source.to_string(),
                tokens: Vec::new(),
                checked_form: None,
                diagnostics: vec![diagnostic(error, span)],
            };
        }
    };
    match parse_checked_with_span(source, catalog) {
        Ok(checked_form) => FormDocument {
            source: source.to_string(),
            tokens,
            checked_form: Some(checked_form),
            diagnostics: Vec::new(),
        },
        Err((error, span)) => FormDocument {
            source: source.to_string(),
            tokens,
            checked_form: None,
            diagnostics: vec![diagnostic(error, span)],
        },
    }
}

pub fn parse(source: &str, catalog: &ProfileCatalog) -> Result<CheckedForm, FormError> {
    if source.len() > MAXIMUM_FORM_SOURCE_BYTES {
        return Err(FormError::SourceLimitExceeded);
    }
    parse_checked_with_span(source, catalog).map_err(|(error, _)| error)
}

#[derive(Debug, Clone, Copy)]
struct LocatedLine<'a> {
    text: &'a str,
    span: Span,
}

fn parse_checked_with_span(
    source: &str,
    catalog: &ProfileCatalog,
) -> Result<CheckedForm, (FormError, Span)> {
    let lines = significant_lines(source);
    let eof = eof_span(source);
    let first_span = lines.first().map_or(eof, |line| line.span);
    if lines.first().map_or("", |line| line.text) != "form 0" {
        return Err((FormError::InvalidHeader, first_span));
    }
    if lines.len() < 2 {
        return Err((FormError::IncompleteForm, eof));
    }
    let (form, next) = parse_form_block(source, &lines, 1, catalog, 0, Some(source))?;
    if next != lines.len() {
        return Err((
            FormError::InvalidStatement(lines[next].text.to_string()),
            lines[next].span,
        ));
    }
    Ok(form)
}

fn parse_form_block(
    source: &str,
    lines: &[LocatedLine<'_>],
    start: usize,
    catalog: &ProfileCatalog,
    depth: usize,
    identity_source: Option<&str>,
) -> Result<(CheckedForm, usize), (FormError, Span)> {
    let header = lines
        .get(start)
        .copied()
        .ok_or_else(|| (FormError::IncompleteForm, eof_span(source)))?;
    if depth > MAXIMUM_FORM_NESTING_DEPTH {
        return Err((FormError::NestingLimitExceeded, header.span));
    }
    if !header.text.ends_with('{') {
        return Err((FormError::InvalidBlockStart, header.span));
    }
    let declaration = header.text.trim_end_matches('{').trim();
    let name = if identity_source.is_some() {
        declaration
    } else {
        declaration
            .split_once(':')
            .map_or(declaration, |(name, _)| name)
    }
    .trim()
    .to_string();
    if name.is_empty() {
        return Err((FormError::EmptyFormName, header.span));
    }

    let mut operations = BTreeMap::<String, OperationDraft>::new();
    let mut connections = Vec::<CheckedConnection>::new();
    let mut exports = Vec::<CheckedExport>::new();
    let mut nested_forms = Vec::<CheckedNestedForm>::new();
    let mut index = start + 1;
    while index < lines.len() {
        let located = lines[index];
        let line = located.text;
        if line == "}" {
            nested_forms.sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
            let checked_operations = operations
                .iter()
                .map(|(operation_name, draft)| CheckedOperation {
                    operation_id: OperationId::from(operation_name.as_str()),
                    kind_id: draft.definition.kind_id.clone(),
                    kind_contract_revision: draft.definition.kind_contract_revision.clone(),
                    inputs: draft.definition.inputs.clone(),
                    outputs: draft.definition.outputs.clone(),
                    configuration: draft.configuration.clone(),
                })
                .collect::<Vec<_>>();
            let checked_form_id =
                crate::checking::checked_form_id(&name, &checked_operations, &connections, &exports);
            let checked_source =
                identity_source.unwrap_or_else(|| &source[header.span.start..located.span.end]);
            let source_document_id =
                SourceDocumentId::from(crate::checking::hash_string(&format!("source-document:{checked_source}")));
            let expanded_form_id = crate::checking::expanded_form_id(&checked_form_id, &nested_forms);
            return Ok((
                CheckedForm {
                    source_document_id,
                    checked_form_id,
                    expanded_form_id,
                    name,
                    operations: checked_operations,
                    connections,
                    exports,
                    nested_forms,
                },
                index + 1,
            ));
        }
        if line.starts_with("export ") && line.ends_with('{') {
            let (export, next) = parse_export_block(lines, index, &operations)
                .map_err(|error| (error, located.span))?;
            if exports
                .iter()
                .any(|checked| checked.capability_id == export.capability_id)
            {
                return Err((
                    FormError::InvalidExport(format!(
                        "duplicate capability '{}'",
                        export.capability_id.as_str()
                    )),
                    located.span,
                ));
            }
            exports.push(export);
            index = next;
            continue;
        }
        if line.ends_with('{') {
            let nested_declaration = line.trim_end_matches('{').trim();
            let (operation_name, capability_name) =
                nested_declaration.split_once(':').ok_or_else(|| {
                    (
                        FormError::InvalidNestedForm("expected 'operation: capability {'".into()),
                        located.span,
                    )
                })?;
            let operation_name = operation_name.trim();
            let capability_name = capability_name.trim();
            if operation_name.is_empty() || capability_name.is_empty() {
                return Err((FormError::InvalidBlockStart, located.span));
            }
            if operations.contains_key(operation_name) {
                return Err((
                    FormError::DuplicateOperation(operation_name.to_string()),
                    located.span,
                ));
            }
            let (nested_form, next) =
                parse_form_block(source, lines, index, catalog, depth + 1, None)?;
            let export_capability_id = CapabilityId::from(capability_name);
            let boundary = nested_form
                .export_boundary(&export_capability_id)
                .map_err(|error| (error, located.span))?;
            operations.insert(
                operation_name.to_string(),
                OperationDraft {
                    definition: boundary.kind_definition(),
                    configuration: Vec::new(),
                },
            );
            nested_forms.push(CheckedNestedForm {
                operation_id: OperationId::from(operation_name),
                export_capability_id,
                form: nested_form,
            });
            index = next;
            continue;
        }
        if line.starts_with("export ") {
            return Err((FormError::InvalidExport(line.to_string()), located.span));
        }
        if let Some((left, right)) = line.split_once(':') {
            let operation_id = left.trim().to_string();
            if operations.contains_key(&operation_id) {
                return Err((FormError::DuplicateOperation(operation_id), located.span));
            }
            operations.insert(
                operation_id,
                OperationDraft::new(right.trim(), catalog)
                    .map_err(|error| (error, located.span))?,
            );
            index += 1;
            continue;
        }
        if let Some((left, right)) = line.split_once('=') {
            let (operation_id, key) = left.trim().split_once('.').ok_or_else(|| {
                (
                    FormError::InvalidConfiguration(line.to_string()),
                    located.span,
                )
            })?;
            let operation = operations.get_mut(operation_id.trim()).ok_or_else(|| {
                (
                    FormError::UnknownOperation(operation_id.trim().to_string()),
                    located.span,
                )
            })?;
            let entry = operation
                .configuration
                .iter_mut()
                .find(|entry| entry.key == key.trim())
                .ok_or_else(|| {
                    (
                        FormError::InvalidConfiguration(format!(
                            "unsupported key '{}' for '{}'",
                            key.trim(),
                            operation.definition.kind_id.as_str()
                        )),
                        located.span,
                    )
                })?;
            let value = parse_configuration_value(right.trim(), &entry.value)
                .map_err(|error| (error, located.span))?;
            let field = operation
                .definition
                .configuration
                .iter()
                .find(|field| field.key == key.trim())
                .expect("configuration entry came from its catalog field");
            if !field.validation.accepts(&value) {
                return Err((
                    FormError::InvalidConfiguration(format!(
                        "value for '{}.{}' violates the profile catalog rule",
                        operation_id.trim(),
                        key.trim()
                    )),
                    located.span,
                ));
            }
            entry.value = value;
            index += 1;
            continue;
        }
        if let Some((left, right)) = line.split_once("->") {
            connections.push(
                parse_connection(left.trim(), right.trim(), &operations)
                    .map_err(|error| (error, located.span))?,
            );
            index += 1;
            continue;
        }
        if let Some((left, right)) = line.split_once('>') {
            connections.push(
                parse_shorthand_connection(left.trim(), right.trim(), &operations)
                    .map_err(|error| (error, located.span))?,
            );
            index += 1;
            continue;
        }
        return Err((FormError::InvalidStatement(line.to_string()), located.span));
    }
    Err((FormError::MissingBlockEnd, eof_span(source)))
}

fn significant_lines(source: &str) -> Vec<LocatedLine<'_>> {
    let mut lines = Vec::new();
    let mut offset = 0;
    for (line_index, raw) in source.split_inclusive('\n').enumerate() {
        let content = raw.strip_suffix('\n').unwrap_or(raw);
        let trimmed_start = content.trim_start();
        let leading = content.len() - trimmed_start.len();
        let leading_columns = content[..leading].chars().count();
        let text = trimmed_start.trim_end();
        if !text.is_empty() && !text.starts_with('#') {
            let start = offset + leading;
            let end = start + text.len();
            lines.push(LocatedLine {
                text,
                span: Span {
                    start,
                    end,
                    line: line_index + 1,
                    column: leading_columns + 1,
                    end_line: line_index + 1,
                    end_column: leading_columns + text.chars().count() + 1,
                },
            });
        }
        offset += raw.len();
    }
    lines
}

fn tokenize_losslessly(source: &str) -> Result<Vec<CstToken>, Span> {
    let mut tokens = Vec::new();
    let mut offset = 0;
    let mut line = 1;
    let mut column = 1;

    while offset < source.len() {
        let start = offset;
        let start_line = line;
        let start_column = column;
        let first = source[offset..]
            .chars()
            .next()
            .expect("offset is inside source");
        let kind;

        if first.is_whitespace() {
            kind = CstTokenKind::Whitespace;
            while offset < source.len() {
                let next = source[offset..]
                    .chars()
                    .next()
                    .expect("offset is inside source");
                if !next.is_whitespace() {
                    break;
                }
                advance(next, &mut offset, &mut line, &mut column);
            }
        } else if first == '#' {
            kind = CstTokenKind::Comment;
            while offset < source.len() {
                let next = source[offset..]
                    .chars()
                    .next()
                    .expect("offset is inside source");
                if next == '\n' {
                    break;
                }
                advance(next, &mut offset, &mut line, &mut column);
            }
        } else {
            kind = CstTokenKind::Lexeme;
            let quote = matches!(first, '\'' | '"').then_some(first);
            let mut escaped = false;
            while offset < source.len() {
                let next = source[offset..]
                    .chars()
                    .next()
                    .expect("offset is inside source");
                if quote.is_none() && (next.is_whitespace() || next == '#') {
                    break;
                }
                advance(next, &mut offset, &mut line, &mut column);
                if let Some(quote) = quote {
                    if next == quote && offset > start + next.len_utf8() && !escaped {
                        break;
                    }
                    escaped = next == '\\' && !escaped;
                }
            }
        }

        let span = Span {
            start,
            end: offset,
            line: start_line,
            column: start_column,
            end_line: line,
            end_column: column,
        };
        if tokens.len() == MAXIMUM_FORM_TOKENS {
            return Err(span);
        }
        tokens.push(CstToken {
            kind,
            span,
            text: source[start..offset].to_string(),
        });
    }
    Ok(tokens)
}

fn advance(character: char, offset: &mut usize, line: &mut usize, column: &mut usize) {
    *offset += character.len_utf8();
    if character == '\n' {
        *line += 1;
        *column = 1;
    } else {
        *column += 1;
    }
}

fn whole_source_span(source: &str) -> Span {
    let end = eof_span(source);
    Span {
        start: 0,
        end: source.len(),
        line: 1,
        column: 1,
        end_line: end.line,
        end_column: end.column,
    }
}

fn eof_span(source: &str) -> Span {
    let mut line = 1;
    let mut column = 1;
    for character in source.chars() {
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    Span {
        start: source.len(),
        end: source.len(),
        line,
        column,
        end_line: line,
        end_column: column,
    }
}

fn diagnostic(error: FormError, span: Span) -> FormDiagnostic {
    let code = match error {
        FormError::InvalidHeader => "CND-FRM-001",
        FormError::IncompleteForm => "CND-FRM-002",
        FormError::InvalidBlockStart => "CND-FRM-003",
        FormError::MissingBlockEnd => "CND-FRM-004",
        FormError::EmptyFormName => "CND-FRM-005",
        FormError::DuplicateKind(_) => "CND-FRM-006",
        FormError::DuplicateOperation(_) => "CND-FRM-007",
        FormError::UnknownOperation(_) => "CND-FRM-008",
        FormError::UnsupportedKind { .. } => "CND-FRM-009",
        FormError::InvalidConfiguration(_) => "CND-FRM-010",
        FormError::InvalidConnection(_) => "CND-FRM-011",
        FormError::InvalidExport(_) => "CND-FRM-012",
        FormError::InvalidStatement(_) => "CND-FRM-013",
        FormError::SourceLimitExceeded => "CND-FRM-014",
        FormError::TokenLimitExceeded => "CND-FRM-015",
        FormError::NestingLimitExceeded => "CND-FRM-016",
        FormError::InvalidNestedForm(_) => "CND-FRM-017",
        FormError::InvalidIdentity(_) => "CND-FRM-018",
    };
    FormDiagnostic {
        code,
        span,
        message: error.to_string(),
    }
}

fn parse_export_block(
    lines: &[LocatedLine<'_>],
    start: usize,
    operations: &BTreeMap<String, OperationDraft>,
) -> Result<(CheckedExport, usize), FormError> {
    let header = lines
        .get(start)
        .ok_or_else(|| FormError::InvalidExport("missing export header".into()))?;
    let declaration = header
        .text
        .strip_prefix("export ")
        .and_then(|value| value.strip_suffix('{'))
        .map(str::trim)
        .ok_or_else(|| FormError::InvalidExport(header.text.to_string()))?;
    let (capability_id, kind_id) = declaration
        .split_once(':')
        .ok_or_else(|| FormError::InvalidExport(declaration.to_string()))?;
    if capability_id.trim().is_empty() || kind_id.trim().is_empty() {
        return Err(FormError::InvalidExport(declaration.to_string()));
    }
    let mut input_faces = Vec::new();
    let mut output_faces = Vec::new();
    let mut index = start + 1;
    while let Some(line) = lines.get(index) {
        if line.text == "}" {
            let export = CheckedExport {
                capability_id: CapabilityId::from(capability_id.trim()),
                kind_id: KindId::from(kind_id.trim()),
                input_faces,
                output_faces,
            };
            validate_export_face_names(&export)?;
            return Ok((export, index + 1));
        }
        let face = parse_export_face(line.text, operations)?;
        match face.external_port.direction {
            PortDirection::Input => input_faces.push(face),
            PortDirection::Output => output_faces.push(face),
        }
        index += 1;
    }
    Err(FormError::MissingBlockEnd)
}

fn parse_export_face(
    source: &str,
    operations: &BTreeMap<String, OperationDraft>,
) -> Result<CheckedCompositeFace, FormError> {
    let (direction, body) = if let Some(body) = source.strip_prefix("input ") {
        (PortDirection::Input, body)
    } else if let Some(body) = source.strip_prefix("output ") {
        (PortDirection::Output, body)
    } else {
        return Err(FormError::InvalidExport(format!(
            "expected an input or output face in '{source}'"
        )));
    };
    let (mapping, terminal) = body
        .rsplit_once(" terminal ")
        .ok_or_else(|| FormError::InvalidExport(source.to_string()))?;
    if terminal.trim() != "independent" {
        return Err(FormError::InvalidExport(format!(
            "unsupported terminal contract '{}'",
            terminal.trim()
        )));
    }
    let (declaration, endpoint) = mapping
        .split_once('=')
        .ok_or_else(|| FormError::InvalidExport(source.to_string()))?;
    let (external_port_id, value_kind) = declaration
        .split_once(':')
        .ok_or_else(|| FormError::InvalidExport(source.to_string()))?;
    let external_port_id = external_port_id.trim();
    let value_kind = value_kind.trim();
    if external_port_id.is_empty() || value_kind.is_empty() {
        return Err(FormError::InvalidExport(source.to_string()));
    }
    let (operation_id, port_id) = parse_endpoint(endpoint.trim())?;
    let operation = operation(operations, operation_id.trim())?;
    let internal_port = match direction {
        PortDirection::Input => &operation.definition.inputs,
        PortDirection::Output => &operation.definition.outputs,
    }
    .iter()
    .find(|port| port.port_id.as_str() == port_id.trim())
    .ok_or_else(|| {
        FormError::InvalidExport(format!(
            "{} face endpoint '{}.{}' is not a checked {} port",
            match direction {
                PortDirection::Input => "input",
                PortDirection::Output => "output",
            },
            operation_id.trim(),
            port_id.trim(),
            match direction {
                PortDirection::Input => "input",
                PortDirection::Output => "output",
            }
        ))
    })?;
    if internal_port.value_kind.as_str() != value_kind {
        return Err(FormError::InvalidExport(format!(
            "face kind '{}' differs from endpoint kind '{}'",
            value_kind,
            internal_port.value_kind.as_str()
        )));
    }
    Ok(CheckedCompositeFace {
        external_port: PortDescriptor {
            port_id: PortId::from(external_port_id),
            value_kind: KindId::from(value_kind),
            direction,
        },
        internal_operation_id: OperationId::from(operation_id.trim()),
        internal_port_id: PortId::from(port_id.trim()),
        terminal: CompositeFaceTerminal::Independent,
    })
}

fn validate_export_face_names(export: &CheckedExport) -> Result<(), FormError> {
    let mut names = std::collections::BTreeSet::new();
    for face in export.input_faces.iter().chain(&export.output_faces) {
        if !names.insert(face.external_port.port_id.clone()) {
            return Err(FormError::InvalidExport(format!(
                "duplicate face name '{}'",
                face.external_port.port_id.as_str()
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_export_faces(
    export: &CheckedExport,
    operations: &[CheckedOperation],
) -> Result<(), FormError> {
    validate_export_face_names(export)?;
    for (direction, faces) in [
        (PortDirection::Input, &export.input_faces),
        (PortDirection::Output, &export.output_faces),
    ] {
        for face in faces {
            if face.external_port.direction != direction {
                return Err(FormError::InvalidExport(
                    "face direction differs from its export collection".into(),
                ));
            }
            let operation = operations
                .iter()
                .find(|operation| operation.operation_id == face.internal_operation_id)
                .ok_or_else(|| {
                    FormError::InvalidExport(format!(
                        "face '{}' names a missing internal operation",
                        face.external_port.port_id.as_str()
                    ))
                })?;
            let endpoint = match direction {
                PortDirection::Input => &operation.inputs,
                PortDirection::Output => &operation.outputs,
            }
            .iter()
            .find(|port| port.port_id == face.internal_port_id)
            .ok_or_else(|| {
                FormError::InvalidExport(format!(
                    "face '{}' names a missing or wrongly directed internal port",
                    face.external_port.port_id.as_str()
                ))
            })?;
            if endpoint.value_kind != face.external_port.value_kind {
                return Err(FormError::InvalidExport(format!(
                    "face '{}' value kind differs from its internal endpoint",
                    face.external_port.port_id.as_str()
                )));
            }
            if face.terminal != CompositeFaceTerminal::Independent {
                return Err(FormError::InvalidExport(format!(
                    "face '{}' has an unsupported terminal contract",
                    face.external_port.port_id.as_str()
                )));
            }
        }
    }
    Ok(())
}

fn parse_configuration_value(
    source: &str,
    expected: &ConfigurationValue,
) -> Result<ConfigurationValue, FormError> {
    match expected {
        ConfigurationValue::Bool(_) => match source {
            "true" => Ok(ConfigurationValue::Bool(true)),
            "false" => Ok(ConfigurationValue::Bool(false)),
            _ => Err(FormError::InvalidConfiguration(format!(
                "invalid boolean '{source}'"
            ))),
        },
        ConfigurationValue::U64(_) => source
            .parse()
            .map(ConfigurationValue::U64)
            .map_err(|_| FormError::InvalidConfiguration(format!("invalid integer '{source}'"))),
    }
}

fn parse_connection(
    left: &str,
    right: &str,
    operations: &BTreeMap<String, OperationDraft>,
) -> Result<CheckedConnection, FormError> {
    let (source_operation, source_port) = parse_endpoint(left)?;
    let (sink_operation, sink_port) = parse_endpoint(right)?;
    connection_from_ports(
        source_operation,
        source_port,
        sink_operation,
        sink_port,
        operations,
    )
}

fn parse_shorthand_connection(
    source_operation: &str,
    sink_operation: &str,
    operations: &BTreeMap<String, OperationDraft>,
) -> Result<CheckedConnection, FormError> {
    let source = operation(operations, source_operation)?;
    let sink = operation(operations, sink_operation)?;
    if source.definition.outputs.len() != 1 || sink.definition.inputs.len() != 1 {
        return Err(FormError::InvalidConnection(format!(
            "shorthand requires exactly one output and one input for '{source_operation} > {sink_operation}'"
        )));
    }
    connection_from_ports(
        source_operation,
        source.definition.outputs[0].port_id.as_str(),
        sink_operation,
        sink.definition.inputs[0].port_id.as_str(),
        operations,
    )
}

fn connection_from_ports(
    source_operation: &str,
    source_port: &str,
    sink_operation: &str,
    sink_port: &str,
    operations: &BTreeMap<String, OperationDraft>,
) -> Result<CheckedConnection, FormError> {
    let source = operation(operations, source_operation)?;
    let sink = operation(operations, sink_operation)?;
    let source_descriptor = source
        .definition
        .outputs
        .iter()
        .find(|port| port.port_id.as_str() == source_port)
        .ok_or_else(|| {
            FormError::InvalidConnection(format!(
                "'{source_operation}' has no output port '{source_port}'"
            ))
        })?;
    let sink_descriptor = sink
        .definition
        .inputs
        .iter()
        .find(|port| port.port_id.as_str() == sink_port)
        .ok_or_else(|| {
            FormError::InvalidConnection(format!(
                "'{sink_operation}' has no input port '{sink_port}'"
            ))
        })?;
    if source_descriptor.value_kind != sink_descriptor.value_kind {
        return Err(FormError::InvalidConnection(format!(
            "value kind '{}' cannot connect to '{}'",
            source_descriptor.value_kind.as_str(),
            sink_descriptor.value_kind.as_str()
        )));
    }
    Ok(CheckedConnection {
        source_operation_id: OperationId::from(source_operation),
        source_port_id: source_descriptor.port_id.clone(),
        sink_operation_id: OperationId::from(sink_operation),
        sink_port_id: sink_descriptor.port_id.clone(),
        value_kind: source_descriptor.value_kind.clone(),
    })
}

fn operation<'a>(
    operations: &'a BTreeMap<String, OperationDraft>,
    operation_id: &str,
) -> Result<&'a OperationDraft, FormError> {
    operations
        .get(operation_id)
        .ok_or_else(|| FormError::UnknownOperation(operation_id.to_string()))
}

fn parse_endpoint(endpoint: &str) -> Result<(&str, &str), FormError> {
    endpoint.split_once('.').ok_or_else(|| {
        FormError::InvalidConnection(format!("expected explicit port in '{endpoint}'"))
    })
}
