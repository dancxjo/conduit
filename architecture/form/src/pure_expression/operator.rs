//! Canonical pure-expression operator spellings and binding powers.

use crate::BinaryOperator;

pub(super) fn binary(tail: &str) -> Option<(BinaryOperator, u8, usize)> {
    if tail
        .strip_prefix("is")
        .is_some_and(|rest| rest.chars().next().is_some_and(char::is_whitespace))
    {
        return Some((BinaryOperator::CaseIs, 7, 2));
    }
    [
        ("<<<", BinaryOperator::ShiftLeft, 9),
        (">>>", BinaryOperator::ShiftRight, 9),
        ("<=", BinaryOperator::LessOrEqual, 8),
        (">=", BinaryOperator::GreaterOrEqual, 8),
        ("==", BinaryOperator::Equal, 7),
        ("!=", BinaryOperator::NotEqual, 7),
        ("&&", BinaryOperator::BooleanAnd, 3),
        ("||", BinaryOperator::BooleanOr, 2),
        ("*", BinaryOperator::Multiply, 11),
        ("/", BinaryOperator::Divide, 11),
        ("%", BinaryOperator::Remainder, 11),
        ("+", BinaryOperator::Add, 10),
        ("-", BinaryOperator::Subtract, 10),
        ("<", BinaryOperator::Less, 8),
        (">", BinaryOperator::Greater, 8),
        ("&", BinaryOperator::BitAnd, 6),
        ("^", BinaryOperator::BitXor, 5),
        ("|", BinaryOperator::BitOr, 4),
    ]
    .into_iter()
    .find_map(|(spelling, operator, precedence)| {
        tail.starts_with(spelling)
            .then_some((operator, precedence, spelling.len()))
    })
}

pub(super) fn is_delimiter(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '(' | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | ','
                | ':'
                | '?'
                | '.'
                | '!'
                | '+'
                | '*'
                | '/'
                | '%'
                | '<'
                | '>'
                | '='
                | '&'
                | '^'
                | '|'
        )
}
