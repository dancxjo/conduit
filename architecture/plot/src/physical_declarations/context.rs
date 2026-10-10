//! Source-custodied immutable catalogue overlays.
use super::*;
use crate::prelude::*;
use crate::{StartupCatalog, SyntaxCheckDiagnostic, SyntaxDocument, TypeDefinitionSyntax};

pub(crate) fn install(
    document: &SyntaxDocument,
    base: &StartupCatalog,
) -> Result<StartupCatalog, SyntaxCheckDiagnostic> {
    let has_declarations = !document.dimensions.is_empty()
        || !document.prefixes.is_empty()
        || !document.units.is_empty()
        || document
            .types
            .iter()
            .any(|t| matches!(t.definition, TypeDefinitionSyntax::Quantity(_)));
    // Only this module installs physical source custody, and it returns it after
    // the complete unit registry and every quantity role profile have checked.
    // An unchanged source set therefore already carries the admitted meaning.
    if !base.physical_sources.is_empty()
        && (!has_declarations
            || base
                .physical_sources
                .iter()
                .any(|source| source.source_document_id() == document.source_document_id()))
    {
        return Ok(base.clone());
    }
    let mut catalogue = base.clone();
    if catalogue.physical_sources.is_empty() {
        catalogue
            .physical_sources
            .push(crate::parse_syntax_document(
                conduit_core::BUILTIN_PHYSICAL_SOURCE,
            ));
    }
    if has_declarations
        && !catalogue
            .physical_sources
            .iter()
            .any(|source| source.source_document_id() == document.source_document_id())
    {
        catalogue.physical_sources.push(document.clone());
    }
    if catalogue.physical_sources.len() > 64
        || catalogue
            .physical_sources
            .iter()
            .map(|source| source.round_trip().len())
            .sum::<usize>()
            > 1024 * 1024
    {
        let span = catalogue.physical_sources[0].dimensions[0].span;
        return Err(SyntaxCheckDiagnostic {
            code: "CND-FRM-058",
            span,
            message: "physical source custody exceeds its bounded document profile".into(),
        });
    }
    let mut dimensions = Vec::new();
    let mut prefixes = Vec::new();
    let mut quantities = Vec::new();
    let mut units = Vec::new();
    for source in &catalogue.physical_sources {
        if let Some(error) = source.diagnostics.first() {
            return Err(SyntaxCheckDiagnostic {
                code: error.code,
                span: error.span,
                message: error.message.clone(),
            });
        }
        dimensions.extend(source.dimensions.iter().cloned());
        prefixes.extend(source.prefixes.iter().cloned());
        units.extend(source.units.iter().cloned());
        quantities.extend(source.types.iter().filter_map(|t| match &t.definition {
            TypeDefinitionSyntax::Quantity(q) => Some((t.name.clone(), q.clone())),
            _ => None,
        }));
    }
    catalogue.physical = check_physical_declarations(&dimensions, &prefixes, &quantities, &units)
        .map_err(|error| SyntaxCheckDiagnostic {
        code: "CND-FRM-058",
        span: error.span,
        message: error.message,
    })?;
    for (name, role) in catalogue.physical.quantities.clone() {
        let leaf =
            conduit_core::StructuredInfoType::leaf(role.leaf_kind.clone()).map_err(|reason| {
                SyntaxCheckDiagnostic {
                    code: "CND-FRM-058",
                    span: role.span,
                    message: format!("invalid physical role profile: {reason:?}"),
                }
            })?;
        catalogue
            .ensure_structured_type(name.clone(), leaf)
            .map_err(|message| SyntaxCheckDiagnostic {
                code: "CND-FRM-058",
                span: role.span,
                message,
            })?;
    }
    Ok(catalogue)
}

/// Original authored declaration retained beside a resolved immutable value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalUnitSourceOrigin {
    pub source_document_id: conduit_core::SourceDocumentId,
    pub declaration_symbol: String,
    pub declaration_span: Span,
}
impl StartupCatalog {
    pub fn physical_unit_source(&self, symbol: &str) -> Option<PhysicalUnitSourceOrigin> {
        let definition = self.physical.units.get(symbol)?;
        let span = self.physical.unit_spans.get(symbol)?;
        self.physical_sources.iter().find_map(|source| {
            source
                .units
                .iter()
                .find(|unit| {
                    unit.span == *span
                        && self
                            .physical
                            .units
                            .get(&unit.symbol.text)
                            .is_some_and(|root| {
                                root.family() == definition.family()
                                    && root.reference_anchor() == definition.reference_anchor()
                            })
                })
                .map(|unit| PhysicalUnitSourceOrigin {
                    source_document_id: source.source_document_id(),
                    declaration_symbol: unit.symbol.text.clone(),
                    declaration_span: unit.span,
                })
        })
    }
}
