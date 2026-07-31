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
    output.push('}');
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
