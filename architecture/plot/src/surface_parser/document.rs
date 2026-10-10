//! Full Source assembly and explicit checked-family lexical entrance.
use super::*;

pub(crate) fn parse_surface(source: &str) -> SyntaxDocument {
    parse(source, None)
}

pub(crate) fn parse_surface_scoped(
    source: &str,
    startup: &crate::StartupCatalog,
) -> SyntaxDocument {
    let mut document = parse(source, Some(startup));
    if document.diagnostics.is_empty() {
        if let Err(refusal) = crate::resolve_glyph_notation_scope(&document, startup) {
            document.diagnostics.push(diagnostic(
                PlotError::InvalidSyntax(refusal.message),
                refusal.span,
            ));
        }
    }
    document
}

fn parse(source: &str, startup: Option<&crate::StartupCatalog>) -> SyntaxDocument {
    if source.len() > MAXIMUM_PLOT_SOURCE_BYTES {
        return SyntaxDocument::new(
            String::new(),
            Vec::new(),
            Vec::new(),
            true,
            SyntaxDefinitions::default(),
            vec![diagnostic(
                PlotError::SourceLimitExceeded,
                crate::whole_source_span(source),
            )],
        );
    }
    let tokens = match tokenize_losslessly(source) {
        Ok(tokens) => tokens,
        Err(span) => {
            return SyntaxDocument::new(
                source.to_string(),
                Vec::new(),
                Vec::new(),
                true,
                SyntaxDefinitions::default(),
                vec![diagnostic(PlotError::TokenLimitExceeded, span)],
            );
        }
    };
    match Parser::new(source).parse_document(startup) {
        Ok(parsed) => SyntaxDocument::new(
            source.to_string(),
            tokens,
            parsed.uses,
            parsed.standard_glyphs,
            SyntaxDefinitions {
                types: parsed.types,
                type_forms: parsed.type_forms,
                glyph_notations: parsed.glyph_notations,
                plots: parsed.plots,
                constructions: parsed.constructions,
                packages: parsed.packages,
            },
            Vec::new(),
        ),
        Err((error, span)) => SyntaxDocument::new(
            source.to_string(),
            tokens,
            Vec::new(),
            true,
            SyntaxDefinitions::default(),
            vec![diagnostic(error, span)],
        ),
    }
}

struct ParsedSurface {
    uses: Vec<UseDeclaration>,
    standard_glyphs: bool,
    types: Vec<TypeSyntax>,
    type_forms: Vec<TypeFormSyntax>,
    glyph_notations: Vec<crate::GlyphNotationSyntax>,
    plots: Vec<PlotSyntax>,
    constructions: Vec<ConstructionSyntax>,
    packages: Vec<crate::syntax::PackageSyntax>,
}

impl Parser<'_> {
    fn parse_document(
        mut self,
        startup: Option<&crate::StartupCatalog>,
    ) -> Result<ParsedSurface, (PlotError, Span)> {
        let mut uses = Vec::new();
        let mut standard_glyphs = true;
        let mut types = Vec::new();
        let mut type_forms = Vec::new();
        let mut glyph_notations = Vec::new();
        let mut plots = Vec::new();
        let mut constructions = Vec::new();
        let mut packages = Vec::new();
        self.skip_empty();
        while self.index < self.lines.len() {
            let (text, start) = self.lines[self.index].statement();
            if text == "sans glyphs" {
                if !standard_glyphs {
                    return Err((
                        PlotError::InvalidSyntax("duplicate 'sans glyphs' header".into()),
                        self.line_span(self.lines[self.index]),
                    ));
                }
                standard_glyphs = false;
                self.index += 1;
                self.skip_empty();
                continue;
            }
            let Some(import) = text.strip_prefix("with ") else {
                break;
            };
            uses.extend(self.parse_use(import, text, start)?);
            if uses.len() > MAXIMUM_USE_DECLARATIONS {
                return Err((
                    PlotError::InvalidSyntax(alloc::format!(
                        "source exceeds the {MAXIMUM_USE_DECLARATIONS}-import bound"
                    )),
                    self.line_span(self.lines[self.index]),
                ));
            }
            self.index += 1;
            self.skip_empty();
        }
        if let Some(startup) = startup {
            let header = SyntaxDocument::new(
                self.source.into(),
                Vec::new(),
                uses.clone(),
                standard_glyphs,
                SyntaxDefinitions::default(),
                Vec::new(),
            );
            let scope = crate::resolve_glyph_notation_scope(&header, startup)
                .map_err(|refusal| (PlotError::InvalidSyntax(refusal.message), refusal.span))?;
            if scope.bindings().next().is_some() {
                self.glyph_scope = Some(scope);
            }
        }
        while self.index < self.lines.len() {
            let (text, _) = self.lines[self.index].statement();
            if text.starts_with("glyph notation ") {
                if glyph_notations.len() == crate::MAXIMUM_TYPED_LITERAL_FAMILIES {
                    return Err((
                        PlotError::InvalidSyntax(
                            "glyph notation declaration limit exceeded".into(),
                        ),
                        self.line_span(self.lines[self.index]),
                    ));
                }
                glyph_notations.push(glyph_notation::parse(&mut self)?);
            } else if text.starts_with("type ") {
                types.push(self.parse_type_declaration()?);
            } else if text.starts_with("form ") {
                type_forms.push(parse_type_form(&mut self)?);
            } else if text.starts_with("plot ") {
                plots.push(self.parse_plot()?);
            } else if text.starts_with("host ") {
                constructions.push(parse_construction(
                    &mut self,
                    ConstructionRole::Host,
                    "host",
                )?);
            } else if text.starts_with("body ") {
                constructions.push(parse_construction(
                    &mut self,
                    ConstructionRole::Body,
                    "body",
                )?);
            } else if text.starts_with("pack ") {
                packages.push(parse_pack(&mut self)?);
            } else {
                return Err((
                    PlotError::InvalidSyntax(
                        "expected 'type NAME', 'form ID', 'plot NAME', 'host NAME', 'body NAME', or 'pack PATH' definition"
                            .into(),
                    ),
                    self.line_span(self.lines[self.index]),
                ));
            }
            self.skip_empty();
        }
        if types.is_empty()
            && type_forms.is_empty()
            && glyph_notations.is_empty()
            && plots.is_empty()
            && constructions.is_empty()
            && packages.is_empty()
        {
            return Err((PlotError::IncompletePlot, eof_span(self.source)));
        }
        if !packages.is_empty()
            && (!types.is_empty()
                || !type_forms.is_empty()
                || !glyph_notations.is_empty()
                || !plots.is_empty()
                || !constructions.is_empty()
                || !uses.is_empty()
                || packages.len() != 1)
        {
            return Err((
                PlotError::InvalidSyntax(
                    "pack.conduit contains exactly one pack declaration and no type, form, plot, host, body, or with declarations".into(),
                ),
                packages[0].span,
            ));
        }
        Ok(ParsedSurface {
            uses,
            standard_glyphs,
            types,
            type_forms,
            glyph_notations,
            plots,
            constructions,
            packages,
        })
    }

    fn parse_use(
        &self,
        import: &str,
        line: &str,
        start: usize,
    ) -> Result<Vec<UseDeclaration>, (PlotError, Span)> {
        let import = import.trim();
        if import.is_empty() {
            return Err(self.invalid_statement(line, start));
        }
        if let Some(open) = import.find("/{") {
            let prefix = &import[..open];
            let members = import[open + 2..].strip_suffix('}').ok_or_else(|| {
                (
                    PlotError::InvalidSyntax("grouped with requires a final '}'".into()),
                    self.line_span(self.lines[self.index]),
                )
            })?;
            if !is_source_import_path(prefix) || members.trim().is_empty() {
                return Err(self.invalid_statement(line, start));
            }
            let mut declarations = Vec::new();
            for member in split_top_level(members, ',') {
                let member = member.trim();
                if !is_name(member) {
                    return Err(self.invalid_statement(line, start));
                }
                let path = alloc::format!("{prefix}/{member}");
                let member_offset = start + line.find(member).unwrap_or(0);
                declarations.push(UseDeclaration {
                    path,
                    path_span: self.span(member_offset, member_offset + member.len()),
                    alias: self.spanned(member, member_offset),
                    span: self.line_span(self.lines[self.index]),
                });
            }
            return Ok(declarations);
        }
        let (path, alias) = import
            .split_once(" as ")
            .map_or((import, None), |(path, alias)| {
                (path.trim(), Some(alias.trim()))
            });
        if !is_source_import_path(path)
            || alias.is_some_and(|alias| !crate::surface_lex::is_gear_name(alias))
        {
            return Err(self.invalid_statement(line, start));
        }
        let alias = alias.unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path));
        let path_offset = start + line.find(path).unwrap_or(0);
        let alias_offset = start + line.rfind(alias).unwrap_or(0);
        Ok(vec![UseDeclaration {
            path: path.to_string(),
            path_span: self.span(path_offset, path_offset + path.len()),
            alias: self.spanned(alias, alias_offset),
            span: self.line_span(self.lines[self.index]),
        }])
    }
}

#[cfg(test)]
mod tests;
