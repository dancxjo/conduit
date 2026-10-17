//! Bounded dimension declaration graph and normalized algebra admission.
use super::*;
use crate::{BinaryOperator, ExpressionSyntax};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{Dimension, DimensionDefinition};

const MAXIMUM_DEPTH: usize = 32;
const MAXIMUM_WORK: usize = 65_536;
type Result<T> = core::result::Result<T, PhysicalDeclarationDiagnostic>;

pub(super) fn check_dimensions(
    declarations: &[DimensionDeclarationSyntax],
) -> Result<BTreeMap<String, DimensionDefinition>> {
    let mut names = BTreeMap::new();
    for (index, declaration) in declarations.iter().enumerate() {
        conduit_core::DefinitionName::new(&declaration.name.text).map_err(|reason| {
            refusal(
                declaration.name.span,
                alloc::format!("invalid dimension name: {reason:?}"),
            )
        })?;
        if names
            .insert(declaration.name.text.as_str(), index)
            .is_some()
        {
            return Err(refusal(
                declaration.name.span,
                "duplicate physical dimension",
            ));
        }
    }
    let mut resolver = Resolver {
        declarations,
        names,
        states: alloc::vec![0; declarations.len()],
        values: BTreeMap::new(),
        work: 0,
    };
    for index in 0..declarations.len() {
        resolver.declaration(index, 0)?;
    }
    Ok(resolver.values)
}

struct Resolver<'a> {
    declarations: &'a [DimensionDeclarationSyntax],
    names: BTreeMap<&'a str, usize>,
    states: Vec<u8>,
    values: BTreeMap<String, DimensionDefinition>,
    work: usize,
}
impl Resolver<'_> {
    fn admit_work(&mut self, span: Span, depth: usize) -> Result<()> {
        self.work += 1;
        if depth > MAXIMUM_DEPTH || self.work > MAXIMUM_WORK {
            return Err(refusal(
                span,
                "dimension law exceeds finite depth/work admission",
            ));
        }
        Ok(())
    }
    fn declaration(&mut self, index: usize, depth: usize) -> Result<DimensionDefinition> {
        let declaration = &self.declarations[index];
        self.admit_work(declaration.span, depth)?;
        match self.states[index] {
            1 => return Err(refusal(declaration.span, "dimension definition cycle")),
            2 => return Ok(self.values[&declaration.name.text]),
            _ => {}
        }
        self.states[index] = 1;
        let value = if let Some(law) = &declaration.definition {
            self.expression(law, depth + 1)?
        } else {
            Dimension::basis(&declaration.name.text)
                .map_err(|reason| {
                    refusal(
                        declaration.span,
                        alloc::format!("invalid dimension: {reason:?}"),
                    )
                })?
                .definition()
        };
        self.values.insert(declaration.name.text.clone(), value);
        self.states[index] = 2;
        Ok(value)
    }
    fn expression(&mut self, law: &ExpressionSyntax, depth: usize) -> Result<DimensionDefinition> {
        self.admit_work(law.span(), depth)?;
        match law {
            ExpressionSyntax::Atomic(name) => {
                let index = *self.names.get(name.text.as_str())
                    .ok_or_else(|| refusal(name.span, "unknown physical dimension"))?;
                self.declaration(index, depth + 1)
            }
            ExpressionSyntax::Binary { operator, left, right, span }
                if matches!(operator, BinaryOperator::Multiply | BinaryOperator::Divide) => {
                let left = self.expression(left, depth + 1)?;
                let right = self.expression(right, depth + 1)?;
                let result = if *operator == BinaryOperator::Multiply { left.product(right) } else { left.quotient(right) };
                result.map_err(|reason| refusal(*span, alloc::format!("invalid dimension composition: {reason:?}")))
            }
            ExpressionSyntax::Record { fields, .. } => {
                let mut names = BTreeSet::new();
                let mut result = DimensionDefinition::DIMENSIONLESS;
                for field in fields {
                    if field.punned || !names.insert(field.name.text.as_str()) {
                        return Err(refusal(field.span, "dimension powers must be explicit and unique"));
                    }
                    let index = *self.names.get(field.name.text.as_str())
                        .ok_or_else(|| refusal(field.name.span, "unknown physical dimension"))?;
                    let power = checked_power(&field.value)?;
                    let value = self.declaration(index, depth + 1)?.power(power)
                        .map_err(|reason| refusal(field.span, alloc::format!("invalid dimension power: {reason:?}")))?;
                    result = result.product(value)
                        .map_err(|reason| refusal(field.span, alloc::format!("invalid dimension composition: {reason:?}")))?;
                }
                Ok(result)
            }
            _ => Err(refusal(law.span(), "dimension laws accept declared names, multiplication, division or exact power records")),
        }
    }
}
fn checked_power(expression: &ExpressionSyntax) -> Result<i8> {
    check_exact_scalar(expression)?
        .integer()
        .and_then(|power| i8::try_from(power).ok())
        .filter(|power| *power != 0 && (-16..=16).contains(power))
        .ok_or_else(|| {
            refusal(
                expression.span(),
                "dimension power must be a nonzero bounded integer",
            )
        })
}
pub(super) fn quantity_dimension(
    terms: &[DimensionPowerSyntax],
    dimensions: &BTreeMap<String, DimensionDefinition>,
) -> Result<DimensionDefinition> {
    let mut result = DimensionDefinition::DIMENSIONLESS;
    let mut names = BTreeSet::new();
    for term in terms {
        if !names.insert(term.dimension.text.as_str()) {
            return Err(refusal(term.span, "duplicate physical dimension term"));
        }
        let dimension = dimensions
            .get(&term.dimension.text)
            .ok_or_else(|| refusal(term.dimension.span, "unknown physical dimension"))?;
        let dimension = dimension
            .power(checked_power(&term.power)?)
            .map_err(|reason| {
                refusal(
                    term.span,
                    alloc::format!("invalid dimension power: {reason:?}"),
                )
            })?;
        result = result.product(dimension).map_err(|reason| {
            refusal(
                term.span,
                alloc::format!("invalid dimension law: {reason:?}"),
            )
        })?;
    }
    Ok(result)
}
