//! Immutable physical facts captured before closed numerical checking.
use super::*;
use crate::{ExpressionSyntax as E, StartupCatalog};
pub(super) struct PhysicalFacts {
    pub types: BTreeMap<String, CheckedExpressionType>,
    pub literals: BTreeMap<(usize, usize), Vec<u8>>,
    pub constructors: BTreeMap<(usize, usize), conduit_core::Unit>,
}
impl PhysicalFacts {
    pub fn collect(syntax: &mut E, catalog: &StartupCatalog) -> Result<Self, String> {
        let mut facts = Self {
            types: BTreeMap::new(),
            literals: BTreeMap::new(),
            constructors: BTreeMap::new(),
        };
        facts.walk(syntax, catalog)?;
        Ok(facts)
    }
    fn walk(&mut self, syntax: &mut E, catalog: &StartupCatalog) -> Result<(), String> {
        if let Some(source) = symbol(syntax) {
            if let Some(crate::CanonicalStartupValue::Quantity(quantity)) =
                crate::physical_declarations::value::parse_value(&source, None, catalog)
                    .map_err(|error| format!("physical literal: {error:?}"))?
            {
                let span = syntax.span();
                let quantity = quantity.value();
                let kind = conduit_core::quantity_role_info_id(quantity.family(), quantity.role())
                    .map_err(|error| format!("physical role: {error:?}"))?;
                self.types
                    .insert(source.clone(), CheckedExpressionType::semantic(&kind));
                self.literals
                    .insert((span.start, span.end), quantity.encode());
                *syntax = E::Atomic(crate::SpannedText { text: source, span });
                return Ok(());
            }
        }
        match syntax {
            E::Atomic(_) => {}
            E::Unary { operand, .. } => self.walk(operand, catalog)?,
            E::Binary { left, right, .. } => {
                self.walk(left, catalog)?;
                self.walk(right, catalog)?;
            }
            E::SemanticCall {
                kind,
                arguments,
                span,
            } => {
                if let Some(quantity) = catalog.physical.quantities.get(&kind.text) {
                    if arguments.len() != 2 {
                        return Err("quantity constructor requires two operands".into());
                    }
                    let symbol = symbol(&arguments[1]).ok_or_else(|| {
                        "constructor Unit must be a captured catalogue value".to_string()
                    })?;
                    let unit = catalog
                        .physical
                        .units
                        .get(&symbol)
                        .cloned()
                        .ok_or_else(|| format!("unknown captured Unit {symbol}"))?;
                    if unit.family() != quantity.family || !unit.admits_role(quantity.role) {
                        return Err("constructor Unit belongs to another family".into());
                    }
                    let kind_id =
                        conduit_core::quantity_role_info_id(quantity.family, quantity.role)
                            .map_err(|error| format!("quantity role: {error:?}"))?;
                    self.types.insert(
                        format!("constructor:{}", kind.text),
                        CheckedExpressionType::semantic(&kind_id),
                    );
                    self.constructors.insert((span.start, span.end), unit);
                    self.walk(&mut arguments[0], catalog)?;
                } else {
                    for argument in arguments {
                        self.walk(argument, catalog)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
fn symbol(syntax: &E) -> Option<String> {
    match syntax {
        E::Atomic(value) => Some(value.text.clone()),
        E::Binary {
            operator: crate::BinaryOperator::Divide,
            left,
            right,
            ..
        } if left.span().end + 1 == right.span().start => {
            Some(format!("{}/{}", symbol(left)?, symbol(right)?))
        }
        _ => None,
    }
}
