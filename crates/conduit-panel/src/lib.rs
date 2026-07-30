//! Editable `.panel` source model and parser.
//!
//! Parsing produces source structures only. Module loading is explicit and
//! deterministic; implementation selection, host observation, planning,
//! execution, evidence, and presentation remain outside this crate.

use std::{collections::BTreeSet, fmt};

mod document;
mod modules;

pub use document::{
    CstToken, CstTokenKind, SOURCE_AST_SCHEMA_V1, SOURCE_AST_SCHEMA_V2, SOURCE_AST_SCHEMA_V3,
    SOURCE_AST_SCHEMA_V4, SourceDocument, SourceSchemaError, Span, parse_document,
    parse_document_with_root, semantic_source_hash, semantic_source_hash_v1,
    semantic_source_hash_v2, semantic_source_hash_v3, semantic_source_hash_v4,
    semantic_source_hash_version,
};
pub use modules::{
    LoadedModule, ModuleGraph, ModuleLoader, ModuleResolutionError, ResolvedImport, ResolvedModule,
    ResolvedRootSelection, RootSelectionMode, resolve_modules,
};

/// Portable ceiling applied before the parser retains attacker-controlled
/// source structures.
pub const MAXIMUM_PANEL_SOURCE_BYTES: usize = 4 * 1024 * 1024;
/// Maximum lexical items retained for one source document, including EOF.
pub const MAXIMUM_PANEL_TOKENS: usize = 262_144;
/// Maximum nesting of source value constructors such as `list(record(...))`.
pub const MAXIMUM_SOURCE_VALUE_DEPTH: u8 = 64;
/// Maximum named interfaces declared by one source module.
pub const MAXIMUM_INTERFACE_DECLARATIONS: usize = 256;
/// Maximum complete directional port members in one interface declaration.
pub const MAXIMUM_INTERFACE_MEMBERS: usize = 64;
/// Maximum explicit interface claims on one node boundary.
pub const MAXIMUM_INTERFACE_CLAIMS: usize = 32;

/// Parsed editable panel source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Panel {
    /// Source grammar major version.
    pub version: u16,
    /// Explicit module imports in source order.
    pub imports: Vec<Import>,
    /// Named node-interface declarations in source order.
    pub interfaces: Vec<InterfaceDeclaration>,
    /// Reusable composite node definitions in source order.
    pub definitions: Vec<CompositeDefinition>,
    /// Node instances.
    pub nodes: Vec<Node>,
    /// Cord declarations.
    pub cords: Vec<Cord>,
    /// Explicit selectable root definitions.
    pub roots: Vec<Root>,
    /// Root selected by the caller when the document declares alternatives.
    pub selected_root: Option<String>,
    /// Compile-time top-level port groups.
    pub port_groups: Vec<PortGroup>,
    /// Plan-visible top-level bounded instance pools.
    pub pools: Vec<InstancePool>,
    /// Explicit terminal-supervision bindings. Only grammar version 2 may
    /// author these; version 1 remains frozen.
    pub supervisions: Vec<SupervisionBinding>,
}

/// One deterministic source import. Parsing never fetches it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Import {
    /// Authored relative path or absolute URI.
    pub target: String,
    /// Local namespace alias.
    pub alias: String,
    /// Optional exact UTF-8 content digest.
    pub content_hash: Option<String>,
    /// Exact authored import extent.
    pub source_span: SourceSpan,
}

/// One explicitly selectable root definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Root {
    /// Definition or top-level instance selected as a root.
    pub target: String,
    /// Exact authored root target extent.
    pub source_span: SourceSpan,
}

/// One named source declaration of a finite node-interface boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceDeclaration {
    /// Stable namespaced interface identity.
    pub id: String,
    /// Complete directional port-contract members in authored order.
    pub members: Vec<InterfaceMemberDeclaration>,
    /// Exact authored declaration extent.
    pub source_span: SourceSpan,
}

/// One complete directional interface member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceMemberDeclaration {
    pub direction: ExportDirection,
    pub id: String,
    /// Exact complete PortContract reference; this is not a value-type alias.
    pub port_contract: String,
    /// Optional means absence is permitted. A present member remains complete.
    pub optional: bool,
    /// Exact authored member extent.
    pub source_span: SourceSpan,
    /// Exact authored PortContract reference extent.
    pub contract_span: SourceSpan,
}

/// One explicit claim that a concrete node boundary implements an interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceClaim {
    /// Local namespaced interface ID or `import-alias.interface/id`.
    pub interface: String,
    /// Exact authored reference extent.
    pub source_span: SourceSpan,
}

/// One reusable assemblage that remains an ordinary node at its boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositeDefinition {
    /// Stable semantic definition identity.
    pub id: String,
    /// Typed source parameters. They are not live ports.
    pub parameters: Vec<Parameter>,
    /// Explicit named-interface claims on this composite-derived boundary.
    pub implements: Vec<InterfaceClaim>,
    /// Child instances.
    pub nodes: Vec<Node>,
    /// Internal cords.
    pub cords: Vec<Cord>,
    /// Explicit boundary-to-child port mappings.
    pub exports: Vec<PortExport>,
    /// Explicit boundary-parameter-to-child-config mappings.
    pub bindings: Vec<ConfigBinding>,
    /// Compile-time port groups owned by this definition.
    pub port_groups: Vec<PortGroup>,
    /// Finite pools of this definition's child templates.
    pub pools: Vec<InstancePool>,
    /// Explicit supervision bindings between children in this definition.
    pub supervisions: Vec<SupervisionBinding>,
    /// Exact authored composite extent.
    pub source_span: SourceSpan,
}

/// One typed composite source parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub id: String,
    pub value_type: String,
    pub default: Option<SourceValue>,
    /// Exact authored parameter extent.
    pub source_span: SourceSpan,
    /// Exact authored default extent, absent when the parameter has no default.
    pub default_span: Option<SourceSpan>,
}

/// Direction of one explicitly exported boundary port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportDirection {
    Input,
    Output,
}

/// One transparent composite boundary mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortExport {
    pub direction: ExportDirection,
    pub id: String,
    pub target: Endpoint,
    pub source_span: SourceSpan,
}

/// One composite configuration parameter binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigBinding {
    pub parameter: String,
    pub target: Endpoint,
    pub source_span: SourceSpan,
}

/// One semantic node instance in source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    /// Stable local node ID.
    pub id: String,
    /// Semantic node-contract identity.
    pub kind: String,
    /// Unresolved implementation/capability constraint such as `ready`.
    pub constraint: Option<String>,
    /// Exact constraint token extent, when authored.
    pub constraint_span: Option<SourceSpan>,
    /// Explicit claims on the exact referenced primitive or composite contract.
    pub implements: Vec<InterfaceClaim>,
    /// Source configuration entries.
    pub config: Vec<ConfigEntry>,
    /// Exact authored instance extent.
    pub source_span: SourceSpan,
}

/// Shape of a compile-time group of ordinary ports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortGroupShape {
    /// Explicit stable keys in source order.
    Keyed(Vec<PortGroupMember>),
    /// Stable indices `0..maximum`.
    Indexed,
}

/// One explicitly authored keyed port-group member.
///
/// The span is annotation/provenance. It does not participate in source
/// semantic identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortGroupMember {
    pub key: String,
    pub source_span: SourceSpan,
}

/// One finite compile-time port group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortGroup {
    pub id: String,
    pub direction: ExportDirection,
    /// Complete semantic `PortContract` identity applied to every member.
    pub port_contract: String,
    pub maximum: u16,
    pub shape: PortGroupShape,
    pub source_span: SourceSpan,
}

/// Admission policy for a finite instance pool.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PoolAdmission {
    Reject,
    Block,
    QueueBounded(u16),
    Fail,
}

/// Cleanup policy for a finite instance pool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolCleanup {
    Drain,
    Abort,
}

/// Supervision policy for child attempts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PoolSupervision {
    FailTogether,
    Isolate,
    RestartBounded { attempts: u16, backoff_ms: u64 },
    Fallback(String),
    Escalate,
}

/// One source-level, finitely bounded composite instance pool.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstancePool {
    pub id: String,
    pub template: String,
    pub maximum: u16,
    pub admission: PoolAdmission,
    pub deadline_ms: u64,
    pub idle_timeout_ms: u64,
    pub supervision: PoolSupervision,
    pub cleanup: PoolCleanup,
    pub source_span: SourceSpan,
}

/// One explicit binding from a semantic subject to an ordinary handler node.
///
/// Bounds and admitted decisions are resolved into the exact plan; source does
/// not create a hidden callback, handler registry, or universal error port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupervisionBinding {
    pub subject: String,
    pub handler: String,
    /// Planner-supplied source-binding identity annotation. Parsing leaves it
    /// absent and source semantic identity deliberately ignores it.
    pub resolved_identity: Option<String>,
    pub source_span: SourceSpan,
}

impl Node {
    /// Returns one exact unresolved source value.
    #[must_use]
    pub fn config_value(&self, key: &str) -> Option<&SourceValue> {
        self.config
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| &entry.value)
    }

    /// Returns one configuration value.
    #[must_use]
    pub fn config(&self, key: &str) -> Option<&str> {
        self.config_value(key).and_then(SourceValue::public_text)
    }
}

/// Exact source literal. Parsing never resolves references or secrets.
#[derive(Clone, Eq, PartialEq)]
pub enum SourceValue {
    Boolean(bool),
    Integer(i128),
    Text(String),
    Bytes(Vec<u8>),
    Reference(String),
    ContractReference(String),
    SecretReference(String),
    ExactDecimal(String),
    List(Vec<SourceValue>),
    Record(Vec<(String, SourceValue)>),
}

impl SourceValue {
    /// Returns directly authored public text without resolving references.
    #[must_use]
    pub fn public_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) | Self::Reference(value) | Self::ContractReference(value) => {
                Some(value)
            }
            Self::Boolean(_)
            | Self::Integer(_)
            | Self::Bytes(_)
            | Self::SecretReference(_)
            | Self::ExactDecimal(_)
            | Self::List(_)
            | Self::Record(_) => None,
        }
    }
}

impl fmt::Debug for SourceValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boolean(value) => formatter.debug_tuple("Boolean").field(value).finish(),
            Self::Integer(value) => formatter.debug_tuple("Integer").field(value).finish(),
            Self::Text(value) => formatter.debug_tuple("Text").field(value).finish(),
            Self::Bytes(value) => formatter.debug_tuple("Bytes").field(value).finish(),
            Self::Reference(value) => formatter.debug_tuple("Reference").field(value).finish(),
            Self::ContractReference(value) => formatter
                .debug_tuple("ContractReference")
                .field(value)
                .finish(),
            Self::SecretReference(_) => formatter.write_str("SecretReference([REDACTED])"),
            Self::ExactDecimal(value) => {
                formatter.debug_tuple("ExactDecimal").field(value).finish()
            }
            Self::List(values) => formatter.debug_tuple("List").field(values).finish(),
            Self::Record(fields) => formatter.debug_tuple("Record").field(fields).finish(),
        }
    }
}

/// One source configuration key/value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigEntry {
    /// Configuration field name.
    pub key: String,
    /// Typed source literal, still unresolved.
    pub value: SourceValue,
    /// Exact authored value extent.
    pub source_span: SourceSpan,
}

/// One-based source extent retained on semantic source structures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

/// An unresolved source endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Endpoint {
    /// Local node ID.
    pub node: String,
    /// Semantic port ID.
    pub port: String,
}

/// One source cord declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cord {
    /// Stable generated source ID.
    pub id: String,
    /// Output endpoint.
    pub from: Endpoint,
    /// Input endpoint.
    pub to: Endpoint,
    /// Finite item capacity.
    pub capacity_items: u16,
    /// Maximum accounted bytes for one value.
    pub max_value_bytes: u32,
    /// Maximum accounted resident bytes.
    pub max_queued_bytes: u64,
    /// Pressure clearance threshold.
    pub low_watermark_items: u16,
    /// Pressure entry threshold.
    pub high_watermark_items: u16,
    /// Exact pressure response.
    pub pressure: SourcePressure,
    /// Exact authored cord extent.
    pub source_span: SourceSpan,
}

/// Authored exact pressure policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourcePressure {
    /// FIFO producer blocking.
    Block,
    /// Reject the attempted write.
    Reject,
    /// Use one named domain replacement relation.
    Coalesce {
        /// Exact relation identifier.
        relation: String,
    },
    /// Use one exact arrival-sequence schedule.
    Sample {
        /// Sampling period.
        every: u32,
        /// Selected offset within the period.
        offset: u32,
    },
    /// Drop only values proven disposable.
    DropDisposable,
    /// Disconnect the cord.
    Disconnect,
    /// Fail the affected run scope.
    Fail,
}

impl fmt::Display for SourcePressure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Block => formatter.write_str("block(fifo)"),
            Self::Reject => formatter.write_str("reject"),
            Self::Coalesce { relation } => write!(formatter, "coalesce({relation})"),
            Self::Sample { every, offset } => {
                write!(formatter, "sample(every={every},offset={offset})")
            }
            Self::DropDisposable => formatter.write_str("drop-disposable"),
            Self::Disconnect => formatter.write_str("disconnect"),
            Self::Fail => formatter.write_str("fail"),
        }
    }
}

/// Source parser failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    /// Stable source diagnostic.
    pub code: &'static str,
    /// One-based line.
    pub line: usize,
    /// One-based column.
    pub column: usize,
    /// Human-readable detail.
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {}:{}: {}",
            self.code, self.line, self.column, self.message
        )
    }
}

impl std::error::Error for ParseError {}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TokenKind {
    Word(String),
    String(String),
    Number(i128),
    Colon,
    Comma,
    Equals,
    LeftBrace,
    RightBrace,
    LeftParen,
    RightParen,
    Arrow,
    Eof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Token {
    kind: TokenKind,
    line: usize,
    column: usize,
    end_line: usize,
    end_column: usize,
}

/// Parses normative `.panel` grammar version 1.
///
/// ```text
/// panel 1
///
/// node greeting : conduit/literal {
///     value = "Hello from Conduit."
/// }
/// node output : conduit/stdout
///
/// cord greeting.out -> output.in {
///     capacity = 8
///     pressure = block
/// }
/// ```
pub fn parse(source: &str) -> Result<Panel, ParseError> {
    parse_with_root(source, None)
}

/// Parses a document and explicitly selects one of its declared roots.
pub fn parse_with_root(source: &str, selected_root: Option<&str>) -> Result<Panel, ParseError> {
    Parser::new(lex(source)?).parse(selected_root, true)
}

fn parse_module(source: &str) -> Result<Panel, ParseError> {
    Parser::new(lex(source)?).parse(None, false)
}

fn lex(source: &str) -> Result<Vec<Token>, ParseError> {
    if source.len() > MAXIMUM_PANEL_SOURCE_BYTES {
        return Err(ParseError {
            code: "CND-SEC-001",
            line: 1,
            column: 1,
            message: "panel source byte limit exceeded".to_owned(),
        });
    }
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut line = 1;
    let mut column = 1;

    while index < bytes.len() {
        if tokens.len() >= MAXIMUM_PANEL_TOKENS.saturating_sub(1) {
            return Err(ParseError {
                code: "CND-SEC-001",
                line,
                column,
                message: "panel source token limit exceeded".to_owned(),
            });
        }
        let byte = bytes[index];
        match byte {
            b' ' | b'\t' | b'\r' => {
                index += 1;
                column += 1;
            }
            b'\n' => {
                index += 1;
                line += 1;
                column = 1;
            }
            b'#' => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                    column += 1;
                }
            }
            b':' => {
                tokens.push(Token {
                    kind: TokenKind::Colon,
                    line,
                    column,
                    end_line: line,
                    end_column: column + 1,
                });
                index += 1;
                column += 1;
            }
            b',' => {
                tokens.push(Token {
                    kind: TokenKind::Comma,
                    line,
                    column,
                    end_line: line,
                    end_column: column + 1,
                });
                index += 1;
                column += 1;
            }
            b'=' => {
                tokens.push(Token {
                    kind: TokenKind::Equals,
                    line,
                    column,
                    end_line: line,
                    end_column: column + 1,
                });
                index += 1;
                column += 1;
            }
            b'{' => {
                tokens.push(Token {
                    kind: TokenKind::LeftBrace,
                    line,
                    column,
                    end_line: line,
                    end_column: column + 1,
                });
                index += 1;
                column += 1;
            }
            b'}' => {
                tokens.push(Token {
                    kind: TokenKind::RightBrace,
                    line,
                    column,
                    end_line: line,
                    end_column: column + 1,
                });
                index += 1;
                column += 1;
            }
            b'(' => {
                tokens.push(Token {
                    kind: TokenKind::LeftParen,
                    line,
                    column,
                    end_line: line,
                    end_column: column + 1,
                });
                index += 1;
                column += 1;
            }
            b')' => {
                tokens.push(Token {
                    kind: TokenKind::RightParen,
                    line,
                    column,
                    end_line: line,
                    end_column: column + 1,
                });
                index += 1;
                column += 1;
            }
            b'-' if bytes.get(index + 1) == Some(&b'>') => {
                tokens.push(Token {
                    kind: TokenKind::Arrow,
                    line,
                    column,
                    end_line: line,
                    end_column: column + 2,
                });
                index += 2;
                column += 2;
            }
            b'-' if bytes.get(index + 1).is_some_and(u8::is_ascii_digit) => {
                let start = index;
                let start_column = column;
                index += 1;
                column += 1;
                while bytes
                    .get(index)
                    .is_some_and(|candidate| candidate.is_ascii_digit())
                {
                    index += 1;
                    column += 1;
                }
                let text = &source[start..index];
                let value = text.parse::<i128>().map_err(|error| ParseError {
                    code: "CND-SRC-001",
                    line,
                    column: start_column,
                    message: format!("invalid integer: {error}"),
                })?;
                tokens.push(Token {
                    kind: TokenKind::Number(value),
                    line,
                    column: start_column,
                    end_line: line,
                    end_column: column,
                });
            }
            b'"' => {
                let start_line = line;
                let start_column = column;
                index += 1;
                column += 1;
                let mut value = String::new();
                let mut closed = false;
                while index < bytes.len() {
                    match bytes[index] {
                        b'"' => {
                            index += 1;
                            column += 1;
                            closed = true;
                            break;
                        }
                        b'\\' => {
                            let Some(escaped) = bytes.get(index + 1).copied() else {
                                break;
                            };
                            let character = match escaped {
                                b'n' => '\n',
                                b'r' => '\r',
                                b't' => '\t',
                                b'"' => '"',
                                b'\\' => '\\',
                                _ => {
                                    return Err(ParseError {
                                        code: "CND-SRC-001",
                                        line,
                                        column,
                                        message: format!(
                                            "unsupported escape sequence \\{}",
                                            char::from(escaped)
                                        ),
                                    });
                                }
                            };
                            value.push(character);
                            index += 2;
                            column += 2;
                        }
                        b'\n' => {
                            value.push('\n');
                            index += 1;
                            line += 1;
                            column = 1;
                        }
                        _ => {
                            let remaining = &source[index..];
                            let Some(character) = remaining.chars().next() else {
                                break;
                            };
                            value.push(character);
                            index += character.len_utf8();
                            column += 1;
                        }
                    }
                }
                if !closed {
                    return Err(ParseError {
                        code: "CND-SRC-001",
                        line: start_line,
                        column: start_column,
                        message: "unterminated string".to_owned(),
                    });
                }
                tokens.push(Token {
                    kind: TokenKind::String(value),
                    line: start_line,
                    column: start_column,
                    end_line: line,
                    end_column: column,
                });
            }
            b'0'..=b'9' => {
                let start = index;
                let start_column = column;
                while bytes
                    .get(index)
                    .is_some_and(|candidate| candidate.is_ascii_digit())
                {
                    index += 1;
                    column += 1;
                }
                let text = &source[start..index];
                let value = text.parse::<i128>().map_err(|error| ParseError {
                    code: "CND-SRC-001",
                    line,
                    column: start_column,
                    message: format!("invalid integer: {error}"),
                })?;
                tokens.push(Token {
                    kind: TokenKind::Number(value),
                    line,
                    column: start_column,
                    end_line: line,
                    end_column: column,
                });
            }
            _ if is_word_start_byte(byte) => {
                let start = index;
                let start_column = column;
                while bytes
                    .get(index)
                    .is_some_and(|candidate| is_word_byte(*candidate))
                {
                    index += 1;
                    column += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::Word(source[start..index].to_owned()),
                    line,
                    column: start_column,
                    end_line: line,
                    end_column: column,
                });
            }
            _ => {
                return Err(ParseError {
                    code: "CND-SRC-001",
                    line,
                    column,
                    message: format!("unexpected character {:?}", char::from(byte)),
                });
            }
        }
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        line,
        column,
        end_line: line,
        end_column: column,
    });
    Ok(tokens)
}

const fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/' | b'@' | b'[' | b']')
}

const fn is_word_start_byte(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'-' | b'.' | b'/' | b'@')
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    source_value_depth: u8,
    panel_version: u16,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            index: 0,
            source_value_depth: 0,
            panel_version: 0,
        }
    }

    fn parse(
        mut self,
        selected_root: Option<&str>,
        require_root_selection: bool,
    ) -> Result<Panel, ParseError> {
        self.expect_word("panel")?;
        let version = self.expect_number()?;
        let version =
            u16::try_from(version).map_err(|_| self.error("panel version does not fit in u16"))?;
        if !matches!(version, 1 | 2) {
            return Err(self.error_code(
                "CND-SRC-007",
                format!("unsupported panel version {version}"),
            ));
        }
        self.panel_version = version;

        let mut imports = Vec::new();
        let mut interfaces = Vec::new();
        let mut definitions = Vec::new();
        let mut nodes = Vec::new();
        let mut cords = Vec::new();
        let mut roots = Vec::new();
        let mut port_groups = Vec::new();
        let mut pools = Vec::new();
        let mut supervisions = Vec::new();
        while !matches!(self.current().kind, TokenKind::Eof) {
            let declaration = self.expect_any_word()?;
            match declaration.as_str() {
                "import" => imports.push(self.parse_import()?),
                "interface" => interfaces.push(self.parse_interface()?),
                "node" => {
                    let start_line = self.current().line;
                    let start_column = self.current().column;
                    let id = self.expect_any_word()?;
                    if matches!(self.current().kind, TokenKind::Colon) {
                        nodes.push(self.parse_node_after_id(id, start_line, start_column)?);
                    } else {
                        definitions.push(self.parse_definition_after_id(
                            id,
                            start_line,
                            start_column,
                        )?);
                    }
                }
                "composite" => {
                    let start_line = self.current().line;
                    let start_column = self.current().column;
                    let id = self.expect_any_word()?;
                    definitions.push(self.parse_definition_after_id(
                        id,
                        start_line,
                        start_column,
                    )?);
                }
                "cord" => {
                    let ordinal = cords.len();
                    cords.push(self.parse_cord(ordinal)?);
                }
                "root" => {
                    let start_line = self.current().line;
                    let start_column = self.current().column;
                    let target = self.expect_any_word()?;
                    let (end_line, end_column) = self.previous_end();
                    roots.push(Root {
                        target,
                        source_span: SourceSpan {
                            line: start_line,
                            column: start_column,
                            end_line,
                            end_column,
                        },
                    });
                }
                "port-group" => port_groups.push(self.parse_port_group()?),
                "pool" => pools.push(self.parse_pool()?),
                "supervise" => supervisions.push(self.parse_supervision()?),
                _ => {
                    return Err(self.error(format!(
                        "expected import, interface, node, composite, cord, root, port-group, pool, or supervise; found `{declaration}`"
                    )));
                }
            }
        }

        let selected_root = match (roots.len(), selected_root) {
            (0, None) => None,
            (0, Some(selected)) => {
                return Err(self.error_code(
                    "CND-SRC-006",
                    format!("selected root `{selected}` is not declared"),
                ));
            }
            (1, None) => Some(roots[0].target.clone()),
            (_, None) if require_root_selection => {
                return Err(
                    self.error_code("CND-SRC-006", "multiple roots require explicit selection")
                );
            }
            (_, None) => None,
            (_, Some(selected)) if roots.iter().any(|root| root.target == selected) => {
                Some(selected.to_owned())
            }
            (_, Some(selected)) => {
                return Err(self.error_code(
                    "CND-SRC-006",
                    format!("selected root `{selected}` is not declared"),
                ));
            }
        };

        let panel = Panel {
            version,
            imports,
            interfaces,
            definitions,
            nodes,
            cords,
            roots,
            selected_root,
            port_groups,
            pools,
            supervisions,
        };
        validate_source_symbols(panel, self.current().line, self.current().column)
    }

    fn parse_import(&mut self) -> Result<Import, ParseError> {
        let start_line = self.current().line;
        let start_column = self.current().column;
        let target = self.expect_string()?;
        self.expect_word("as")?;
        let alias = self.expect_any_word()?;
        let content_hash = if self.current_word_is("pin") {
            self.advance();
            Some(self.expect_string()?)
        } else {
            None
        };
        let (end_line, end_column) = self.previous_end();
        Ok(Import {
            target,
            alias,
            content_hash,
            source_span: SourceSpan {
                line: start_line,
                column: start_column,
                end_line,
                end_column,
            },
        })
    }

    fn parse_definition_after_id(
        &mut self,
        id: String,
        start_line: usize,
        start_column: usize,
    ) -> Result<CompositeDefinition, ParseError> {
        let parameters = if matches!(self.current().kind, TokenKind::LeftParen) {
            self.parse_parameters()?
        } else {
            Vec::new()
        };
        let implements = if self.current_word_is("implements") {
            self.parse_implements()?
        } else {
            Vec::new()
        };
        self.expect_simple(TokenKind::LeftBrace, "`{`")?;
        let mut nodes = Vec::new();
        let mut cords = Vec::new();
        let mut exports = Vec::new();
        let mut bindings = Vec::new();
        let mut port_groups = Vec::new();
        let mut pools = Vec::new();
        let mut supervisions = Vec::new();
        while !matches!(self.current().kind, TokenKind::RightBrace) {
            if matches!(self.current().kind, TokenKind::Eof) {
                return Err(self.error("unterminated node definition"));
            }
            let declaration = self.expect_any_word()?;
            match declaration.as_str() {
                "node" => nodes.push(self.parse_node()?),
                "cord" => {
                    let ordinal = cords.len();
                    cords.push(self.parse_cord(ordinal)?);
                }
                "export" => {
                    let start_line = self.current().line;
                    let start_column = self.current().column;
                    let direction = match self.expect_any_word()?.as_str() {
                        "input" => ExportDirection::Input,
                        "output" => ExportDirection::Output,
                        _ => return Err(self.error("export direction must be `input` or `output`")),
                    };
                    let first = self.expect_any_word()?;
                    let (export_id, target) = if self.current_word_is("as") {
                        self.advance();
                        let export_id = self.expect_any_word()?;
                        (export_id, self.endpoint_from_word(first)?)
                    } else {
                        self.expect_simple(TokenKind::Equals, "`=`")?;
                        (first, self.expect_endpoint()?)
                    };
                    let (end_line, end_column) = self.previous_end();
                    exports.push(PortExport {
                        direction,
                        id: export_id,
                        target,
                        source_span: SourceSpan {
                            line: start_line,
                            column: start_column,
                            end_line,
                            end_column,
                        },
                    });
                }
                "bind" => {
                    let start_line = self.current().line;
                    let start_column = self.current().column;
                    let parameter = self.expect_any_word()?;
                    self.expect_simple(TokenKind::Equals, "`=`")?;
                    let target = self.expect_endpoint()?;
                    let (end_line, end_column) = self.previous_end();
                    bindings.push(ConfigBinding {
                        parameter,
                        target,
                        source_span: SourceSpan {
                            line: start_line,
                            column: start_column,
                            end_line,
                            end_column,
                        },
                    });
                }
                "port-group" => port_groups.push(self.parse_port_group()?),
                "pool" => pools.push(self.parse_pool()?),
                "supervise" => supervisions.push(self.parse_supervision()?),
                _ => {
                    return Err(self.error(format!(
                        "expected child, cord, export, binding, port-group, pool, or supervise; found `{declaration}`"
                    )));
                }
            }
        }
        self.advance();
        let (end_line, end_column) = self.previous_end();
        Ok(CompositeDefinition {
            id,
            parameters,
            implements,
            nodes,
            cords,
            exports,
            bindings,
            port_groups,
            pools,
            supervisions,
            source_span: SourceSpan {
                line: start_line,
                column: start_column,
                end_line,
                end_column,
            },
        })
    }

    fn parse_interface(&mut self) -> Result<InterfaceDeclaration, ParseError> {
        if self.panel_version < 2 {
            return Err(self.error_code(
                "CND-SRC-007",
                "`interface` requires `panel 2`; grammar version 1 is frozen",
            ));
        }
        let start_line = self.current().line;
        let start_column = self.current().column;
        let id = self.expect_any_word()?;
        self.expect_simple(TokenKind::LeftBrace, "`{`")?;
        let mut members = Vec::new();
        let mut member_keys = BTreeSet::new();
        while !matches!(self.current().kind, TokenKind::RightBrace) {
            if matches!(self.current().kind, TokenKind::Eof) {
                return Err(self.error("unterminated interface declaration"));
            }
            let member_start_line = self.current().line;
            let member_start_column = self.current().column;
            let direction_word = self.expect_any_word()?;
            let direction = match direction_word.as_str() {
                "input" => ExportDirection::Input,
                "output" => ExportDirection::Output,
                _ => {
                    return Err(self.error(format!(
                        "interface member direction must be `input` or `output`; found `{direction_word}`"
                    )));
                }
            };
            let member_id = self.expect_any_word()?;
            self.expect_simple(TokenKind::Colon, "`:`")?;
            let contract_start_line = self.current().line;
            let contract_start_column = self.current().column;
            let port_contract = self.expect_any_word()?;
            let (contract_end_line, contract_end_column) = self.previous_end();
            let contract_span = SourceSpan {
                line: contract_start_line,
                column: contract_start_column,
                end_line: contract_end_line,
                end_column: contract_end_column,
            };
            let optional = if self.current_word_is("optional") {
                self.advance();
                true
            } else {
                false
            };
            let (member_end_line, member_end_column) = self.previous_end();
            let key = (
                match direction {
                    ExportDirection::Input => 0_u8,
                    ExportDirection::Output => 1_u8,
                },
                member_id.clone(),
            );
            if !member_keys.insert(key) {
                return Err(self.error_code(
                    "CND-SRC-002",
                    format!("duplicate interface member `{member_id}`"),
                ));
            }
            members.push(InterfaceMemberDeclaration {
                direction,
                id: member_id,
                port_contract,
                optional,
                source_span: SourceSpan {
                    line: member_start_line,
                    column: member_start_column,
                    end_line: member_end_line,
                    end_column: member_end_column,
                },
                contract_span,
            });
            if members.len() > MAXIMUM_INTERFACE_MEMBERS {
                return Err(self.error_code("CND-SEC-001", "interface member limit exceeded"));
            }
        }
        self.advance();
        let (end_line, end_column) = self.previous_end();
        Ok(InterfaceDeclaration {
            id,
            members,
            source_span: SourceSpan {
                line: start_line,
                column: start_column,
                end_line,
                end_column,
            },
        })
    }

    fn parse_implements(&mut self) -> Result<Vec<InterfaceClaim>, ParseError> {
        if self.panel_version < 2 {
            return Err(self.error_code(
                "CND-SRC-007",
                "`implements` requires `panel 2`; grammar version 1 is frozen",
            ));
        }
        self.advance();
        let mut claims = Vec::new();
        let mut seen = BTreeSet::new();
        loop {
            let start_line = self.current().line;
            let start_column = self.current().column;
            let interface = self.expect_any_word()?;
            let (end_line, end_column) = self.previous_end();
            if !seen.insert(interface.clone()) {
                return Err(self.error_code(
                    "CND-SRC-002",
                    format!("duplicate interface claim `{interface}`"),
                ));
            }
            claims.push(InterfaceClaim {
                interface,
                source_span: SourceSpan {
                    line: start_line,
                    column: start_column,
                    end_line,
                    end_column,
                },
            });
            if claims.len() > MAXIMUM_INTERFACE_CLAIMS {
                return Err(self.error_code("CND-SEC-001", "interface claim limit exceeded"));
            }
            if matches!(self.current().kind, TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(claims)
    }

    fn parse_parameters(&mut self) -> Result<Vec<Parameter>, ParseError> {
        self.expect_simple(TokenKind::LeftParen, "`(`")?;
        let mut parameters = Vec::new();
        while !matches!(self.current().kind, TokenKind::RightParen) {
            let start_line = self.current().line;
            let start_column = self.current().column;
            let id = self.expect_any_word()?;
            self.expect_simple(TokenKind::Colon, "`:`")?;
            let value_type = self.expect_any_word()?;
            let (default, default_span) = if matches!(self.current().kind, TokenKind::Equals) {
                self.advance();
                let start_line = self.current().line;
                let start_column = self.current().column;
                let value = self.expect_source_value()?;
                let (end_line, end_column) = self.previous_end();
                (
                    Some(value),
                    Some(SourceSpan {
                        line: start_line,
                        column: start_column,
                        end_line,
                        end_column,
                    }),
                )
            } else {
                (None, None)
            };
            let (end_line, end_column) = self.previous_end();
            parameters.push(Parameter {
                id,
                value_type,
                default,
                source_span: SourceSpan {
                    line: start_line,
                    column: start_column,
                    end_line,
                    end_column,
                },
                default_span,
            });
            if matches!(self.current().kind, TokenKind::Comma) {
                self.advance();
            } else if !matches!(self.current().kind, TokenKind::RightParen) {
                return Err(self.error("expected `,` or `)` after parameter"));
            }
        }
        self.advance();
        Ok(parameters)
    }

    fn parse_node(&mut self) -> Result<Node, ParseError> {
        let start_line = self.current().line;
        let start_column = self.current().column;
        let id = self.expect_any_word()?;
        self.parse_node_after_id(id, start_line, start_column)
    }

    fn parse_node_after_id(
        &mut self,
        id: String,
        start_line: usize,
        start_column: usize,
    ) -> Result<Node, ParseError> {
        self.expect_simple(TokenKind::Colon, "`:`")?;
        let kind = self.expect_any_word()?;
        let constraint = if self.current_word_is("using") {
            self.advance();
            let start_line = self.current().line;
            let start_column = self.current().column;
            let constraint = self.expect_any_word()?;
            let (end_line, end_column) = self.previous_end();
            Some((
                constraint,
                SourceSpan {
                    line: start_line,
                    column: start_column,
                    end_line,
                    end_column,
                },
            ))
        } else {
            None
        };
        let implements = if self.current_word_is("implements") {
            self.parse_implements()?
        } else {
            Vec::new()
        };
        let config = if matches!(self.current().kind, TokenKind::LeftBrace) {
            self.advance();
            self.parse_config_block()?
        } else {
            Vec::new()
        };
        let (end_line, end_column) = self.previous_end();
        Ok(Node {
            id,
            kind,
            constraint: constraint.as_ref().map(|(value, _)| value.clone()),
            constraint_span: constraint.map(|(_, span)| span),
            implements,
            config,
            source_span: SourceSpan {
                line: start_line,
                column: start_column,
                end_line,
                end_column,
            },
        })
    }

    fn parse_port_group(&mut self) -> Result<PortGroup, ParseError> {
        let start_line = self.current().line;
        let start_column = self.current().column;
        let id = self.expect_any_word()?;
        let direction = match self.expect_any_word()?.as_str() {
            "input" => ExportDirection::Input,
            "output" => ExportDirection::Output,
            _ => return Err(self.error("port-group direction must be `input` or `output`")),
        };
        self.expect_simple(TokenKind::Colon, "`:`")?;
        let port_contract = self.expect_any_word()?;
        let shape_name = self.expect_any_word()?;
        self.expect_word("max")?;
        let maximum = self.expect_bounded_u16("port-group maximum")?;
        if maximum == 0 {
            return Err(self.error_code(
                "CND-SRC-008",
                "port-group maximum must be positive and finite",
            ));
        }
        let shape = match shape_name.as_str() {
            "indexed" => PortGroupShape::Indexed,
            "keyed" => {
                self.expect_simple(TokenKind::LeftBrace, "`{`")?;
                let mut members = Vec::new();
                let mut unique = BTreeSet::new();
                while !matches!(self.current().kind, TokenKind::RightBrace) {
                    if matches!(self.current().kind, TokenKind::Eof) {
                        return Err(self.error("unterminated keyed port-group"));
                    }
                    self.expect_word("member")?;
                    let member_start_line = self.current().line;
                    let member_start_column = self.current().column;
                    let member = self.expect_any_word()?;
                    if !unique.insert(member.clone()) {
                        return Err(self.error_code(
                            "CND-SRC-002",
                            format!("duplicate port-group member `{member}`"),
                        ));
                    }
                    let (member_end_line, member_end_column) = self.previous_end();
                    members.push(PortGroupMember {
                        key: member,
                        source_span: SourceSpan {
                            line: member_start_line,
                            column: member_start_column,
                            end_line: member_end_line,
                            end_column: member_end_column,
                        },
                    });
                }
                self.advance();
                if members.is_empty() || members.len() > usize::from(maximum) {
                    return Err(self.error_code(
                        "CND-SRC-008",
                        "keyed member count must be positive and at most `max`",
                    ));
                }
                PortGroupShape::Keyed(members)
            }
            _ => return Err(self.error("port-group shape must be `keyed` or `indexed`")),
        };
        let (end_line, end_column) = self.previous_end();
        Ok(PortGroup {
            id,
            direction,
            port_contract,
            maximum,
            shape,
            source_span: SourceSpan {
                line: start_line,
                column: start_column,
                end_line,
                end_column,
            },
        })
    }

    fn parse_pool(&mut self) -> Result<InstancePool, ParseError> {
        let start_line = self.current().line;
        let start_column = self.current().column;
        let id = self.expect_any_word()?;
        self.expect_simple(TokenKind::Colon, "`:`")?;
        let template = self.expect_any_word()?;
        self.expect_simple(TokenKind::LeftBrace, "`{`")?;
        let mut maximum = None;
        let mut admission = None;
        let mut admission_queue = None;
        let mut deadline_ms = None;
        let mut idle_timeout_ms = None;
        let mut supervision = None;
        let mut restart_attempts = None;
        let mut restart_backoff_ms = None;
        let mut fallback = None;
        let mut cleanup = None;
        let mut fields = BTreeSet::new();
        while !matches!(self.current().kind, TokenKind::RightBrace) {
            if matches!(self.current().kind, TokenKind::Eof) {
                return Err(self.error("unterminated pool policy"));
            }
            let key = self.expect_any_word()?;
            if !fields.insert(key.clone()) {
                return Err(self.error_code("CND-SRC-002", format!("duplicate pool field `{key}`")));
            }
            self.expect_simple(TokenKind::Equals, "`=`")?;
            match key.as_str() {
                "maximum" => maximum = Some(self.expect_bounded_u16("pool maximum")?),
                "admission" => admission = Some(self.expect_any_word()?),
                "admission_queue" => {
                    admission_queue = Some(self.expect_bounded_u16("admission queue")?)
                }
                "deadline_ms" => deadline_ms = Some(self.expect_number()?),
                "idle_timeout_ms" => idle_timeout_ms = Some(self.expect_number()?),
                "supervision" => supervision = Some(self.expect_any_word()?),
                "restart_attempts" => restart_attempts = Some(self.expect_u16("restart attempts")?),
                "restart_backoff_ms" => restart_backoff_ms = Some(self.expect_number()?),
                "fallback" => fallback = Some(self.expect_any_word()?),
                "cleanup" => cleanup = Some(self.expect_any_word()?),
                _ => return Err(self.error(format!("unknown pool field `{key}`"))),
            }
        }
        self.advance();

        let maximum = maximum.ok_or_else(|| {
            self.error_code("CND-SRC-008", "pool requires a positive finite `maximum`")
        })?;
        if maximum == 0 {
            return Err(self.error_code("CND-SRC-008", "pool maximum must be positive and finite"));
        }
        let admission = match admission
            .ok_or_else(|| self.error("pool requires `admission`"))?
            .as_str()
        {
            "reject" => PoolAdmission::Reject,
            "block" => PoolAdmission::Block,
            "queue_bounded" | "queue-bounded" => {
                let capacity = admission_queue.ok_or_else(|| {
                    self.error("queue-bounded admission requires `admission_queue`")
                })?;
                if capacity == 0 {
                    return Err(self
                        .error_code("CND-SRC-008", "admission queue must be positive and finite"));
                }
                PoolAdmission::QueueBounded(capacity)
            }
            "fail" => PoolAdmission::Fail,
            _ => return Err(self.error("unknown pool admission policy")),
        };
        if !matches!(&admission, PoolAdmission::QueueBounded(_)) && admission_queue.is_some() {
            return Err(self.error("`admission_queue` is valid only with queue-bounded admission"));
        }
        let fallback_supplied = fallback.is_some();
        let supervision = match supervision
            .ok_or_else(|| self.error("pool requires `supervision`"))?
            .as_str()
        {
            "fail_together" | "fail-together" => PoolSupervision::FailTogether,
            "isolate" => PoolSupervision::Isolate,
            "restart_bounded" | "restart-bounded" => PoolSupervision::RestartBounded {
                attempts: {
                    let attempts = restart_attempts
                        .ok_or_else(|| self.error("bounded restart requires `restart_attempts`"))?;
                    if attempts == 0 {
                        return Err(self.error_code(
                            "CND-SRC-008",
                            "bounded restart attempts must be positive and finite",
                        ));
                    }
                    attempts
                },
                backoff_ms: restart_backoff_ms
                    .ok_or_else(|| self.error("bounded restart requires `restart_backoff_ms`"))?,
            },
            "fallback" => PoolSupervision::Fallback(
                fallback.ok_or_else(|| self.error("fallback supervision requires `fallback`"))?,
            ),
            "escalate" => PoolSupervision::Escalate,
            _ => return Err(self.error("unknown pool supervision policy")),
        };
        if !matches!(&supervision, PoolSupervision::RestartBounded { .. })
            && (restart_attempts.is_some() || restart_backoff_ms.is_some())
        {
            return Err(
                self.error("restart fields are valid only with bounded-restart supervision")
            );
        }
        if !matches!(&supervision, PoolSupervision::Fallback(_)) && fallback_supplied {
            return Err(self.error("`fallback` is valid only with fallback supervision"));
        }
        let cleanup = match cleanup
            .ok_or_else(|| self.error("pool requires `cleanup`"))?
            .as_str()
        {
            "drain" => PoolCleanup::Drain,
            "abort" => PoolCleanup::Abort,
            _ => return Err(self.error("pool cleanup must be `drain` or `abort`")),
        };
        let (end_line, end_column) = self.previous_end();
        Ok(InstancePool {
            id,
            template,
            maximum,
            admission,
            deadline_ms: deadline_ms.ok_or_else(|| self.error("pool requires `deadline_ms`"))?,
            idle_timeout_ms: idle_timeout_ms
                .ok_or_else(|| self.error("pool requires `idle_timeout_ms`"))?,
            supervision,
            cleanup,
            source_span: SourceSpan {
                line: start_line,
                column: start_column,
                end_line,
                end_column,
            },
        })
    }

    fn parse_supervision(&mut self) -> Result<SupervisionBinding, ParseError> {
        if self.panel_version < 2 {
            return Err(self.error_code(
                "CND-SRC-007",
                "`supervise` requires `panel 2`; grammar version 1 is frozen",
            ));
        }
        let start_line = self.current().line;
        let start_column = self.current().column;
        let subject = self.expect_any_word()?;
        self.expect_word("with")?;
        let handler = self.expect_any_word()?;
        let (end_line, end_column) = self.previous_end();
        Ok(SupervisionBinding {
            subject,
            handler,
            resolved_identity: None,
            source_span: SourceSpan {
                line: start_line,
                column: start_column,
                end_line,
                end_column,
            },
        })
    }

    fn parse_cord(&mut self, ordinal: usize) -> Result<Cord, ParseError> {
        let start_line = self.current().line;
        let start_column = self.current().column;
        let from = self.expect_endpoint()?;
        self.expect_simple(TokenKind::Arrow, "`->`")?;
        let to = self.expect_endpoint()?;
        let mut capacity_items = 8_u16;
        let mut pressure_name = "block".to_owned();
        let mut max_value_bytes = None;
        let mut max_queued_bytes = None;
        let mut low_watermark_items = None;
        let mut high_watermark_items = None;
        let mut coalescer = None;
        let mut sample_every = None;
        let mut sample_offset = 0_u32;
        let mut fields = BTreeSet::new();
        if matches!(self.current().kind, TokenKind::LeftBrace) {
            self.advance();
            while !matches!(self.current().kind, TokenKind::RightBrace) {
                let key = self.expect_any_word()?;
                if !fields.insert(key.clone()) {
                    return Err(
                        self.error_code("CND-SRC-002", format!("duplicate cord field `{key}`"))
                    );
                }
                self.expect_simple(TokenKind::Equals, "`=`")?;
                match key.as_str() {
                    "capacity" => {
                        let capacity = self.expect_number()?;
                        capacity_items = u16::try_from(capacity)
                            .map_err(|_| self.error("cord capacity does not fit in u16"))?;
                    }
                    "pressure" => {
                        pressure_name = self.expect_any_word()?;
                    }
                    "max_value_bytes" => {
                        max_value_bytes = Some(
                            u32::try_from(self.expect_number()?)
                                .map_err(|_| self.error("value byte bound does not fit in u32"))?,
                        );
                    }
                    "max_queued_bytes" => {
                        max_queued_bytes = Some(self.expect_number()?);
                    }
                    "low_watermark" => {
                        low_watermark_items = Some(
                            u16::try_from(self.expect_number()?)
                                .map_err(|_| self.error("low watermark does not fit in u16"))?,
                        );
                    }
                    "high_watermark" => {
                        high_watermark_items = Some(
                            u16::try_from(self.expect_number()?)
                                .map_err(|_| self.error("high watermark does not fit in u16"))?,
                        );
                    }
                    "coalescer" => coalescer = Some(self.expect_any_word()?),
                    "sample_every" => {
                        sample_every = Some(
                            u32::try_from(self.expect_number()?)
                                .map_err(|_| self.error("sample period does not fit in u32"))?,
                        );
                    }
                    "sample_offset" => {
                        sample_offset = u32::try_from(self.expect_number()?)
                            .map_err(|_| self.error("sample offset does not fit in u32"))?;
                    }
                    _ => return Err(self.error(format!("unknown cord field `{key}`"))),
                }
            }
            self.advance();
        }

        let max_value_bytes = max_value_bytes.unwrap_or(65_536);
        let max_queued_bytes =
            max_queued_bytes.unwrap_or(u64::from(capacity_items) * u64::from(max_value_bytes));
        let high_watermark_items = high_watermark_items.unwrap_or(capacity_items);
        let low_watermark_items =
            low_watermark_items.unwrap_or(high_watermark_items.saturating_sub(1));
        let pressure = match pressure_name.as_str() {
            "block" => SourcePressure::Block,
            "reject" => SourcePressure::Reject,
            "coalesce" => SourcePressure::Coalesce {
                relation: coalescer.ok_or_else(|| self.error("coalesce requires `coalescer`"))?,
            },
            "sample" => SourcePressure::Sample {
                every: sample_every.ok_or_else(|| self.error("sample requires `sample_every`"))?,
                offset: sample_offset,
            },
            "drop_disposable" | "drop-disposable" => SourcePressure::DropDisposable,
            "disconnect" => SourcePressure::Disconnect,
            "fail" => SourcePressure::Fail,
            _ => return Err(self.error("unknown pressure behavior")),
        };
        let (end_line, end_column) = self.previous_end();

        Ok(Cord {
            id: format!("cord-{ordinal}"),
            from,
            to,
            capacity_items,
            max_value_bytes,
            max_queued_bytes,
            low_watermark_items,
            high_watermark_items,
            pressure,
            source_span: SourceSpan {
                line: start_line,
                column: start_column,
                end_line,
                end_column,
            },
        })
    }

    fn parse_config_block(&mut self) -> Result<Vec<ConfigEntry>, ParseError> {
        let mut entries = Vec::new();
        let mut keys = BTreeSet::new();
        while !matches!(self.current().kind, TokenKind::RightBrace) {
            let key = self.expect_any_word()?;
            if !keys.insert(key.clone()) {
                return Err(self.error_code(
                    "CND-SRC-002",
                    format!("duplicate configuration field `{key}`"),
                ));
            }
            self.expect_simple(TokenKind::Equals, "`=`")?;
            let start_line = self.current().line;
            let start_column = self.current().column;
            let value = self.expect_source_value()?;
            let (end_line, end_column) = self.previous_end();
            entries.push(ConfigEntry {
                key,
                value,
                source_span: SourceSpan {
                    line: start_line,
                    column: start_column,
                    end_line,
                    end_column,
                },
            });
        }
        self.advance();
        Ok(entries)
    }

    fn expect_endpoint(&mut self) -> Result<Endpoint, ParseError> {
        let value = self.expect_any_word()?;
        self.endpoint_from_word(value)
    }

    fn endpoint_from_word(&self, value: String) -> Result<Endpoint, ParseError> {
        let Some((node, port)) = value.rsplit_once('.') else {
            return Err(self.error(format!("endpoint `{value}` must be `node.port`")));
        };
        if node.is_empty() || port.is_empty() {
            return Err(self.error(format!("endpoint `{value}` must be `node.port`")));
        }
        Ok(Endpoint {
            node: node.to_owned(),
            port: port.to_owned(),
        })
    }

    fn expect_source_value(&mut self) -> Result<SourceValue, ParseError> {
        if self.source_value_depth >= MAXIMUM_SOURCE_VALUE_DEPTH {
            return Err(self.error_code("CND-SEC-002", "source value nesting limit exceeded"));
        }
        self.source_value_depth += 1;
        let result = self.expect_source_value_inner();
        self.source_value_depth -= 1;
        result
    }

    fn expect_source_value_inner(&mut self) -> Result<SourceValue, ParseError> {
        match self.current().kind.clone() {
            TokenKind::String(value) => {
                self.advance();
                Ok(SourceValue::Text(value))
            }
            TokenKind::Number(value) => {
                self.advance();
                Ok(SourceValue::Integer(value))
            }
            TokenKind::Word(value) => {
                self.advance();
                match value.as_str() {
                    "true" => Ok(SourceValue::Boolean(true)),
                    "false" => Ok(SourceValue::Boolean(false)),
                    _ if matches!(self.current().kind, TokenKind::LeftParen) => {
                        self.parse_value_call(&value)
                    }
                    _ => Ok(SourceValue::Reference(value)),
                }
            }
            _ => Err(self.error("expected source value")),
        }
    }

    fn parse_value_call(&mut self, function: &str) -> Result<SourceValue, ParseError> {
        self.expect_simple(TokenKind::LeftParen, "`(`")?;
        match function {
            "bytes" => {
                let value = self.expect_string()?;
                self.expect_simple(TokenKind::RightParen, "`)`")?;
                Ok(SourceValue::Bytes(decode_source_hex(&value).map_err(
                    |message| self.error_code("CND-SRC-010", message),
                )?))
            }
            "ref" | "contract" | "secret" | "decimal" => {
                let value = match self.current().kind.clone() {
                    TokenKind::String(value) | TokenKind::Word(value) => {
                        self.advance();
                        value
                    }
                    _ => return Err(self.error("literal function requires text argument")),
                };
                self.expect_simple(TokenKind::RightParen, "`)`")?;
                match function {
                    "ref" => Ok(SourceValue::Reference(value)),
                    "contract" => Ok(SourceValue::ContractReference(value)),
                    "secret" => Ok(SourceValue::SecretReference(value)),
                    "decimal" => {
                        if !valid_exact_decimal(&value) {
                            return Err(self.error_code(
                                "CND-SRC-010",
                                "decimal requires exact base-10 text without exponent",
                            ));
                        }
                        Ok(SourceValue::ExactDecimal(value))
                    }
                    _ => unreachable!(),
                }
            }
            "list" => {
                let mut values = Vec::new();
                while !matches!(self.current().kind, TokenKind::RightParen) {
                    values.push(self.expect_source_value()?);
                    self.expect_comma_or_right_paren()?;
                }
                self.advance();
                Ok(SourceValue::List(values))
            }
            "record" | "map" => {
                let mut fields = Vec::new();
                let mut keys = BTreeSet::new();
                while !matches!(self.current().kind, TokenKind::RightParen) {
                    let key = self.expect_any_word()?;
                    if !keys.insert(key.clone()) {
                        return Err(self
                            .error_code("CND-SRC-002", format!("duplicate record field `{key}`")));
                    }
                    self.expect_simple(TokenKind::Equals, "`=`")?;
                    fields.push((key, self.expect_source_value()?));
                    self.expect_comma_or_right_paren()?;
                }
                self.advance();
                Ok(SourceValue::Record(fields))
            }
            _ => Err(self.error_code(
                "CND-SRC-010",
                format!("unknown literal constructor `{function}`"),
            )),
        }
    }

    fn expect_comma_or_right_paren(&mut self) -> Result<(), ParseError> {
        if matches!(self.current().kind, TokenKind::Comma) {
            self.advance();
            if matches!(self.current().kind, TokenKind::RightParen) {
                return Err(self.error("trailing comma is not part of source grammar version 1"));
            }
            Ok(())
        } else if matches!(self.current().kind, TokenKind::RightParen) {
            Ok(())
        } else {
            Err(self.error("expected `,` or `)`"))
        }
    }

    fn expect_string(&mut self) -> Result<String, ParseError> {
        if let TokenKind::String(value) = self.current().kind.clone() {
            self.advance();
            Ok(value)
        } else {
            Err(self.error("expected string"))
        }
    }

    fn expect_u16(&mut self, label: &str) -> Result<u16, ParseError> {
        u16::try_from(self.expect_number()?)
            .map_err(|_| self.error(format!("{label} does not fit in u16")))
    }

    fn expect_bounded_u16(&mut self, label: &str) -> Result<u16, ParseError> {
        u16::try_from(self.expect_number()?)
            .map_err(|_| self.error_code("CND-SRC-008", format!("{label} does not fit in u16")))
    }

    fn expect_word(&mut self, expected: &str) -> Result<(), ParseError> {
        let actual = self.expect_any_word()?;
        if actual == expected {
            Ok(())
        } else {
            Err(self.error(format!("expected `{expected}`, found `{actual}`")))
        }
    }

    fn expect_any_word(&mut self) -> Result<String, ParseError> {
        if let TokenKind::Word(value) = self.current().kind.clone() {
            self.advance();
            Ok(value)
        } else {
            Err(self.error("expected identifier"))
        }
    }

    fn expect_number(&mut self) -> Result<u64, ParseError> {
        if let TokenKind::Number(value) = self.current().kind {
            self.advance();
            u64::try_from(value).map_err(|_| self.error("expected non-negative u64 integer"))
        } else {
            Err(self.error("expected integer"))
        }
    }

    fn expect_simple(&mut self, expected: TokenKind, label: &str) -> Result<(), ParseError> {
        if self.current().kind == expected {
            self.advance();
            Ok(())
        } else {
            Err(self.error(format!("expected {label}")))
        }
    }

    fn current_word_is(&self, expected: &str) -> bool {
        matches!(&self.current().kind, TokenKind::Word(value) if value == expected)
    }

    fn current(&self) -> &Token {
        &self.tokens[self.index]
    }

    fn previous_end(&self) -> (usize, usize) {
        let previous = &self.tokens[self.index.saturating_sub(1)];
        (previous.end_line, previous.end_column)
    }

    fn advance(&mut self) {
        self.index += 1;
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        self.error_code("CND-SRC-001", message)
    }

    fn error_code(&self, code: &'static str, message: impl Into<String>) -> ParseError {
        ParseError {
            code,
            line: self.current().line,
            column: self.current().column,
            message: message.into(),
        }
    }
}

fn decode_source_hex(value: &str) -> Result<Vec<u8>, &'static str> {
    if value.len() % 2 != 0 {
        return Err("bytes literal must have even hexadecimal length");
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair)
                .map_err(|_| "bytes literal must contain ASCII hexadecimal")?;
            u8::from_str_radix(pair, 16)
                .map_err(|_| "bytes literal contains non-hexadecimal digits")
        })
        .collect()
}

fn valid_exact_decimal(value: &str) -> bool {
    let value = value
        .strip_prefix('-')
        .or_else(|| value.strip_prefix('+'))
        .unwrap_or(value);
    let Some((integer, fraction)) = value.split_once('.') else {
        return !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit());
    };
    !integer.is_empty()
        && !fraction.is_empty()
        && integer.bytes().all(|byte| byte.is_ascii_digit())
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
        && !fraction.contains('.')
}

fn validate_source_symbols(
    panel: Panel,
    diagnostic_line: usize,
    diagnostic_column: usize,
) -> Result<Panel, ParseError> {
    let duplicate = |kind: &str, id: &str| ParseError {
        code: "CND-SRC-002",
        line: diagnostic_line,
        column: diagnostic_column,
        message: format!("duplicate {kind} `{id}`"),
    };
    let mut aliases = BTreeSet::new();
    for import in &panel.imports {
        if !aliases.insert(import.alias.as_str()) {
            return Err(duplicate("import alias", &import.alias));
        }
    }
    let mut interfaces = BTreeSet::new();
    for interface in &panel.interfaces {
        if !interfaces.insert(interface.id.as_str()) {
            return Err(duplicate("interface", &interface.id));
        }
    }
    let mut definitions = BTreeSet::new();
    for definition in &panel.definitions {
        if !definitions.insert(definition.id.as_str()) {
            return Err(duplicate("definition", &definition.id));
        }
        let mut names = BTreeSet::new();
        for parameter in &definition.parameters {
            if !names.insert(parameter.id.as_str()) {
                return Err(duplicate("definition member", &parameter.id));
            }
        }
        for node in &definition.nodes {
            if !names.insert(node.id.as_str()) {
                return Err(duplicate("definition member", &node.id));
            }
        }
        for group in &definition.port_groups {
            if !names.insert(group.id.as_str()) {
                return Err(duplicate("definition member", &group.id));
            }
        }
        for pool in &definition.pools {
            if !names.insert(pool.id.as_str()) {
                return Err(duplicate("definition member", &pool.id));
            }
        }
        let mut exports = BTreeSet::new();
        for export in &definition.exports {
            let direction = match export.direction {
                ExportDirection::Input => 0_u8,
                ExportDirection::Output => 1_u8,
            };
            if !exports.insert((direction, export.id.as_str())) {
                return Err(duplicate("export", &export.id));
            }
        }
        let mut bindings = BTreeSet::new();
        for binding in &definition.bindings {
            if !bindings.insert(binding.parameter.as_str()) {
                return Err(duplicate("binding", &binding.parameter));
            }
            if !definition.parameters.is_empty()
                && !definition
                    .parameters
                    .iter()
                    .any(|parameter| parameter.id == binding.parameter)
            {
                return Err(ParseError {
                    code: "CND-SRC-003",
                    line: diagnostic_line,
                    column: diagnostic_column,
                    message: format!(
                        "binding `{}` names no declared parameter in `{}`",
                        binding.parameter, definition.id
                    ),
                });
            }
        }
        let children: BTreeSet<&str> = definition
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect();
        let mut supervised = BTreeSet::new();
        for supervision in &definition.supervisions {
            if !supervised.insert(supervision.subject.as_str()) {
                return Err(duplicate("supervision subject", &supervision.subject));
            }
            if supervision.subject == supervision.handler
                || !children.contains(supervision.subject.as_str())
                || !children.contains(supervision.handler.as_str())
            {
                return Err(ParseError {
                    code: "CND-SRC-012",
                    line: supervision.source_span.line,
                    column: supervision.source_span.column,
                    message: format!(
                        "supervision in `{}` requires distinct declared child subject `{}` and handler `{}`",
                        definition.id, supervision.subject, supervision.handler
                    ),
                });
            }
        }
        for endpoint in definition
            .cords
            .iter()
            .flat_map(|cord| [&cord.from, &cord.to])
            .chain(definition.exports.iter().map(|export| &export.target))
            .chain(definition.bindings.iter().map(|binding| &binding.target))
        {
            if !children.contains(endpoint.node.as_str()) {
                return Err(ParseError {
                    code: "CND-SRC-009",
                    line: diagnostic_line,
                    column: diagnostic_column,
                    message: format!(
                        "definition `{}` endpoint bypasses or names no child `{}`",
                        definition.id, endpoint.node
                    ),
                });
            }
        }
        for export in &definition.exports {
            let Some(child) = definition
                .nodes
                .iter()
                .find(|node| node.id == export.target.node)
            else {
                continue;
            };
            let Some(child_definition) = panel
                .definitions
                .iter()
                .find(|candidate| candidate.id == child.kind)
            else {
                // Catalog and imported contracts are validated during typed
                // lowering; source parsing has no semantic registry.
                continue;
            };
            if !definition_exposes_port(child_definition, &export.target.port, export.direction) {
                return Err(ParseError {
                    code: "CND-SRC-009",
                    line: diagnostic_line,
                    column: diagnostic_column,
                    message: format!(
                        "definition `{}` exports unknown or inaccessible member `{}.{}`",
                        definition.id, export.target.node, export.target.port
                    ),
                });
            }
        }
    }
    let mut top = BTreeSet::new();
    for node in &panel.nodes {
        if !top.insert(node.id.as_str()) {
            return Err(duplicate("top-level node", &node.id));
        }
    }
    for group in &panel.port_groups {
        if !top.insert(group.id.as_str()) {
            return Err(duplicate("top-level member", &group.id));
        }
    }
    for pool in &panel.pools {
        if !top.insert(pool.id.as_str()) {
            return Err(duplicate("top-level member", &pool.id));
        }
    }
    for endpoint in panel.cords.iter().flat_map(|cord| [&cord.from, &cord.to]) {
        if !top.contains(endpoint.node.as_str()) {
            return Err(ParseError {
                code: "CND-SRC-009",
                line: diagnostic_line,
                column: diagnostic_column,
                message: format!(
                    "top-level endpoint bypasses or names no instance `{}`",
                    endpoint.node
                ),
            });
        }
    }
    let top_nodes: BTreeSet<&str> = panel.nodes.iter().map(|node| node.id.as_str()).collect();
    let mut supervised = BTreeSet::new();
    for supervision in &panel.supervisions {
        if !supervised.insert(supervision.subject.as_str()) {
            return Err(duplicate("supervision subject", &supervision.subject));
        }
        if supervision.subject == supervision.handler
            || !top_nodes.contains(supervision.subject.as_str())
            || !top_nodes.contains(supervision.handler.as_str())
        {
            return Err(ParseError {
                code: "CND-SRC-012",
                line: supervision.source_span.line,
                column: supervision.source_span.column,
                message: format!(
                    "top-level supervision requires distinct declared node subject `{}` and handler `{}`",
                    supervision.subject, supervision.handler
                ),
            });
        }
    }
    let mut roots = BTreeSet::new();
    for root in &panel.roots {
        if !roots.insert(root.target.as_str()) {
            return Err(duplicate("root", &root.target));
        }
        if !definitions.contains(root.target.as_str()) && !top.contains(root.target.as_str()) {
            return Err(ParseError {
                code: "CND-SRC-006",
                line: diagnostic_line,
                column: diagnostic_column,
                message: format!("root `{}` names no definition or instance", root.target),
            });
        }
    }
    Ok(panel)
}

fn definition_exposes_port(
    definition: &CompositeDefinition,
    port: &str,
    direction: ExportDirection,
) -> bool {
    if definition
        .exports
        .iter()
        .any(|export| export.id == port && export.direction == direction)
    {
        return true;
    }
    definition.port_groups.iter().any(|group| {
        if group.direction != direction {
            return false;
        }
        let Some(member) = port
            .strip_prefix(&group.id)
            .and_then(|suffix| suffix.strip_prefix('['))
            .and_then(|suffix| suffix.strip_suffix(']'))
        else {
            return false;
        };
        match &group.shape {
            PortGroupShape::Keyed(members) => members.iter().any(|item| item.key == member),
            PortGroupShape::Indexed => member
                .parse::<u16>()
                .is_ok_and(|index| index < group.maximum),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nodes_configs_and_bounded_cords() {
        let panel = parse(
            r#"
                panel 1
                node greeting : conduit/literal {
                    value = "Hello\n"
                }
                node output : conduit/stdout
                cord greeting.out -> output.in {
                    capacity = 4
                    pressure = reject
                }
            "#,
        )
        .expect("valid panel");

        assert_eq!(panel.nodes.len(), 2);
        assert_eq!(panel.nodes[0].config("value"), Some("Hello\n"));
        assert_eq!(panel.cords[0].capacity_items, 4);
        assert_eq!(panel.cords[0].pressure, SourcePressure::Reject);
        assert_eq!(panel.cords[0].max_value_bytes, 65_536);
        assert_eq!(panel.cords[0].max_queued_bytes, 4 * 65_536);
    }

    #[test]
    fn reports_source_location() {
        let error = parse("panel 1\nnode broken conduit/literal").expect_err("invalid panel");
        assert_eq!(error.code, "CND-SRC-001");
        assert_eq!(error.line, 2);
    }

    #[test]
    fn requires_exact_parameters_for_sampling_and_coalescing() {
        let missing_sample = parse(
            "panel 1\nnode a : conduit/stdin\nnode b : conduit/stdout\n\
             cord a.out -> b.in { pressure = sample }",
        )
        .expect_err("sampling interval must not be implicit");
        assert!(missing_sample.message.contains("sample_every"));

        let missing_coalescer = parse(
            "panel 1\nnode a : conduit/stdin\nnode b : conduit/stdout\n\
             cord a.out -> b.in { pressure = coalesce }",
        )
        .expect_err("coalescing relation must not be implicit");
        assert!(missing_coalescer.message.contains("coalescer"));

        let panel = parse(
            "panel 1\nnode a : conduit/stdin\nnode b : conduit/stdout\n\
             cord a.out -> b.in {\n\
               pressure = sample\n\
               sample_every = 4\n\
               sample_offset = 1\n\
             }",
        )
        .expect("exact sample schedule");
        assert_eq!(
            panel.cords[0].pressure,
            SourcePressure::Sample {
                every: 4,
                offset: 1
            }
        );
    }

    #[test]
    fn parses_composite_exports_and_parameter_bindings() {
        let panel = parse(
            r#"
                panel 1
                composite example/upper-line {
                    node source : conduit/literal
                    node upper : conduit/uppercase
                    cord source.out -> upper.in
                    export output text = upper.out
                    bind value = source.value
                }
                node line : example/upper-line { value = "hello" }
                node sink : conduit/stdout
                cord line.text -> sink.in
            "#,
        )
        .expect("composite source parses");
        let definition = &panel.definitions[0];
        assert_eq!(definition.id, "example/upper-line");
        assert_eq!(definition.exports[0].target.node, "upper");
        assert_eq!(definition.bindings[0].target.port, "value");
        assert_eq!(panel.nodes[0].kind, "example/upper-line");
    }
}
