//! Compiled notation catalogs for lexical projection, without Host offers.
use conduit_plot::{
    highlight_syntax, highlight_syntax_in_scope, GlyphNotationScope, ProfileCatalog,
    StartupCatalog, SyntaxHighlightSpan,
};
use std::cell::OnceCell;

thread_local! {
    static CATALOG: OnceCell<Result<StartupCatalog, String>> = const { OnceCell::new() };
}

fn catalog() -> Result<StartupCatalog, String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_text::install_pattern_notation(&mut startup, &mut profile)?;
    conduit_speech::authoring::install(&mut startup)?;
    conduit_speech::ipa_constructors::install(&mut startup, &mut profile)?;
    conduit_speech::ipa_constructors::install_notation(&mut startup, &profile)?;
    Ok(startup)
}

pub(super) fn highlight(source: &str) -> Result<Vec<SyntaxHighlightSpan>, String> {
    CATALOG.with(|catalog_cache| {
        let startup = catalog_cache
            .get_or_init(catalog)
            .as_ref()
            .map_err(Clone::clone)?;
        // Invalid or unfinished import edits remain highlightable. No family
        // is inferred from an alias or delimiter when resolution refuses.
        match GlyphNotationScope::from_source_header(source, startup) {
            Ok(scope) => highlight_syntax_in_scope(source, &scope),
            Err(_) => highlight_syntax(source),
        }
        .map_err(super::refusal)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_plot::SyntaxHighlightKind;

    #[test]
    fn incomplete_edits_resolve_only_explicit_shipped_notation_imports() {
        for (path, spelling, payload) in [
            ("text/pattern/notation", "r⟦t͡ʃ#'\"⟧", "t͡ʃ#'\""),
            ("speech/ipa/notation", "ph[t͡ʃɑ]", "t͡ʃɑ"),
            ("speech/ipa/notation", "ph/ˈkæt/", "ˈkæt"),
        ] {
            let alias = if path.starts_with("speech/") {
                "ph"
            } else {
                "r"
            };
            let source = format!(
                "with {path} as {alias}\nplot unfinished {{\n value = {spelling} # outer\n"
            );
            let spans = highlight(&source).unwrap();
            assert_eq!(
                spans
                    .iter()
                    .map(|span| &source[span.start..span.end])
                    .collect::<String>(),
                source
            );
            assert!(spans
                .iter()
                .any(|span| span.kind == SyntaxHighlightKind::Literal
                    && &source[span.start..span.end] == payload));
            assert!(spans
                .iter()
                .any(|span| span.kind == SyntaxHighlightKind::Comment
                    && &source[span.start..span.end] == "# outer"));
        }
    }

    #[test]
    fn missing_foreign_and_conflicting_imports_never_invent_a_glyph_family() {
        for header in [
            "",
            "with unknown/notation as r\n",
            "with text/pattern/notation as r\nwith speech/ipa/notation as r\n",
        ] {
            let source = format!("{header}plot unfinished {{\n value = r/a#b/\n");
            let spans = highlight(&source).unwrap();
            assert!(!spans
                .iter()
                .any(|span| span.kind == SyntaxHighlightKind::Literal
                    && &source[span.start..span.end] == "a#b"));
        }
    }
}
