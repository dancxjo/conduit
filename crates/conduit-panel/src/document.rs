use std::fmt::Write as _;

use sha2::{Digest as _, Sha256};

use crate::{
    ExportDirection, Panel, ParseError, PoolAdmission, PoolCleanup, PoolSupervision,
    PortGroupShape, SourcePressure, parse_with_root,
};

pub const SOURCE_AST_SCHEMA_VERSION: u16 = 0;

/// Exact source extent, using UTF-8 byte offsets and one-based locations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

/// Lossless concrete-source token class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CstTokenKind {
    Whitespace,
    Comment,
    Lexeme,
}

/// One lossless concrete-source token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CstToken {
    pub kind: CstTokenKind,
    pub span: Span,
    pub text: String,
}

/// Lossless source document plus its separate semantic AST and diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDocument {
    source: String,
    pub tokens: Vec<CstToken>,
    pub ast: Option<Panel>,
    pub diagnostics: Vec<ParseError>,
}

/// Bounded source facts recoverable without claiming a complete semantic AST.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveredDocument {
    pub nodes: Vec<RecoveredNode>,
    pub cords: Vec<RecoveredCord>,
    pub state: RecoveredDocumentState,
    pub recovery_limited: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveredDocumentState {
    Exact,
    Invalid,
    Partial,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveredNode {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub source_span: Span,
    pub id_span: Option<Span>,
    pub kind_span: Option<Span>,
    pub complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveredEndpoint {
    pub node: Option<String>,
    pub port: Option<String>,
    pub source_span: Option<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveredCord {
    pub id: String,
    pub from: RecoveredEndpoint,
    pub to: RecoveredEndpoint,
    pub source_span: Span,
    pub complete: bool,
}

impl SourceDocument {
    /// Returns the exact original UTF-8 bytes.
    #[must_use]
    pub fn round_trip(&self) -> &str {
        &self.source
    }

    /// Returns the parsed source AST when the document is well formed.
    pub fn panel(&self) -> Result<&Panel, &ParseError> {
        self.ast.as_ref().ok_or_else(|| &self.diagnostics[0])
    }

    /// Hashes normalized source semantics, excluding comments, trivia, and spans.
    #[must_use]
    pub fn semantic_hash(&self) -> Option<String> {
        self.ast.as_ref().map(semantic_source_hash)
    }
}

/// Parses a lossless document without selecting among multiple declared roots.
///
/// The returned CST is always available. On a malformed source, `ast` is
/// absent and `diagnostics` contains stable source diagnostics.
#[must_use]
pub fn parse_document(source: &str) -> SourceDocument {
    parse_document_with_root(source, None)
}

/// Parses a lossless document with an explicit root selection.
#[must_use]
pub fn parse_document_with_root(source: &str, selected_root: Option<&str>) -> SourceDocument {
    if source.len() > crate::MAXIMUM_PANEL_SOURCE_BYTES {
        return SourceDocument {
            source: source.to_owned(),
            tokens: Vec::new(),
            ast: None,
            diagnostics: vec![ParseError {
                code: "CND-SEC-001",
                line: 1,
                column: 1,
                message: "panel source byte limit exceeded".to_owned(),
            }],
        };
    }
    let tokens = lossless_tokens(source);
    match parse_with_root(source, selected_root) {
        Ok(panel) => SourceDocument {
            source: source.to_owned(),
            tokens,
            ast: Some(panel),
            diagnostics: Vec::new(),
        },
        Err(error) => SourceDocument {
            source: source.to_owned(),
            tokens,
            ast: None,
            diagnostics: vec![error],
        },
    }
}

/// Recovers bounded authored instance and graph-chain declarations for editor
/// presentation. Recovery never produces a [`Panel`] and is never executable.
#[must_use]
pub fn recover_document(source: &str) -> RecoveredDocument {
    if source.len() > crate::MAXIMUM_PANEL_SOURCE_BYTES {
        return RecoveredDocument {
            nodes: Vec::new(),
            cords: Vec::new(),
            state: RecoveredDocumentState::Partial,
            recovery_limited: true,
        };
    }
    let parsed = parse_document(source);
    let exact = parsed.ast.is_some();
    let mut nodes = Vec::new();
    let mut cords = Vec::new();
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let line_without_newline = line.strip_suffix('\n').unwrap_or(line);
        let content = line_without_newline.trim_start();
        let leading = line_without_newline.len() - content.len();
        if content.starts_with('#') {
            offset += line.len();
            continue;
        }
        if let Some(colon) = concise_declaration_colon(content) {
            if nodes.len() >= 1_024 {
                return RecoveredDocument {
                    nodes,
                    cords,
                    state: RecoveredDocumentState::Partial,
                    recovery_limited: true,
                };
            }
            let start = offset + leading;
            let end = declaration_end(source, start);
            let header_end = source[start..end]
                .find(['{', '\n'])
                .map_or(end, |relative| start + relative);
            let header = &source[start..header_end];
            let (id, id_span) = next_source_word(source, start, &header[..colon]);
            let kind_start = start + colon + 1;
            let (kind, kind_span) =
                next_source_word(source, kind_start, &source[kind_start..header_end]);
            let complete =
                id_span.is_some() && kind_span.is_some() && braces_complete(&source[start..end]);
            nodes.push(RecoveredNode {
                id,
                kind,
                source_span: span_for_bytes(source, start, end),
                id_span,
                kind_span,
                complete,
            });
        } else if let Some(operator) = concise_graph_operator(content) {
            if cords.len() >= 4_096 {
                return RecoveredDocument {
                    nodes,
                    cords,
                    state: RecoveredDocumentState::Partial,
                    recovery_limited: true,
                };
            }
            let start = offset + leading;
            let end = declaration_end(source, start);
            let header_end = source[start..end]
                .find(['{', '\n'])
                .map_or(end, |relative| start + relative);
            let header = source[start..header_end].trim();
            let header_offset = source[start..header_end]
                .find(header)
                .map_or(start, |relative| start + relative);
            let (from_text, to_text) = (&header[..operator], &header[operator + 1..]);
            let from_text = from_text.trim();
            let to_text = to_text.trim();
            let from_offset = header_offset + header.find(from_text).unwrap_or(0);
            let suffix = &header[operator + 1..];
            let to_offset = header_offset + operator + 1 + suffix.find(to_text).unwrap_or(0);
            let from = recovered_endpoint(source, from_text, from_offset);
            let to = recovered_endpoint(source, to_text, to_offset);
            let complete = from.node.is_some()
                && to.node.is_some()
                && !from_text.ends_with('.')
                && !to_text.ends_with('.')
                && braces_complete(&source[start..end]);
            cords.push(RecoveredCord {
                id: format!("cord-{}", cords.len()),
                from,
                to,
                source_span: span_for_bytes(source, start, end),
                complete,
            });
        }
        offset += line.len();
    }
    RecoveredDocument {
        nodes,
        cords,
        state: if exact {
            RecoveredDocumentState::Exact
        } else if parsed
            .diagnostics
            .first()
            .is_some_and(|diagnostic| diagnostic.line >= source.lines().count().saturating_sub(1))
        {
            RecoveredDocumentState::Partial
        } else {
            RecoveredDocumentState::Invalid
        },
        recovery_limited: false,
    }
}

fn concise_declaration_colon(source: &str) -> Option<usize> {
    if source.starts_with(['>', '<'])
        || [
            "panel ",
            "import ",
            "interface ",
            "root ",
            "export ",
            "bind ",
            "port-group ",
            "pool ",
            "supervise ",
        ]
        .iter()
        .any(|keyword| source.starts_with(keyword))
    {
        return None;
    }
    let colon = source.find(':')?;
    let equals = source.find('=').unwrap_or(usize::MAX);
    let greater = source.find('>').unwrap_or(usize::MAX);
    (colon < equals && colon < greater && !source[..colon].trim().is_empty()).then_some(colon)
}

fn concise_graph_operator(source: &str) -> Option<usize> {
    if source.starts_with(['>', '<'])
        || ["export ", "interface ", "port-group "]
            .iter()
            .any(|keyword| source.starts_with(keyword))
    {
        return None;
    }
    source.find('>')
}

fn next_source_word(source: &str, base: usize, fragment: &str) -> (Option<String>, Option<Span>) {
    let trimmed = fragment.trim_start();
    let leading = fragment.len() - trimmed.len();
    let word = trimmed
        .split(|character: char| character.is_whitespace() || matches!(character, ':' | '{' | '}'))
        .next()
        .unwrap_or("");
    if word.is_empty() {
        return (None, None);
    }
    let start = base + leading;
    let end = start + word.len();
    (
        Some(word.to_owned()),
        Some(span_for_bytes(source, start, end)),
    )
}

fn recovered_endpoint(source: &str, spelling: &str, start: usize) -> RecoveredEndpoint {
    let source_span =
        (!spelling.is_empty()).then(|| span_for_bytes(source, start, start + spelling.len()));
    let mut members = spelling.splitn(2, '.');
    let node = members
        .next()
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let port = members
        .next()
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    RecoveredEndpoint {
        node,
        port,
        source_span,
    }
}

fn declaration_end(source: &str, start: usize) -> usize {
    let tail = &source[start..];
    let Some(open) = tail.find('{') else {
        return tail
            .find('\n')
            .map_or(source.len(), |relative| start + relative);
    };
    let mut depth = 0_u32;
    let mut quoted = false;
    let mut escaped = false;
    for (relative, character) in tail[open..].char_indices() {
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
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return start + open + relative + character.len_utf8();
                }
            }
            _ => {}
        }
    }
    source.len()
}

fn braces_complete(source: &str) -> bool {
    !source.contains('{') || source.trim_end().ends_with('}')
}

fn span_for_bytes(source: &str, start: usize, end: usize) -> Span {
    let location = |byte: usize| {
        let prefix = &source[..byte.min(source.len())];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
        let column = prefix[line_start..].chars().count() + 1;
        (line, column)
    };
    let (line, column) = location(start);
    let (end_line, end_column) = location(end);
    Span {
        start,
        end,
        line,
        column,
        end_line,
        end_column,
    }
}

/// Stable source-semantic hash. This is distinct from resolved contract and
/// execution-plan identities.
#[must_use]
pub fn semantic_source_hash(panel: &Panel) -> String {
    let mut normalized = String::new();
    write_panel(panel, &mut normalized, false);
    format!(
        "sha256:{:x}",
        Sha256::digest([b"conduit.panel-source\0".as_slice(), normalized.as_bytes()].concat())
    )
}

pub(crate) fn lossless_tokens(source: &str) -> Vec<CstToken> {
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut line = 1;
    let mut column = 1;
    while index < source.len() {
        let start = index;
        let start_line = line;
        let start_column = column;
        let first = source[index..].chars().next().expect("valid char boundary");
        let kind = if first.is_whitespace() {
            while index < source.len() {
                let character = source[index..].chars().next().expect("valid char boundary");
                if !character.is_whitespace() {
                    break;
                }
                advance(character, &mut index, &mut line, &mut column);
            }
            CstTokenKind::Whitespace
        } else if first == '#' {
            while index < source.len() {
                let character = source[index..].chars().next().expect("valid char boundary");
                if character == '\n' {
                    break;
                }
                advance(character, &mut index, &mut line, &mut column);
            }
            CstTokenKind::Comment
        } else if first == '"' {
            advance(first, &mut index, &mut line, &mut column);
            let mut escaped = false;
            while index < source.len() {
                let character = source[index..].chars().next().expect("valid char boundary");
                advance(character, &mut index, &mut line, &mut column);
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    break;
                }
            }
            CstTokenKind::Lexeme
        } else {
            while index < source.len() {
                let character = source[index..].chars().next().expect("valid char boundary");
                if character.is_whitespace() || character == '#' {
                    break;
                }
                advance(character, &mut index, &mut line, &mut column);
            }
            CstTokenKind::Lexeme
        };
        tokens.push(CstToken {
            kind,
            span: Span {
                start,
                end: index,
                line: start_line,
                column: start_column,
                end_line: line,
                end_column: column,
            },
            text: source[start..index].to_owned(),
        });
    }
    tokens
}

fn advance(character: char, index: &mut usize, line: &mut usize, column: &mut usize) {
    *index += character.len_utf8();
    if character == '\n' {
        *line += 1;
        *column = 1;
    } else {
        *column += 1;
    }
}

fn write_panel(panel: &Panel, output: &mut String, include_selected_root: bool) {
    field(output, "version", panel.version);
    for import in &panel.imports {
        output.push_str("import{");
        text(output, &import.target);
        text(output, &import.alias);
        optional_text(output, import.content_hash.as_deref());
        output.push('}');
    }
    for import in &panel.package_imports {
        output.push_str("package-import{");
        text(output, &import.target);
        match &import.selection {
            crate::PackageImportSelection::Named(names) => {
                output.push_str("named{");
                for name in names {
                    text(output, &name.export);
                    text(output, &name.local);
                }
                output.push('}');
            }
            crate::PackageImportSelection::Alias { local, .. } => {
                output.push_str("alias{");
                text(output, local);
                output.push('}');
            }
        }
        output.push('}');
    }
    for interface in &panel.interfaces {
        write_interface(interface, output);
    }
    for definition in &panel.definitions {
        output.push_str("definition{");
        text(output, &definition.id);
        for parameter in &definition.parameters {
            output.push_str("parameter{");
            text(output, &parameter.id);
            text(output, &parameter.value_type);
            match &parameter.default {
                Some(value) => {
                    output.push_str("some");
                    write_source_value(value, output);
                }
                None => output.push_str("none;"),
            }
            output.push('}');
        }
        for claim in &definition.implements {
            output.push_str("implements{");
            text(output, &claim.interface);
            output.push('}');
        }
        for node in &definition.nodes {
            write_node(node, output);
        }
        for cord in &definition.cords {
            write_cord(cord, output);
        }
        for export in &definition.exports {
            output.push_str("export{");
            direction(output, export.direction);
            text(output, &export.id);
            endpoint(output, &export.target.node, &export.target.port);
            output.push('}');
        }
        for binding in &definition.bindings {
            output.push_str("binding{");
            text(output, &binding.parameter);
            endpoint(output, &binding.target.node, &binding.target.port);
            output.push('}');
        }
        for group in &definition.port_groups {
            write_group(group, output);
        }
        for pool in &definition.pools {
            write_pool(pool, output);
        }
        for supervision in &definition.supervisions {
            write_supervision(supervision, output);
        }
        output.push('}');
    }
    for node in &panel.nodes {
        write_node(node, output);
    }
    for cord in &panel.cords {
        write_cord(cord, output);
    }
    for root in &panel.roots {
        output.push_str("root{");
        text(output, &root.target);
        output.push('}');
    }
    if include_selected_root {
        optional_text(output, panel.selected_root.as_deref());
    }
    for group in &panel.port_groups {
        write_group(group, output);
    }
    for pool in &panel.pools {
        write_pool(pool, output);
    }
    for supervision in &panel.supervisions {
        write_supervision(supervision, output);
    }
}

fn write_interface(interface: &crate::InterfaceDeclaration, output: &mut String) {
    output.push_str("interface{");
    text(output, &interface.id);
    for member in &interface.members {
        output.push_str("member{");
        direction(output, member.direction);
        text(output, &member.id);
        text(output, &member.port_contract);
        field(output, "optional", member.optional);
        output.push('}');
    }
    output.push('}');
}

fn write_node(node: &crate::Node, output: &mut String) {
    output.push_str("node{");
    text(output, &node.id);
    text(output, &node.kind);
    optional_text(output, node.constraint.as_deref());
    for claim in &node.implements {
        output.push_str("implements{");
        text(output, &claim.interface);
        output.push('}');
    }
    for config in &node.config {
        output.push_str("config{");
        text(output, &config.key);
        write_source_value(&config.value, output);
        output.push('}');
    }
    match &node.expression {
        Some(expression) => {
            output.push_str("expression{");
            write_expression(expression, output);
            output.push('}');
        }
        None => output.push_str("no-expression;"),
    }
    output.push('}');
}

fn write_expression(expression: &crate::SourceExpression, output: &mut String) {
    match expression {
        crate::SourceExpression::Value(value) => {
            output.push_str("value{");
            write_source_value(value, output);
            output.push('}');
        }
        crate::SourceExpression::Binding(binding) => {
            output.push_str("binding{");
            text(output, binding);
            output.push('}');
        }
        crate::SourceExpression::Binary {
            operation,
            left,
            right,
            operator_span: _,
        } => {
            output.push_str("binary{");
            text(output, expression_operator_name(*operation));
            write_expression(left, output);
            write_expression(right, output);
            output.push('}');
        }
    }
}

fn expression_operator_name(operation: crate::ExpressionOperator) -> &'static str {
    match operation {
        crate::ExpressionOperator::Add => "add",
        crate::ExpressionOperator::Subtract => "subtract",
        crate::ExpressionOperator::Multiply => "multiply",
        crate::ExpressionOperator::Divide => "divide",
        crate::ExpressionOperator::LessThan => "less-than",
        crate::ExpressionOperator::LessThanOrEqual => "less-than-or-equal",
        crate::ExpressionOperator::GreaterThan => "greater-than",
        crate::ExpressionOperator::GreaterThanOrEqual => "greater-than-or-equal",
        crate::ExpressionOperator::Equal => "equal",
        crate::ExpressionOperator::NotEqual => "not-equal",
    }
}

fn write_source_value(value: &crate::SourceValue, output: &mut String) {
    match value {
        crate::SourceValue::Boolean(value) => field(output, "boolean", value),
        crate::SourceValue::Integer(value) => field(output, "integer", value),
        crate::SourceValue::Text(value) => {
            output.push_str("text");
            text(output, value);
        }
        crate::SourceValue::Bytes(value) => {
            output.push_str("bytes:");
            for byte in value {
                write!(output, "{byte:02x}").expect("write to String");
            }
            output.push(';');
        }
        crate::SourceValue::Reference(value) => {
            output.push_str("reference");
            text(output, value);
        }
        crate::SourceValue::ContractReference(value) => {
            output.push_str("contract");
            text(output, value);
        }
        crate::SourceValue::SecretReference(value) => {
            output.push_str("secret");
            text(output, value);
        }
        crate::SourceValue::ExactDecimal(value) => {
            output.push_str("decimal");
            text(output, value);
        }
        crate::SourceValue::List(values) => {
            output.push_str("list{");
            for value in values {
                write_source_value(value, output);
            }
            output.push('}');
        }
        crate::SourceValue::Record(fields) => {
            output.push_str("record{");
            let mut fields: Vec<_> = fields.iter().collect();
            fields.sort_by(|left, right| left.0.cmp(&right.0));
            for (key, value) in fields {
                text(output, key);
                write_source_value(value, output);
            }
            output.push('}');
        }
    }
}

fn write_cord(cord: &crate::Cord, output: &mut String) {
    output.push_str("cord{");
    text(output, &cord.id);
    endpoint(output, &cord.from.node, &cord.from.port);
    endpoint(output, &cord.to.node, &cord.to.port);
    field(output, "capacity", cord.capacity_items);
    field(output, "value-bytes", cord.max_value_bytes);
    field(output, "queued-bytes", cord.max_queued_bytes);
    field(output, "low", cord.low_watermark_items);
    field(output, "high", cord.high_watermark_items);
    match &cord.pressure {
        SourcePressure::Block => output.push_str("pressure:block;"),
        SourcePressure::Reject => output.push_str("pressure:reject;"),
        SourcePressure::Coalesce { relation } => {
            output.push_str("pressure:coalesce;");
            text(output, relation);
        }
        SourcePressure::Sample { every, offset } => {
            field(output, "sample-every", *every);
            field(output, "sample-offset", *offset);
        }
        SourcePressure::DropDisposable => output.push_str("pressure:drop-disposable;"),
        SourcePressure::Disconnect => output.push_str("pressure:disconnect;"),
        SourcePressure::Fail => output.push_str("pressure:fail;"),
    }
    output.push('}');
}

fn write_group(group: &crate::PortGroup, output: &mut String) {
    output.push_str("group{");
    text(output, &group.id);
    direction(output, group.direction);
    text(output, &group.port_contract);
    field(output, "maximum", group.maximum);
    match &group.shape {
        PortGroupShape::Keyed(members) => {
            output.push_str("keyed{");
            for member in members {
                text(output, &member.key);
            }
            output.push('}');
        }
        PortGroupShape::Indexed => output.push_str("indexed;"),
    }
    output.push('}');
}

fn write_pool(pool: &crate::InstancePool, output: &mut String) {
    output.push_str("pool{");
    text(output, &pool.id);
    text(output, &pool.template);
    field(output, "maximum", pool.maximum);
    match pool.admission {
        PoolAdmission::Reject => output.push_str("admission:reject;"),
        PoolAdmission::Block => output.push_str("admission:block;"),
        PoolAdmission::QueueBounded(capacity) => {
            field(output, "admission-queue", capacity);
        }
        PoolAdmission::Fail => output.push_str("admission:fail;"),
    }
    field(output, "deadline-ms", pool.deadline_ms);
    field(output, "idle-timeout-ms", pool.idle_timeout_ms);
    match &pool.supervision {
        PoolSupervision::FailTogether => output.push_str("supervision:fail-together;"),
        PoolSupervision::Isolate => output.push_str("supervision:isolate;"),
        PoolSupervision::RestartBounded {
            attempts,
            backoff_ms,
        } => {
            field(output, "restart-attempts", *attempts);
            field(output, "restart-backoff-ms", *backoff_ms);
        }
        PoolSupervision::Fallback(target) => {
            output.push_str("supervision:fallback;");
            text(output, target);
        }
        PoolSupervision::Escalate => output.push_str("supervision:escalate;"),
    }
    match pool.cleanup {
        PoolCleanup::Drain => output.push_str("cleanup:drain;"),
        PoolCleanup::Abort => output.push_str("cleanup:abort;"),
    }
    output.push('}');
}

fn write_supervision(supervision: &crate::SupervisionBinding, output: &mut String) {
    output.push_str("supervision{");
    text(output, &supervision.subject);
    text(output, &supervision.handler);
    output.push('}');
}

fn direction(output: &mut String, direction: ExportDirection) {
    output.push_str(match direction {
        ExportDirection::Input => "input;",
        ExportDirection::Output => "output;",
    });
}

fn endpoint(output: &mut String, node: &str, port: &str) {
    output.push_str("endpoint{");
    text(output, node);
    text(output, port);
    output.push('}');
}

fn optional_text(output: &mut String, value: Option<&str>) {
    match value {
        Some(value) => {
            output.push_str("some");
            text(output, value);
        }
        None => output.push_str("none;"),
    }
}

fn text(output: &mut String, value: &str) {
    write!(output, "{}:", value.len()).expect("write to String");
    output.push_str(value);
    output.push(';');
}

fn field(output: &mut String, name: &str, value: impl std::fmt::Display) {
    write!(output, "{name}:{value};").expect("write to String");
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    #[test]
    fn incomplete_typing_preserves_every_recoverable_authored_identity() {
        let prefix = "panel 0\nstable: std/literal { value = \"ok\" }\ngreeting :";
        for suffix in ["", " std/lit", " std/literal {", " std/literal {\n value ="] {
            let recovered = recover_document(&format!("{prefix}{suffix}"));
            assert!(
                recovered
                    .nodes
                    .iter()
                    .any(|node| node.id.as_deref() == Some("stable"))
            );
            assert!(
                recovered
                    .nodes
                    .iter()
                    .any(|node| node.id.as_deref() == Some("greeting"))
            );
            assert!(!recovered.recovery_limited);
        }
    }

    #[test]
    fn partial_cord_retains_only_authored_endpoint_facts() {
        let recovered = recover_document("panel 0\nsource: std/literal\nsource.value > sink.\n");
        assert_eq!(recovered.cords.len(), 1);
        assert_eq!(recovered.cords[0].from.node.as_deref(), Some("source"));
        assert_eq!(recovered.cords[0].from.port.as_deref(), Some("value"));
        assert_eq!(recovered.cords[0].to.node.as_deref(), Some("sink"));
        assert_eq!(recovered.cords[0].to.port, None);
        assert!(!recovered.cords[0].complete);
    }

    #[test]
    fn trailing_graph_operator_is_visible_but_never_semantically_executable() {
        let source = "panel 0\nsource: fixture/source\nsource >\n";
        let document = crate::parse_document(source);
        assert!(document.ast.is_none());
        assert_eq!(document.round_trip(), source);

        let recovered = recover_document(source);
        assert_eq!(recovered.cords.len(), 1);
        assert_eq!(recovered.cords[0].from.node.as_deref(), Some("source"));
        assert_eq!(recovered.cords[0].to.node, None);
        assert!(!recovered.cords[0].complete);
    }

    #[test]
    fn recovery_fails_closed_at_the_source_bound() {
        let source = "x".repeat(crate::MAXIMUM_PANEL_SOURCE_BYTES + 1);
        let recovered = recover_document(&source);
        assert!(recovered.recovery_limited);
        assert!(recovered.nodes.is_empty());
        assert!(recovered.cords.is_empty());
    }
}
