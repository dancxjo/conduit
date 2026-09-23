use super::{checked_route_track, route_diagnostic, Resolver};
use crate::{
    CanonicalStartupValue, CheckedCanonicalCord, CheckedCanonicalGear, KindSignature, MatchedRoute,
    MatchedRoutePattern, StartupCatalog, SyntaxCheckDiagnostic,
};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::{
    StructuredInfoTypeShape, StructuredInfoValueShape, StructuredSelector,
    UnmatchedVariantDisposition,
};

pub(super) fn check_guarded(
    route: &MatchedRoute,
    catalog: &StartupCatalog,
    form_signatures: &BTreeMap<String, KindSignature>,
    resolver: &mut Resolver<'_>,
    gears: &mut Vec<CheckedCanonicalGear>,
) -> Result<Vec<CheckedCanonicalCord>, SyntaxCheckDiagnostic> {
    let otherwise = route
        .arms
        .iter()
        .enumerate()
        .filter_map(|(index, arm)| {
            matches!(arm.pattern, MatchedRoutePattern::Otherwise(_)).then_some(index)
        })
        .collect::<Vec<_>>();
    if otherwise.len() != 1 {
        return Err(route_diagnostic(
            route.span,
            "guarded routing requires exactly one final otherwise track '_'",
        ));
    }
    if otherwise[0] + 1 != route.arms.len() {
        let MatchedRoutePattern::Otherwise(span) = &route.arms[otherwise[0]].pattern else {
            unreachable!()
        };
        return Err(route_diagnostic(*span, "otherwise track must be final"));
    }
    let Some(MatchedRoutePattern::Otherwise(otherwise_span)) =
        route.arms.last().map(|arm| &arm.pattern)
    else {
        unreachable!("the unique otherwise track was checked as final")
    };
    let mut route_type = None;
    let mut route_field = None;
    let mut checked = Vec::with_capacity(route.arms.len().saturating_sub(1));
    let mut seen = BTreeSet::new();
    for arm in &route.arms[..route.arms.len() - 1] {
        let MatchedRoutePattern::Guard {
            value_type,
            field,
            expected,
            span,
        } = &arm.pattern
        else {
            return Err(route_diagnostic(
                arm.span,
                "guarded routing accepts exact guards followed by one final '_'",
            ));
        };
        if route_type.get_or_insert(value_type.text.as_str()) != &value_type.text.as_str()
            || route_field.get_or_insert(field.text.as_str()) != &field.text.as_str()
        {
            return Err(route_diagnostic(
                *span,
                "all guards in one route must test the same typed field",
            ));
        }
        let input_type = catalog
            .structured_type(&value_type.text)
            .cloned()
            .ok_or_else(|| route_diagnostic(value_type.span, "unknown guarded route type"))?;
        let StructuredInfoTypeShape::Record { fields, .. } = input_type.shape() else {
            return Err(route_diagnostic(
                value_type.span,
                "guarded routing type must be a structured record",
            ));
        };
        let field_type = fields
            .iter()
            .find(|candidate| candidate.name() == field.text)
            .map(|candidate| candidate.value_type())
            .ok_or_else(|| route_diagnostic(field.span, "unknown guarded route field"))?;
        if !matches!(field_type.shape(), StructuredInfoTypeShape::Leaf(_)) {
            return Err(route_diagnostic(
                field.span,
                "guarded route field must be a leaf value",
            ));
        }
        let value = resolver
            .resolve_expression(expected, Some(field_type))
            .map_err(|error| error.diagnostic(expected.span))?;
        let CanonicalStartupValue::Structured(value) = value else {
            return Err(route_diagnostic(
                expected.span,
                "guard value must be concrete",
            ));
        };
        let concrete = value
            .try_concrete()
            .ok_or_else(|| route_diagnostic(expected.span, "guard value must be concrete"))?;
        let StructuredInfoValueShape::Leaf(raw) = concrete.shape() else {
            return Err(route_diagnostic(
                expected.span,
                "guard value must be a leaf value",
            ));
        };
        if !seen.insert(raw.to_vec()) {
            return Err(route_diagnostic(
                expected.span,
                "duplicate or overlapping guard track",
            ));
        }
        checked.push((arm, input_type, raw.to_vec(), *span));
    }
    let input_type = checked
        .first()
        .map(|(_, value_type, _, _)| value_type.clone())
        .ok_or_else(|| {
            route_diagnostic(route.span, "guarded routing requires at least one guard")
        })?;
    let field = route_field.expect("a checked guard names one field");
    let mut cords = Vec::with_capacity(route.arms.len());
    for (arm, _, expected, span) in checked {
        let selector = StructuredSelector::record_field_values(
            input_type.clone(),
            field,
            vec![expected],
            true,
            UnmatchedVariantDisposition::Drop,
        )
        .map_err(|_| route_diagnostic(span, "invalid guarded route track"))?;
        cords.push(checked_route_track(
            route,
            arm,
            selector,
            span,
            (catalog, form_signatures, resolver, gears),
        )?);
    }
    let otherwise = route.arms.last().expect("otherwise track exists");
    let selector = StructuredSelector::record_field_values(
        input_type,
        field,
        seen.into_iter().collect(),
        false,
        UnmatchedVariantDisposition::Drop,
    )
    .map_err(|_| route_diagnostic(*otherwise_span, "invalid otherwise route track"))?;
    cords.push(checked_route_track(
        route,
        otherwise,
        selector,
        *otherwise_span,
        (catalog, form_signatures, resolver, gears),
    )?);
    Ok(cords)
}
