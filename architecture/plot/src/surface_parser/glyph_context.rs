//! Bounded explicit constructor-context selection on a glyph-family import.
use crate::prelude::*;
use crate::{Span, SpannedText};

type ContextSelections = Vec<(SpannedText, SpannedText)>;

pub(super) fn parse<'a>(
    source: &str,
    import: &'a str,
    statement_start: usize,
) -> Result<(&'a str, ContextSelections), String> {
    let Some((binding, selections)) = import.split_once(" using ") else {
        return Ok((import, Vec::new()));
    };
    let selections = selections.trim();
    let body = selections
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .ok_or("glyph context requires 'using {key: local}'")?;
    if body.trim().is_empty() {
        return Err("glyph context selection must not be empty".into());
    }
    let mut result = Vec::new();
    let mut keys = alloc::collections::BTreeSet::new();
    let base = statement_start + 5 + import.find(selections).unwrap() + 1;
    let mut offset = 0;
    for item in body.split(',') {
        if result.len() == 64 {
            return Err("glyph context exceeds 64 selections".into());
        }
        let (key, local) = item
            .split_once(':')
            .ok_or("glyph context requires a key and immutable local name")?;
        let key = key.trim();
        let local = local.trim();
        if !crate::surface_lex::is_name(key)
            || !crate::surface_lex::is_name(local)
            || key.len() > 256
            || local.len() > 256
        {
            return Err("glyph context keys and local names must be finite identifiers".into());
        }
        if !keys.insert(key) {
            return Err("duplicate glyph context key".into());
        }
        let spanned = |text: &str, start: usize| {
            let (line, column) = crate::surface_lex::location(source, start);
            let (end_line, end_column) = crate::surface_lex::location(source, start + text.len());
            SpannedText {
                text: text.into(),
                span: Span {
                    start,
                    end: start + text.len(),
                    line,
                    column,
                    end_line,
                    end_column,
                },
            }
        };
        result.push((
            spanned(key, base + offset + item.find(key).unwrap()),
            spanned(
                local,
                base + offset
                    + item.find(':').unwrap()
                    + 1
                    + item[item.find(':').unwrap() + 1..].find(local).unwrap(),
            ),
        ));
        offset += item.len() + 1;
    }
    Ok((binding, result))
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn explicit_context_retains_spans_and_refuses_malformed_or_excessive_selections() {
        let source =
            "  with   fixture/notation as ph using {provenance: ph-evidence}\nplot example {\n}\n";
        let document = parse_syntax_document(source);
        assert!(
            document.diagnostics.is_empty(),
            "{:?}",
            document.diagnostics
        );
        let import = &document.uses[0];
        assert_eq!(
            &source[import.alias.span.start..import.alias.span.end],
            "ph"
        );
        let (key, local) = &import.glyph_context[0];
        assert_eq!(&source[key.span.start..key.span.end], "provenance");
        assert_eq!(&source[local.span.start..local.span.end], "ph-evidence");
        for context in [
            "{}",
            "{key: a, key: b}",
            "{key: a.b}",
            "{key: \"a\"}",
            "{key: a,}",
            "{key: a",
        ] {
            let source =
                format!("with fixture/notation as ph using {context}\nplot example {{\n}}\n");
            assert!(
                !parse_syntax_document(&source).diagnostics.is_empty(),
                "{source}"
            );
        }
        let context = (0..65)
            .map(|i| format!("key-{i}: local-{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let source =
            format!("with fixture/notation as ph using {{{context}}}\nplot example {{\n}}\n");
        assert!(!parse_syntax_document(&source).diagnostics.is_empty());
        let ordinary = parse_syntax_document(
            "with fixture/ordinary as gear using {key: local}\nplot example {\n}\n",
        );
        assert!(resolve_glyph_notation_scope(&ordinary, &StartupCatalog::new()).is_err());
    }
}
