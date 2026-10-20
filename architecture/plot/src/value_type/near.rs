//! One absolute physical Fore qualification, carried by the evidence Type.
use super::*;
use conduit_core::{Quantity, StructuredInfoRefusal, StructuredInfoType, ValueConstraint};

pub(super) fn qualification(
    source: &str,
    catalog: &StartupCatalog,
) -> Result<Option<(StructuredInfoType, ValueConstraint)>, StructuredInfoRefusal> {
    let Some((base, tolerance)) = crate::surface_lex::split_near_qualification(source) else {
        return Ok(None);
    };
    // Exactly one absolute qualification. Optional and Flow qualification need
    // their own admitted carrying contract and are not silently unwrapped.
    if base.trim().is_empty()
        || crate::surface_lex::split_near_qualification(tolerance).is_some()
        || tolerance.trim().is_empty()
    {
        return Err(StructuredInfoRefusal::WrongType);
    }
    let source_kind = checked_value_kind(base.trim(), catalog)?;
    let limit = Quantity::parse_plot_literal(tolerance.trim())
        .map_err(|_| StructuredInfoRefusal::WrongType)?;
    let schema = catalog
        .structured_type("NearQuantity")
        .ok_or(StructuredInfoRefusal::WrongType)?
        .clone();
    let constraint = ValueConstraint::NearQuantity {
        source_kind,
        limit: alloc::boxed::Box::new(limit),
        schema: schema.canonical_bytes()?,
    };
    CheckedValueContract::new(
        schema.profile()?.value_kind().clone(),
        conduit_core::NEAR_QUANTITY_EVIDENCE_MAX_BYTES as u32,
        alloc::vec![constraint.clone()],
    )
    .map_err(|_| StructuredInfoRefusal::WrongType)?;
    Ok(Some((schema, constraint)))
}

pub(super) fn checked_refinements(
    source_type: &str,
    refinements: &[crate::ValueRefinement],
    maximum_bytes: Option<u64>,
    value_kind: &KindId,
    span: crate::Span,
    catalog: &StartupCatalog,
) -> Result<(u32, Vec<ValueConstraint>), SyntaxCheckDiagnostic> {
    let (maximum, mut constraints) = super::refinement::checked_refinements(
        refinements,
        maximum_bytes,
        value_kind,
        span,
        catalog,
    )?;
    let near = qualification(source_type, catalog).map_err(|_| SyntaxCheckDiagnostic {
        code: "CND-FRM-057", span,
        message: "near requires one nonnegative absolute physical tolerance compatible with its source Type".into(),
    })?;
    if let Some((_, constraint)) = near {
        if !refinements.is_empty() {
            return Err(SyntaxCheckDiagnostic {
                code: "CND-FRM-057",
                span,
                message: "near cannot be combined with another value refinement in this profile"
                    .into(),
            });
        }
        constraints.push(constraint);
    }
    Ok((maximum, constraints))
}
