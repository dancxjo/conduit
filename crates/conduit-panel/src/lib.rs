//! Editable `.panel` source model and parser.
//!
//! The current grammar is intentionally a small executable seed. It establishes
//! source identity, nodes, configuration, typed endpoint references, and
//! bounded cord policy. Composite definitions and imports will extend this
//! grammar without creating a separate runtime Panel object.

use std::fmt;

/// Parsed editable panel source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Panel {
    /// Source grammar major version.
    pub version: u16,
    /// Node instances.
    pub nodes: Vec<Node>,
    /// Cord declarations.
    pub cords: Vec<Cord>,
}

/// One semantic node instance in source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    /// Stable local node ID.
    pub id: String,
    /// Semantic node-contract identity.
    pub kind: String,
    /// Source configuration entries.
    pub config: Vec<ConfigEntry>,
}

impl Node {
    /// Returns one configuration value.
    #[must_use]
    pub fn config(&self, key: &str) -> Option<&str> {
        self.config
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.value.as_str())
    }
}

/// One source configuration key/value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigEntry {
    /// Configuration field name.
    pub key: String,
    /// String representation lowered by the selected contract.
    pub value: String,
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
    Number(u64),
    Colon,
    Equals,
    LeftBrace,
    RightBrace,
    Arrow,
    Eof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Token {
    kind: TokenKind,
    line: usize,
    column: usize,
}

/// Parses the initial `.panel` grammar.
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
    Parser::new(lex(source)?).parse()
}

fn lex(source: &str) -> Result<Vec<Token>, ParseError> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut line = 1;
    let mut column = 1;

    while index < bytes.len() {
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
                });
                index += 1;
                column += 1;
            }
            b'=' => {
                tokens.push(Token {
                    kind: TokenKind::Equals,
                    line,
                    column,
                });
                index += 1;
                column += 1;
            }
            b'{' => {
                tokens.push(Token {
                    kind: TokenKind::LeftBrace,
                    line,
                    column,
                });
                index += 1;
                column += 1;
            }
            b'}' => {
                tokens.push(Token {
                    kind: TokenKind::RightBrace,
                    line,
                    column,
                });
                index += 1;
                column += 1;
            }
            b'-' if bytes.get(index + 1) == Some(&b'>') => {
                tokens.push(Token {
                    kind: TokenKind::Arrow,
                    line,
                    column,
                });
                index += 2;
                column += 2;
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
                let value = text.parse::<u64>().map_err(|error| ParseError {
                    code: "CND-SRC-001",
                    line,
                    column: start_column,
                    message: format!("invalid integer: {error}"),
                })?;
                tokens.push(Token {
                    kind: TokenKind::Number(value),
                    line,
                    column: start_column,
                });
            }
            _ if is_word_byte(byte) => {
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
    });
    Ok(tokens)
}

const fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/' | b'@')
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, index: 0 }
    }

    fn parse(mut self) -> Result<Panel, ParseError> {
        self.expect_word("panel")?;
        let version = self.expect_number()?;
        let version =
            u16::try_from(version).map_err(|_| self.error("panel version does not fit in u16"))?;
        if version != 1 {
            return Err(self.error(format!("unsupported panel version {version}")));
        }

        let mut nodes = Vec::new();
        let mut cords = Vec::new();
        while !matches!(self.current().kind, TokenKind::Eof) {
            let declaration = self.expect_any_word()?;
            match declaration.as_str() {
                "node" => nodes.push(self.parse_node()?),
                "cord" => {
                    let ordinal = cords.len();
                    cords.push(self.parse_cord(ordinal)?);
                }
                _ => {
                    return Err(
                        self.error(format!("expected `node` or `cord`, found `{declaration}`"))
                    );
                }
            }
        }

        Ok(Panel {
            version,
            nodes,
            cords,
        })
    }

    fn parse_node(&mut self) -> Result<Node, ParseError> {
        let id = self.expect_any_word()?;
        self.expect_simple(TokenKind::Colon, "`:`")?;
        let kind = self.expect_any_word()?;
        let config = if matches!(self.current().kind, TokenKind::LeftBrace) {
            self.advance();
            self.parse_config_block()?
        } else {
            Vec::new()
        };
        Ok(Node { id, kind, config })
    }

    fn parse_cord(&mut self, ordinal: usize) -> Result<Cord, ParseError> {
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
        if matches!(self.current().kind, TokenKind::LeftBrace) {
            self.advance();
            while !matches!(self.current().kind, TokenKind::RightBrace) {
                let key = self.expect_any_word()?;
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
        })
    }

    fn parse_config_block(&mut self) -> Result<Vec<ConfigEntry>, ParseError> {
        let mut entries = Vec::new();
        while !matches!(self.current().kind, TokenKind::RightBrace) {
            let key = self.expect_any_word()?;
            self.expect_simple(TokenKind::Equals, "`=`")?;
            let value = match self.current().kind.clone() {
                TokenKind::String(value) | TokenKind::Word(value) => {
                    self.advance();
                    value
                }
                TokenKind::Number(value) => {
                    self.advance();
                    value.to_string()
                }
                _ => return Err(self.error("expected configuration value")),
            };
            entries.push(ConfigEntry { key, value });
        }
        self.advance();
        Ok(entries)
    }

    fn expect_endpoint(&mut self) -> Result<Endpoint, ParseError> {
        let value = self.expect_any_word()?;
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
            Ok(value)
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

    fn current(&self) -> &Token {
        &self.tokens[self.index]
    }

    fn advance(&mut self) {
        self.index += 1;
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        ParseError {
            code: "CND-SRC-001",
            line: self.current().line,
            column: self.current().column,
            message: message.into(),
        }
    }
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
}
