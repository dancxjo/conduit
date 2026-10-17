//! Outer grammar boundaries exclude payloads of explicitly resolved glyphs.
use super::Parser;
use crate::prelude::*;

impl Parser<'_> {
    pub(super) fn split_default<'a>(&self, text: &'a str) -> (&'a str, Option<&'a str>) {
        self.top_level_positions(text, '=')
            .into_iter()
            .find(|position| {
                *position == 0
                    || !matches!(text.as_bytes()[position - 1], b'<' | b'>' | b'!' | b'=')
            })
            .map_or((text, None), |position| {
                (&text[..position], Some(&text[position + 1..]))
            })
    }

    pub(super) fn top_level_positions(&self, text: &str, target: char) -> Vec<usize> {
        crate::surface_lex::top_level_positions_in_scope(text, target, self.glyph_scope.as_ref())
    }

    pub(super) fn top_level_token_positions(&self, text: &str, token: &str) -> Vec<usize> {
        let Some(first) = token.chars().next() else {
            return Vec::new();
        };
        self.top_level_positions(text, first)
            .into_iter()
            .filter(|position| text[*position..].starts_with(token))
            .collect()
    }

    pub(super) fn split_top_level<'a>(&self, text: &'a str, target: char) -> Vec<&'a str> {
        self.split_positions(
            text,
            self.top_level_positions(text, target),
            target.len_utf8(),
        )
    }

    pub(super) fn split_top_level_token<'a>(&self, text: &'a str, token: &str) -> Vec<&'a str> {
        self.split_positions(
            text,
            self.top_level_token_positions(text, token),
            token.len(),
        )
    }

    fn split_positions<'a>(
        &self,
        text: &'a str,
        positions: Vec<usize>,
        length: usize,
    ) -> Vec<&'a str> {
        let mut pieces = Vec::new();
        let mut start = 0;
        for position in positions {
            pieces.push(&text[start..position]);
            start = position + length;
        }
        pieces.push(&text[start..]);
        pieces
    }
}
