//! Authored source parser for Conduit's finite portable text-pattern language.

use crate::prelude::*;
use crate::TextPatternExpression;

const MAX_SCALAR: u32 = 0x10_ffff;

/// Finite authored pattern payload admitted before recursive parsing.
pub const MAXIMUM_TEXT_PATTERN_SOURCE_BYTES: usize = 4096;
/// Shared recursion ceiling for groups and leading assertions.
pub const MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH: usize = 32;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextPatternSourceError {
    Empty,
    SourceLimitExceeded,
    DepthLimitExceeded {
        offset: usize,
    },
    Unexpected {
        offset: usize,
        character: Option<char>,
    },
    UnclosedGroup {
        offset: usize,
    },
    UnclosedClass {
        offset: usize,
    },
    EmptyClass {
        offset: usize,
    },
    InvalidRange {
        offset: usize,
    },
    InvalidRepeat {
        offset: usize,
    },
}

impl TextPatternSourceError {
    fn shifted(self, base: usize) -> Self {
        match self {
            Self::Empty | Self::SourceLimitExceeded => self,
            Self::DepthLimitExceeded { offset } => Self::DepthLimitExceeded {
                offset: base + offset,
            },
            Self::Unexpected { offset, character } => Self::Unexpected {
                offset: base + offset,
                character,
            },
            Self::UnclosedGroup { offset } => Self::UnclosedGroup {
                offset: base + offset,
            },
            Self::UnclosedClass { offset } => Self::UnclosedClass {
                offset: base + offset,
            },
            Self::EmptyClass { offset } => Self::EmptyClass {
                offset: base + offset,
            },
            Self::InvalidRange { offset } => Self::InvalidRange {
                offset: base + offset,
            },
            Self::InvalidRepeat { offset } => Self::InvalidRepeat {
                offset: base + offset,
            },
        }
    }
}

/// Parses Conduit's deliberately bounded regular-expression surface.
///
/// Authored payloads are limited to 4096 bytes and 32 combined levels of groups
/// and assertions before recursive parsing or automaton construction.
/// This parses the expression inside a `~ /.../flags` relation. The expression
/// itself compiles as an exact regular language; the relation's anchor profile
/// then selects bounded search, prefix, suffix, or whole-value matching. The
/// admitted subset contains sequences, `|`, groups, scalar classes/ranges,
/// `.`, `?`, and finite `{n}`/`{n,m}` repetition. `*` and `+` are bounded by
/// the refined Text contract's admitted input ceiling before the automaton
/// enters a Plan. A leading positive `(?=...)` or negative `(?!...)`
/// lookahead is admitted when its product automaton fits the same bounds;
/// interior assertions refuse instead of introducing a backtracking engine.
pub fn parse_text_pattern(source: &str) -> Result<TextPatternExpression, TextPatternSourceError> {
    if source.len() > MAXIMUM_TEXT_PATTERN_SOURCE_BYTES {
        return Err(TextPatternSourceError::SourceLimitExceeded);
    }
    parse_with_depth(source, 0)
}

fn parse_with_depth(
    source: &str,
    depth: usize,
) -> Result<TextPatternExpression, TextPatternSourceError> {
    if depth > MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH {
        return Err(TextPatternSourceError::DepthLimitExceeded { offset: 0 });
    }
    if source.is_empty() {
        return Err(TextPatternSourceError::Empty);
    }
    if source.starts_with("(?=") || source.starts_with("(?!") {
        if depth == MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH {
            return Err(TextPatternSourceError::DepthLimitExceeded { offset: 0 });
        }
        let negated = source.as_bytes()[2] == b'!';
        let assertion_end = leading_assertion_end(source)?;
        let assertion = parse_with_depth(&source[3..assertion_end], depth + 1)
            .map_err(|error| error.shifted(3))?;
        if assertion_end + 1 == source.len() {
            return Err(TextPatternSourceError::Unexpected {
                offset: source.len(),
                character: None,
            });
        }
        let remainder = parse_with_depth(&source[assertion_end + 1..], depth + 1)
            .map_err(|error| error.shifted(assertion_end + 1))?;
        return Ok(TextPatternExpression::PrefixAssertion {
            assertion: Box::new(assertion),
            remainder: Box::new(remainder),
            negated,
        });
    }
    let mut parser = PatternParser {
        source,
        offset: 0,
        depth,
    };
    let expression = parser.choice()?;
    if parser.offset != source.len() {
        return Err(parser.unexpected());
    }
    Ok(expression)
}

fn leading_assertion_end(source: &str) -> Result<usize, TextPatternSourceError> {
    let mut depth = 1_u16;
    let mut escaped = false;
    let mut class = false;
    for (offset, character) in source.char_indices().skip_while(|(offset, _)| *offset < 3) {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '[' => class = true,
            ']' => class = false,
            '(' if !class => depth = depth.saturating_add(1),
            ')' if !class => {
                depth -= 1;
                if depth == 0 {
                    return Ok(offset);
                }
            }
            _ => {}
        }
    }
    Err(TextPatternSourceError::UnclosedGroup { offset: 0 })
}

struct PatternParser<'a> {
    source: &'a str,
    offset: usize,
    depth: usize,
}

impl PatternParser<'_> {
    fn choice(&mut self) -> Result<TextPatternExpression, TextPatternSourceError> {
        let mut choices = vec![self.sequence()?];
        while self.peek() == Some('|') {
            self.take();
            choices.push(self.sequence()?);
        }
        if choices.len() == 1 {
            Ok(choices.pop().expect("one choice exists"))
        } else {
            Ok(TextPatternExpression::Choice(choices))
        }
    }

    fn sequence(&mut self) -> Result<TextPatternExpression, TextPatternSourceError> {
        let mut expressions = Vec::new();
        while self
            .peek()
            .is_some_and(|character| !matches!(character, '|' | ')'))
        {
            expressions.push(self.repeated_atom()?);
        }
        Ok(match expressions.len() {
            0 => TextPatternExpression::Empty,
            1 => expressions.pop().expect("one expression exists"),
            _ => TextPatternExpression::Sequence(expressions),
        })
    }

    fn repeated_atom(&mut self) -> Result<TextPatternExpression, TextPatternSourceError> {
        let mut expression = self.atom()?;
        match self.peek() {
            Some('?') => {
                self.take();
                expression = TextPatternExpression::Repeat {
                    expression: Box::new(expression),
                    minimum: 0,
                    maximum: 1,
                };
            }
            Some('{') => {
                let repeat_offset = self.offset;
                self.take();
                let minimum = self.number(repeat_offset)?;
                let maximum = if self.peek() == Some(',') {
                    self.take();
                    self.number(repeat_offset)?
                } else {
                    minimum
                };
                if self.take() != Some('}') || minimum > maximum {
                    return Err(TextPatternSourceError::InvalidRepeat {
                        offset: repeat_offset,
                    });
                }
                expression = TextPatternExpression::Repeat {
                    expression: Box::new(expression),
                    minimum,
                    maximum,
                };
            }
            Some(character @ ('*' | '+')) => {
                self.take();
                expression = TextPatternExpression::InputBoundRepeat {
                    expression: Box::new(expression),
                    minimum: u16::from(character == '+'),
                };
            }
            _ => {}
        }
        Ok(expression)
    }

    fn atom(&mut self) -> Result<TextPatternExpression, TextPatternSourceError> {
        let offset = self.offset;
        match self.take() {
            Some('(') => {
                if self.depth == MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH {
                    return Err(TextPatternSourceError::DepthLimitExceeded { offset });
                }
                self.depth += 1;
                if self.source[self.offset..].starts_with("?:") {
                    self.offset += 2;
                } else if self.source[self.offset..].starts_with("?<") {
                    self.offset += 2;
                    let name_start = self.offset;
                    while self.peek().is_some_and(|character| character != '>') {
                        self.take();
                    }
                    let name = &self.source[name_start..self.offset];
                    if self.take() != Some('>') || !portable_group_name(name) {
                        return Err(TextPatternSourceError::Unexpected {
                            offset,
                            character: self.peek(),
                        });
                    }
                }
                let expression = self.choice()?;
                if self.take() != Some(')') {
                    return Err(TextPatternSourceError::UnclosedGroup { offset });
                }
                self.depth -= 1;
                Ok(expression)
            }
            Some('[') => self.class(offset),
            Some('.') => Ok(TextPatternExpression::ScalarRange {
                first: 0,
                last: MAX_SCALAR,
            }),
            Some('\\') => self.escaped_scalar(offset),
            Some(character)
                if matches!(
                    character,
                    '|' | ')' | '{' | '}' | '?' | '*' | '+' | '^' | '$'
                ) =>
            {
                Err(TextPatternSourceError::Unexpected {
                    offset,
                    character: Some(character),
                })
            }
            Some(character) => Ok(scalar_expression(character)),
            None => Err(self.unexpected()),
        }
    }

    fn class(
        &mut self,
        class_offset: usize,
    ) -> Result<TextPatternExpression, TextPatternSourceError> {
        let mut ranges = Vec::new();
        while self.peek().is_some_and(|character| character != ']') {
            let range_offset = self.offset;
            let first = self.class_scalar(class_offset)?;
            let last = if self.peek() == Some('-') {
                self.take();
                if self.peek() == Some(']') {
                    return Err(TextPatternSourceError::InvalidRange {
                        offset: range_offset,
                    });
                }
                self.class_scalar(class_offset)?
            } else {
                first
            };
            if first > last {
                return Err(TextPatternSourceError::InvalidRange {
                    offset: range_offset,
                });
            }
            ranges.push(TextPatternExpression::ScalarRange {
                first: first as u32,
                last: last as u32,
            });
        }
        if self.take() != Some(']') {
            return Err(TextPatternSourceError::UnclosedClass {
                offset: class_offset,
            });
        }
        if ranges.is_empty() {
            return Err(TextPatternSourceError::EmptyClass {
                offset: class_offset,
            });
        }
        Ok(if ranges.len() == 1 {
            ranges.pop().expect("one range exists")
        } else {
            TextPatternExpression::Choice(ranges)
        })
    }

    fn class_scalar(&mut self, class_offset: usize) -> Result<char, TextPatternSourceError> {
        match self.take() {
            Some('\\') => self.escaped_character(self.offset.saturating_sub(1)),
            Some('^') => Err(TextPatternSourceError::Unexpected {
                offset: self.offset.saturating_sub(1),
                character: Some('^'),
            }),
            Some(character) => Ok(character),
            None => Err(TextPatternSourceError::UnclosedClass {
                offset: class_offset,
            }),
        }
    }

    fn escaped_scalar(
        &mut self,
        escape_offset: usize,
    ) -> Result<TextPatternExpression, TextPatternSourceError> {
        self.escaped_character(escape_offset).map(scalar_expression)
    }

    fn escaped_character(&mut self, escape_offset: usize) -> Result<char, TextPatternSourceError> {
        let character = self.take().ok_or(TextPatternSourceError::Unexpected {
            offset: escape_offset,
            character: None,
        })?;
        if !matches!(
            character,
            '[' | ']'
                | '('
                | ')'
                | '{'
                | '}'
                | '|'
                | '.'
                | '?'
                | '*'
                | '+'
                | '-'
                | '\\'
                | '^'
                | '$'
        ) {
            return Err(TextPatternSourceError::Unexpected {
                offset: escape_offset,
                character: Some(character),
            });
        }
        Ok(character)
    }

    fn number(&mut self, repeat_offset: usize) -> Result<u16, TextPatternSourceError> {
        let start = self.offset;
        while self
            .peek()
            .is_some_and(|character| character.is_ascii_digit())
        {
            self.take();
        }
        if start == self.offset {
            return Err(TextPatternSourceError::InvalidRepeat {
                offset: repeat_offset,
            });
        }
        self.source[start..self.offset]
            .parse()
            .map_err(|_| TextPatternSourceError::InvalidRepeat {
                offset: repeat_offset,
            })
    }

    fn peek(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }

    fn take(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.offset += character.len_utf8();
        Some(character)
    }

    fn unexpected(&self) -> TextPatternSourceError {
        TextPatternSourceError::Unexpected {
            offset: self.offset,
            character: self.peek(),
        }
    }
}

fn portable_group_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn scalar_expression(character: char) -> TextPatternExpression {
    TextPatternExpression::ScalarRange {
        first: character as u32,
        last: character as u32,
    }
}
