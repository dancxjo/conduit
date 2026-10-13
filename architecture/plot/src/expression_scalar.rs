//! Closed ordinary Scalar admission through the shared expression checker.
mod physical;
use crate::prelude::*;
use alloc::collections::{BTreeMap, BTreeSet};
use crate::{CheckedExpressionType, ExpressionSyntax, ExpressionTypeContext};
use conduit_core::{MathValue, NumericComputation, NumericComputedResult};

/// Keeps the ordered computation alongside its exact denotation. Callers which
/// choose a realization must retain the graph, rather than only the folded value.
pub(crate) fn checked_scalar_expression(
    syntax: &ExpressionSyntax,
) -> Result<(MathValue, NumericComputation), String> {
    let graph = checked_closed_numeric_expression(syntax)?;
    match graph
        .evaluate_exact(&[])
        .map_err(|reason| format!("invalid closed Scalar: {reason:?}"))?
    {
        NumericComputedResult::Number(value)
            if value.semantic_kind == conduit_core::SCALAR_INFO_ID =>
        {
            Ok((value.value, graph))
        }
        _ => Err("closed Scalar expression has a different semantic result".into()),
    }
}

pub(crate) fn checked_closed_numeric_expression(
    syntax: &ExpressionSyntax,
) -> Result<NumericComputation, String> {
    checked_closed_numeric_expression_with_catalog(syntax, None)
}

pub(crate) fn checked_closed_numeric_expression_with_catalog(
    syntax: &ExpressionSyntax, catalog: Option<&crate::StartupCatalog>,
) -> Result<NumericComputation, String> {
    let mut admitted_syntax = syntax.clone();
    let facts = catalog.map(|catalog| physical::PhysicalFacts::collect(&mut admitted_syntax, catalog)).transpose()?;
    let scalar = CheckedExpressionType::semantic(conduit_core::SCALAR_INFO_ID);
    let immutable_values = BTreeMap::new();
    let structured_types = BTreeMap::new();
    let literal_types = facts.as_ref().map(|facts| facts.types.clone()).unwrap_or_default();
    let numeric_types = BTreeSet::new();
    let semantic_kinds = BTreeMap::new();
    let mut checked = crate::expression_check::check_expression_as(
        &admitted_syntax,
        None,
        &ExpressionTypeContext {
            input: &scalar,
            immutable_values: &immutable_values,
            structured_types: &structured_types,
            literal_types: &literal_types,
            numeric_types: &numeric_types,
            semantic_kinds: &semantic_kinds,
        },
    )
    .map_err(|reason| reason.message)?;
    if let Some(facts) = facts {
        checked.canonical_literals = facts.literals;
        checked.quantity_construction_units = facts.constructors;
        checked.numeric_frequency_references = catalog.and_then(|catalog| Some([
            catalog.physical.units.get("Hz")?.clone(), catalog.physical.units.get("turn/s")?.clone(),
        ]));
        checked.numeric_angle_reference = catalog.and_then(|catalog| catalog.physical.units.get("turn").cloned());
    }
    let graph = crate::checked_numeric_computation(&checked)
        .map_err(|reason| format!("invalid Scalar computation: {reason:?}"))?
        .ok_or_else(|| "Scalar requires an exact mathematical computation".to_string())?;
    Ok(graph)
}

pub(crate) fn checked_scalar_source(
    source: &str,
) -> Result<(MathValue, NumericComputation), String> {
    let syntax =
        crate::pure_expression::parse(source, source, 0).map_err(|(message, _)| message)?;
    checked_scalar_expression(&syntax)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scalar_denotation_and_ordered_computation_are_both_retained() {
        let (sum, graph) = checked_scalar_source("0.1 + 0.2").unwrap();
        let (decimal, literal_graph) = checked_scalar_source("0.3").unwrap();
        assert_eq!(
            sum.mathematical_equality(&decimal).unwrap(),
            conduit_core::MathematicalEquality::Equal
        );
        assert_ne!(graph.identity().unwrap(), literal_graph.identity().unwrap());
        assert!(checked_scalar_source("1 / 3").is_ok());
        assert!(checked_scalar_source("math/sqrt(2)").is_ok());
        assert!(checked_scalar_source("pi / 180").is_ok());
        assert!(checked_scalar_source("math/pow(10, 1000) + 1").is_ok());
        assert!(checked_scalar_source("1 / 0").is_err());
        assert!(checked_scalar_source("1m").is_err());
        assert!(checked_scalar_source(".").is_err());
    }
}

#[cfg(test)]
mod physical_source_tests {
    use super::*;
    fn graph(source: &str) -> Result<NumericComputation, String> {
        let document = crate::parse_syntax_document("");
        let catalog = crate::checked_physical_catalog_for_document(&document, &crate::StartupCatalog::new()).unwrap();
        let syntax = crate::pure_expression::parse(source, source, 0).unwrap();
        checked_closed_numeric_expression_with_catalog(&syntax, Some(&catalog))
    }
    #[test]
    fn exact_constructor_trig_and_frequency_bridges_capture_declared_laws() {
        let trig = graph("math/sin(Angle(pi / 2, rad))").unwrap();
        let NumericComputedResult::Number(value) = trig.evaluate_exact(&[]).unwrap() else { panic!("Scalar") };
        assert_eq!(value.value, MathValue::ONE);
        let angular = graph("math/cyclic-to-angular-frequency(1Hz)").unwrap();
        let NumericComputedResult::Number(value) = angular.evaluate_exact(&[]).unwrap() else { panic!("frequency") };
        assert_eq!(value.value, MathValue::ONE);
        assert_eq!(value.unit.unwrap().symbol(), "turn/s");
        let cyclic = graph("math/angular-to-cyclic-frequency(AngularFrequency(2 * pi, rad/s))").unwrap();
        let NumericComputedResult::Number(value) = cyclic.evaluate_exact(&[]).unwrap() else { panic!("frequency") };
        assert_eq!(value.value, MathValue::ONE);
        assert!(graph("math/sin(42)").is_err());
        assert!(graph("math/cyclic-to-angular-frequency(TemperatureDelta(1, K))").is_err());
    }
    #[test]
    fn ieee_explicit_constructors_keep_rounded_source_and_raw_bits_disjoint() {
        let ieee = graph("F64(0.1) + F64(0.2) == F64(0.3)").unwrap();
        assert_eq!(ieee.nodes().iter().filter(|node| matches!(node.operation, conduit_core::NumericComputationOperation::IeeeRoundedLiteral { .. })).count(), 3);
        assert!(ieee.evaluate_exact(&[]).is_err());
        let raw = graph("ieee/binary64-bits(9223372036854775808)").unwrap();
        assert!(matches!(raw.nodes()[0].operation, conduit_core::NumericComputationOperation::IeeeLiteral { bits: 9223372036854775808 }));
    }
}
