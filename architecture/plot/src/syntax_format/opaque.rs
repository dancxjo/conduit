//! Exact lexical ranges excluded from indentation and trivia rewriting.
use crate::prelude::*;
use crate::GlyphNotationScope;

pub(super) fn ranges(source: &str, scope: &GlyphNotationScope) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut offset = 0;
    while offset < source.len() {
        let start = offset;
        if let Some(literal) = scope.scan_at(source, offset) {
            offset += literal.consumed_bytes;
        } else if source[offset..].starts_with("r\"") {
            offset += 2;
            offset = source[offset..]
                .find('"')
                .map_or(source.len(), |end| offset + end + 1);
        } else {
            let character = source[offset..].chars().next().unwrap();
            if character == '#' {
                offset = source[offset..]
                    .find('\n')
                    .map_or(source.len(), |end| offset + end);
            } else if matches!(character, '\'' | '"') {
                offset += character.len_utf8();
                let mut escaped = false;
                while offset < source.len() {
                    let next = source[offset..].chars().next().unwrap();
                    offset += next.len_utf8();
                    if next == character && !escaped {
                        break;
                    }
                    escaped = next == '\\' && !escaped;
                    if next != '\\' {
                        escaped = false;
                    }
                }
            } else if character == '/' && source[..offset].trim_end().ends_with('~') {
                if let Some((_, _, _, _, consumed)) =
                    crate::surface_parser::front::pattern::slash_pattern(&source[offset..])
                {
                    offset += consumed;
                } else {
                    offset += 1;
                    continue;
                }
            } else {
                offset += character.len_utf8();
                continue;
            }
        }
        ranges.push((start, offset));
    }
    ranges
}

pub(super) fn inside(ranges: &[(usize, usize)], offset: usize) -> bool {
    let index = ranges.partition_point(|(start, _)| *start <= offset);
    index > 0 && offset < ranges[index - 1].1
}

pub(super) fn continuation(ranges: &[(usize, usize)], offset: usize) -> bool {
    let index = ranges.partition_point(|(start, _)| *start <= offset);
    index > 0 && ranges[index - 1].0 < offset && offset < ranges[index - 1].1
}
