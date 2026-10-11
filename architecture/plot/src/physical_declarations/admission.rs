//! Source admission before portable carrier construction. References are
//! resolved from a complete declaration set, so order does not affect meaning.
use super::*;
use crate::prelude::*;
use alloc::collections::{BTreeMap, BTreeSet};

pub(crate) const MAXIMUM_PHYSICAL_DECLARATIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedPrefixGroups {
    pub decimal_exponents: Vec<i8>,
    pub binary_exponents: Vec<u8>,
    /// Declaration index -> canonical declaration index. Keeps source spellings
    /// distinct from the canonical symbols used in physical value identities.
    pub canonical_prefixes: Vec<usize>,
}

pub(crate) fn admit_prefix_declarations(
    prefixes: &[PrefixDeclarationSyntax],
) -> Result<CheckedPrefixGroups> {
    if prefixes.len() > MAXIMUM_PHYSICAL_DECLARATIONS {
        return Err(refusal(
            prefixes[0].span,
            "prefix declaration count exceeds the bounded catalogue",
        ));
    }
    let mut names = BTreeMap::new();
    let mut decimal = BTreeSet::new();
    let mut binary = BTreeSet::new();
    for (index, prefix) in prefixes.iter().enumerate() {
        if names
            .insert(
                (prefix.group.text.as_str(), prefix.symbol.text.as_str()),
                index,
            )
            .is_some()
        {
            return Err(refusal(
                prefix.symbol.span,
                "duplicate prefix symbol in this group",
            ));
        }
        let exponent = check_exact_scalar(&prefix.exponent)?
            .integer()
            .ok_or_else(|| refusal(prefix.exponent.span(), "prefix exponent must be an integer"))?;
        match prefix.group.text.as_str() {
            "si" if exponent != 0 && (-30..=30).contains(&exponent) => {
                decimal.insert(exponent as i8);
            }
            "binary" if (1..=80).contains(&exponent) => {
                binary.insert(exponent as u8);
            }
            _ => {
                return Err(refusal(
                    prefix.span,
                    "unknown prefix group or out-of-bounds exponent",
                ))
            }
        }
    }
    let mut canonical_prefixes = Vec::new();
    for (index, prefix) in prefixes.iter().enumerate() {
        let mut canonical = index;
        let mut active = BTreeSet::new();
        while let Some(alias) = &prefixes[canonical].alias {
            if !active.insert(canonical) {
                return Err(refusal(alias.span, "prefix alias cycle"));
            }
            let Some(&next) = names.get(&(prefix.group.text.as_str(), alias.text.as_str())) else {
                return Err(refusal(
                    alias.span,
                    "prefix alias requires a declared symbol in the same group",
                ));
            };
            if check_exact_scalar(&prefixes[canonical].exponent)?.integer()
                != check_exact_scalar(&prefixes[next].exponent)?.integer()
            {
                return Err(refusal(alias.span, "prefix alias has a different exponent"));
            }
            canonical = next;
        }
        canonical_prefixes.push(canonical);
    }
    Ok(CheckedPrefixGroups {
        decimal_exponents: decimal.into_iter().collect(),
        binary_exponents: binary.into_iter().collect(),
        canonical_prefixes,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct PhysicalDeclarationOrder {
    /// Unit indexes in dependency-first order.
    pub units: Vec<usize>,
    /// Linear or delta Type name -> optional associated point Type name.
    pub families: BTreeMap<String, Option<String>>,
}

type Result<T> = core::result::Result<T, PhysicalDeclarationDiagnostic>;

#[cfg(test)]
pub(crate) fn admit_declaration_graph(
    dimensions: &[DimensionDeclarationSyntax],
    quantities: &[(SpannedText, QuantityDefinitionSyntax)],
    units: &[UnitDeclarationSyntax],
) -> Result<PhysicalDeclarationOrder> {
    if dimensions.len() + quantities.len() + units.len() > MAXIMUM_PHYSICAL_DECLARATIONS {
        let at = units
            .first()
            .map(|v| v.span)
            .or_else(|| quantities.first().map(|v| v.0.span))
            .or_else(|| dimensions.first().map(|v| v.span));
        if let Some(at) = at {
            return Err(refusal(
                at,
                "physical declaration count exceeds the bounded catalogue",
            ));
        }
    }
    let mut names = BTreeSet::new();
    let mut dimension_names = BTreeSet::new();
    for dimension in dimensions {
        if !names.insert(dimension.name.text.as_str()) {
            return Err(refusal(
                dimension.name.span,
                "duplicate physical declaration name",
            ));
        }
        dimension_names.insert(dimension.name.text.as_str());
    }
    let mut quantity_names = BTreeMap::new();
    let mut families = BTreeMap::new();
    for (name, quantity) in quantities {
        if !names.insert(name.text.as_str()) {
            return Err(refusal(name.span, "duplicate physical declaration name"));
        }
        quantity_names.insert(name.text.as_str(), quantity);
        if let QuantityDefinitionSyntax::Linear { dimensions, .. } = quantity {
            let mut seen = BTreeSet::new();
            for term in dimensions {
                if !dimension_names.contains(term.dimension.text.as_str()) {
                    return Err(refusal(term.dimension.span, "unknown physical dimension"));
                }
                if !seen.insert(term.dimension.text.as_str()) {
                    return Err(refusal(term.span, "duplicate physical dimension term"));
                }
                let power = check_exact_scalar(&term.power)?;
                if !power
                    .integer()
                    .is_some_and(|power| power != 0 && (-16..=16).contains(&power))
                {
                    return Err(refusal(
                        term.power.span(),
                        "dimension power must be a nonzero bounded integer",
                    ));
                }
            }
            families.insert(name.text.clone(), None);
        }
    }
    for (name, quantity) in quantities {
        if let QuantityDefinitionSyntax::Point {
            difference_type, ..
        } = quantity
        {
            let Some(QuantityDefinitionSyntax::Linear { .. }) =
                quantity_names.get(difference_type.text.as_str())
            else {
                return Err(refusal(
                    difference_type.span,
                    "point quantity requires a declared linear difference Type",
                ));
            };
            let point = families
                .get_mut(&difference_type.text)
                .expect("linear family admitted");
            if point.replace(name.text.clone()).is_some() {
                return Err(refusal(
                    difference_type.span,
                    "difference Type belongs to more than one point family",
                ));
            }
        }
    }
    let family = |name: &str| -> Option<String> {
        match quantity_names.get(name)? {
            QuantityDefinitionSyntax::Linear { .. } => Some(name.into()),
            QuantityDefinitionSyntax::Point {
                difference_type, ..
            } => Some(difference_type.text.clone()),
        }
    };
    let mut unit_names = BTreeMap::new();
    let mut roots = BTreeMap::new();
    for (index, unit) in units.iter().enumerate() {
        if !names.insert(unit.symbol.text.as_str()) {
            return Err(refusal(
                unit.symbol.span,
                "unit symbol conflicts with another declaration",
            ));
        }
        let Some(unit_family) = family(&unit.quantity_type.text) else {
            return Err(refusal(
                unit.quantity_type.span,
                "unit requires a declared quantity Type",
            ));
        };
        let is_point = matches!(
            quantity_names[unit.quantity_type.text.as_str()],
            QuantityDefinitionSyntax::Point { .. }
        );
        if is_point != unit.difference.is_some() {
            return Err(refusal(
                unit.span,
                "point units declare a delta relationship; linear units cannot declare one",
            ));
        }
        if !is_point && unit.transform.offset.is_some() {
            return Err(refusal(
                unit.transform.span,
                "linear units cannot declare an affine offset",
            ));
        }
        let scale = check_exact_scalar(&unit.transform.scale)?;
        if scale.numerator <= 0 {
            return Err(refusal(
                unit.transform.scale.span(),
                "unit scale must be positive",
            ));
        }
        if let Some(delta) = &unit.difference {
            if delta.quantity.text != unit_family {
                return Err(refusal(
                    delta.quantity.span,
                    "delta quantity must match the point family's difference Type",
                ));
            }
            if check_exact_scalar(&delta.transform.scale)?.numerator <= 0 {
                return Err(refusal(
                    delta.transform.scale.span(),
                    "delta unit scale must be positive",
                ));
            }
        }
        if let Some(power) = &unit.prefix_power {
            let power_value = check_exact_scalar(power)?;
            if !power_value
                .integer()
                .is_some_and(|power| (1..=8).contains(&power))
            {
                return Err(refusal(
                    power.span(),
                    "prefix power must be an integer between one and eight",
                ));
            }
            if !unit.prefixes.si && !unit.prefixes.binary {
                return Err(refusal(
                    power.span(),
                    "prefix power requires an explicitly enabled prefix policy",
                ));
            }
        }
        let origin = match &unit.transform.reference {
            UnitReferenceSyntax::Origin(_) => Some(None),
            UnitReferenceSyntax::NamedOrigin { name, .. } => Some(Some(name.text.clone())),
            UnitReferenceSyntax::Unit(_) => None,
        };
        if let Some(origin) = origin {
            if roots.insert((unit_family.clone(), origin), index).is_some() {
                return Err(refusal(
                    unit.span,
                    "quantity family declares multiple root units",
                ));
            }
        }
        unit_names.insert(unit.symbol.text.as_str(), index);
    }
    let mut dependencies = alloc::vec![Vec::new();units.len()];
    for (index, unit) in units.iter().enumerate() {
        let unit_family = family(&unit.quantity_type.text).expect("unit family admitted");
        if !roots.keys().any(|(family, _)| family == &unit_family) {
            return Err(refusal(unit.span, "quantity family has no root unit"));
        }
        let transforms = core::iter::once(&unit.transform)
            .chain(unit.difference.iter().map(|delta| &delta.transform));
        for transform in transforms {
            if let UnitReferenceSyntax::Unit(reference) = &transform.reference {
                let Some(&dependency) = unit_names.get(reference.text.as_str()) else {
                    return Err(refusal(reference.span, "unknown referenced unit"));
                };
                if family(&units[dependency].quantity_type.text) != Some(unit_family.clone()) {
                    return Err(refusal(
                        reference.span,
                        "referenced unit belongs to another quantity family",
                    ));
                }
                dependencies[index].push(dependency);
            }
        }
    }
    fn visit(
        index: usize,
        dependencies: &[Vec<usize>],
        states: &mut [u8],
        order: &mut Vec<usize>,
        units: &[UnitDeclarationSyntax],
    ) -> Result<()> {
        match states[index] {
            1 => return Err(refusal(units[index].span, "unit definition cycle")),
            2 => return Ok(()),
            _ => {}
        }
        states[index] = 1;
        for &dependency in &dependencies[index] {
            visit(dependency, dependencies, states, order, units)?;
        }
        states[index] = 2;
        order.push(index);
        Ok(())
    }
    let mut states = alloc::vec![0;units.len()];
    let mut order = Vec::new();
    for index in 0..units.len() {
        visit(index, &dependencies, &mut states, &mut order, units)?;
    }
    Ok(PhysicalDeclarationOrder {
        units: order,
        families,
    })
}
