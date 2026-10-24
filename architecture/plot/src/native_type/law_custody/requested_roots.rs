//! Explicit root selection before optional full-document custody retention.
use super::*;
use crate::{Span, SyntaxCheckDiagnostic, SyntaxDocument};
use alloc::collections::BTreeSet;

pub(crate) struct Request<'a> {
    names: BTreeSet<&'a str>,
}
fn failure(
    name: &str,
    cause: NativeLawCustodyRefusal,
    span: Option<Span>,
) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-058",
        span: span.unwrap_or(Span {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
        }),
        message: alloc::format!("requested Native law custody root '{name}' refused: {cause:?}"),
    }
}
pub(crate) fn preflight<'a>(names: &[&'a str]) -> Result<Request<'a>, SyntaxCheckDiagnostic> {
    if names.len() > 4096 {
        return Err(failure(
            "<request>",
            NativeLawCustodyRefusal::Capacity,
            None,
        ));
    }
    let mut bytes = 0usize;
    let mut selected = BTreeSet::new();
    for name in names {
        bytes = bytes.saturating_add(name.len());
        if name.is_empty() || name.len() > 4096 || bytes > MAXIMUM_BYTES {
            // Do not copy an unbounded offending name into a diagnostic.
            return Err(failure(
                "<request-name>",
                NativeLawCustodyRefusal::Capacity,
                None,
            ));
        }
        if !selected.insert(*name) {
            return Err(failure(
                name,
                NativeLawCustodyRefusal::DuplicateImport,
                None,
            ));
        }
    }
    Ok(Request { names: selected })
}
pub(crate) fn collect(
    syntax: &SyntaxDocument,
    base: &StartupCatalog,
    declarations: &[CheckedNativeType],
    request: &Request<'_>,
) -> Result<BTreeMap<String, NativeLawWitness>, SyntaxCheckDiagnostic> {
    let mut result = BTreeMap::new();
    let mut budget = Budget::default();
    for name in &request.names {
        let span = syntax
            .types
            .iter()
            .find(|ty| ty.name.text == *name)
            .map(|ty| ty.name.span);
        let attempt = || {
            let checked = declarations
                .iter()
                .find(|ty| ty.name == *name)
                .ok_or(NativeLawCustodyRefusal::Declaration)?;
            let source = root_source::extract(syntax, name)?;
            let closed = crate::parse_syntax_document(&source);
            let witness = collect_root(&closed, base, checked)?;
            Ok::<_, NativeLawCustodyRefusal>(witness)
        };
        let witness = attempt().map_err(|cause| failure(name, cause, span))?;
        admit_witness(&witness, 0, &mut budget).map_err(|cause| failure(name, cause, span))?;
        result.insert((*name).into(), witness);
    }
    Ok(result)
}
