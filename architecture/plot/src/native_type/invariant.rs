//! Checked pure laws over one complete native record value.

use super::*;

pub(super) fn compile(
    declaration: &TypeSyntax,
    value_type: &conduit_core::StructuredInfoType,
    catalog: &StartupCatalog,
) -> Result<Vec<crate::PortableExpressionProgram>, SyntaxCheckDiagnostic> {
    let input_kind = value_type
        .profile()
        .map_err(|error| {
            diagnostic(
                declaration.span,
                alloc::format!("invalid Type structure: {error:?}"),
            )
        })?
        .value_kind()
        .clone();
    let input = crate::CheckedExpressionType::Semantic(input_kind.clone());
    let immutable_values = alloc::collections::BTreeMap::new();
    let literal_types = alloc::collections::BTreeMap::new();
    let numeric_types = alloc::collections::BTreeSet::new();
    let semantic_kinds = alloc::collections::BTreeMap::new();
    let mut structured_types = catalog.structured_types_by_value_kind().map_err(|error| {
        diagnostic(
            declaration.span,
            alloc::format!("invalid Type structure: {error:?}"),
        )
    })?;
    structured_types.insert(input_kind, value_type.clone());

    let mut invariants = declaration.invariants.iter().collect::<Vec<_>>();
    invariants
        .sort_by_key(|invariant| crate::syntax_identity::canonical_expression(&invariant.syntax));
    for duplicate in invariants.windows(2) {
        if crate::syntax_identity::canonical_expression(&duplicate[0].syntax)
            == crate::syntax_identity::canonical_expression(&duplicate[1].syntax)
        {
            return Err(diagnostic(
                duplicate[1].span,
                "a Type where law is declared more than once".into(),
            ));
        }
    }
    invariants
        .into_iter()
        .map(|invariant| {
            let checked = crate::expression_check::check_expression_as(
                &invariant.syntax,
                Some(&crate::CheckedExpressionType::semantic(
                    conduit_core::BOOL_INFO_ID,
                )),
                &crate::ExpressionTypeContext {
                    glyph_values: None,
                    input: &input,
                    immutable_values: &immutable_values,
                    structured_types: &structured_types,
                    literal_types: &literal_types,
                    numeric_types: &numeric_types,
                    semantic_kinds: &semantic_kinds,
                },
            )
            .map_err(|error| {
                diagnostic(
                    error.span,
                    alloc::format!("invalid Type where law: {}", error.message),
                )
            })?;
            if checked.value_type
                != crate::CheckedExpressionType::semantic(conduit_core::BOOL_INFO_ID)
            {
                return Err(diagnostic(
                    invariant.span,
                    "a Type where law must produce Boolean".into(),
                ));
            }
            crate::PortableExpressionProgram::from_checked(&checked).map_err(|error| {
                diagnostic(
                    invariant.span,
                    alloc::format!("invalid Type where law: {error:?}"),
                )
            })
        })
        .collect()
}
