//! Immutable checked physical meanings. Runtime admission extends this context;
//! unit resolution never edits a process-global Rust catalogue.
use super::*;
use crate::prelude::*;
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    DefinitionScalar, DimensionDefinition, DimensionTerm, KindId, PrefixPolicy,
    QuantityFamilyDefinition, QuantityRole, QuantityRoles, UnitDefinition,
};

const MAXIMUM_RESOLVED_UNITS: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedQuantityRole {
    pub family: QuantityFamilyDefinition,
    pub role: QuantityRole,
    /// Intrinsic capsule-family+role validation survives record and port lowering.
    pub leaf_kind: KindId,
    pub span: Span,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CheckedPhysicalCatalogue {
    pub dimensions: BTreeMap<String, [u8; 32]>,
    pub quantities: BTreeMap<String, CheckedQuantityRole>,
    pub units: BTreeMap<String, UnitDefinition>,
    /// Accepted source spellings map to immutable canonical capsule spellings.
    pub aliases: BTreeMap<String, String>,
    pub unit_spans: BTreeMap<String, Span>,
}

type Result<T> = core::result::Result<T, PhysicalDeclarationDiagnostic>;

fn scalar(value: &crate::ExpressionSyntax) -> Result<DefinitionScalar> {
    let exact = check_exact_scalar(value)?;
    DefinitionScalar::new(exact.numerator, exact.denominator, exact.decimal_exponent).map_err(
        |reason| {
            refusal(
                value.span(),
                alloc::format!("invalid exact physical scalar: {reason:?}"),
            )
        },
    )
}

fn zero() -> DefinitionScalar {
    DefinitionScalar::new(0, 1, 0).expect("bounded zero")
}

/// Complete declaration admission. The caller supplies source declarations from
/// the immutable imported context together with this document's additions.
pub(crate) fn check_physical_declarations(
    dimensions: &[DimensionDeclarationSyntax],
    prefixes: &[PrefixDeclarationSyntax],
    quantities: &[(SpannedText, QuantityDefinitionSyntax)],
    units: &[UnitDeclarationSyntax],
) -> Result<CheckedPhysicalCatalogue> {
    if dimensions.len() + prefixes.len() + quantities.len() + units.len()
        > admission::MAXIMUM_PHYSICAL_DECLARATIONS
    {
        let at = units
            .first()
            .map(|v| v.span)
            .or_else(|| quantities.first().map(|v| v.0.span))
            .or_else(|| dimensions.first().map(|v| v.span))
            .or_else(|| prefixes.first().map(|v| v.span));
        if let Some(at) = at {
            return Err(refusal(
                at,
                "physical declaration count exceeds the bounded catalogue",
            ));
        }
    }
    // Prefix references may name synthesized units; dependency admission below
    // handles those without requiring synthetic declarations in source.
    let prefix_groups = admit_prefix_declarations(prefixes)?;
    let mut result = CheckedPhysicalCatalogue::default();
    let mut names = BTreeSet::new();
    for dimension in dimensions {
        if !names.insert(dimension.name.text.as_str()) {
            return Err(refusal(dimension.span, "duplicate physical dimension"));
        }
        result.dimensions.insert(
            dimension.name.text.clone(),
            DimensionDefinition::anchor(&dimension.name.text).map_err(|reason| {
                refusal(
                    dimension.span,
                    alloc::format!("invalid dimension: {reason:?}"),
                )
            })?,
        );
    }
    let mut linear_dimensions = BTreeMap::new();
    let mut paired = BTreeMap::new();
    for (name, quantity) in quantities {
        if !names.insert(name.text.as_str()) {
            return Err(refusal(name.span, "duplicate physical Type"));
        }
        if let QuantityDefinitionSyntax::Linear { dimensions, .. } = quantity {
            let mut terms = Vec::new();
            for term in dimensions {
                let anchor = result
                    .dimensions
                    .get(&term.dimension.text)
                    .ok_or_else(|| refusal(term.dimension.span, "unknown physical dimension"))?;
                let power = check_exact_scalar(&term.power)?
                    .integer()
                    .and_then(|power| i8::try_from(power).ok())
                    .ok_or_else(|| {
                        refusal(
                            term.power.span(),
                            "dimension power must be a bounded integer",
                        )
                    })?;
                terms.push(DimensionTerm {
                    anchor: *anchor,
                    power,
                });
            }
            let dimension = DimensionDefinition::new(&terms).map_err(|reason| {
                refusal(
                    name.span,
                    alloc::format!("invalid dimension law: {reason:?}"),
                )
            })?;
            linear_dimensions.insert(name.text.as_str(), dimension);
        }
    }
    for (name, quantity) in quantities {
        if let QuantityDefinitionSyntax::Point {
            difference_type, ..
        } = quantity
        {
            if !linear_dimensions.contains_key(difference_type.text.as_str()) {
                return Err(refusal(
                    difference_type.span,
                    "point quantity requires a declared linear difference Type",
                ));
            }
            if paired
                .insert(difference_type.text.as_str(), name.text.as_str())
                .is_some()
            {
                return Err(refusal(
                    name.span,
                    "difference Type belongs to more than one point family",
                ));
            }
        }
    }
    for (name, quantity) in quantities {
        let (family, role) = match quantity {
            QuantityDefinitionSyntax::Linear { .. } => {
                let dimension = linear_dimensions[name.text.as_str()];
                if let Some(point) = paired.get(name.text.as_str()) {
                    (
                        QuantityFamilyDefinition::point_delta(point, &name.text, dimension),
                        QuantityRole::Delta,
                    )
                } else {
                    (
                        QuantityFamilyDefinition::new(&name.text, dimension, QuantityRoles::Linear),
                        QuantityRole::Linear,
                    )
                }
            }
            QuantityDefinitionSyntax::Point {
                difference_type, ..
            } => (
                QuantityFamilyDefinition::point_delta(
                    &name.text,
                    &difference_type.text,
                    linear_dimensions[difference_type.text.as_str()],
                ),
                QuantityRole::Point,
            ),
        };
        let family = family.map_err(|reason| {
            refusal(
                name.span,
                alloc::format!("invalid quantity family: {reason:?}"),
            )
        })?;
        let leaf_kind = conduit_core::kind_id(
            &conduit_core::quantity_role_info_id(family, role).map_err(|reason| {
                refusal(
                    name.span,
                    alloc::format!("invalid role identity: {reason:?}"),
                )
            })?,
        );
        result.quantities.insert(
            name.text.clone(),
            CheckedQuantityRole {
                family,
                role,
                leaf_kind,
                span: name.span,
            },
        );
    }
    let mut by_name = BTreeMap::new();
    let mut roots = BTreeSet::new();
    for (index, unit) in units.iter().enumerate() {
        if !names.insert(unit.symbol.text.as_str()) {
            return Err(refusal(
                unit.symbol.span,
                "unit symbol conflicts with another declaration",
            ));
        }
        let quantity = result
            .quantities
            .get(&unit.quantity_type.text)
            .ok_or_else(|| {
                refusal(
                    unit.quantity_type.span,
                    "unit requires a declared quantity Type",
                )
            })?;
        if (quantity.role == QuantityRole::Point) != unit.difference.is_some() {
            return Err(refusal(unit.span,"point units declare a delta relationship; linear and delta units cannot declare one"));
        }
        if quantity.role != QuantityRole::Point && unit.transform.offset.is_some() {
            return Err(refusal(
                unit.transform.span,
                "linear and delta units cannot declare affine offsets",
            ));
        }
        let origin = match &unit.transform.reference {
            UnitReferenceSyntax::Origin(_) => Some(None),
            UnitReferenceSyntax::NamedOrigin { name, .. } => Some(Some(name.text.clone())),
            UnitReferenceSyntax::Unit(_) => None,
        };
        if let Some(origin) = origin {
            if !roots.insert((quantity.family.identity(), origin)) {
                return Err(refusal(
                    unit.span,
                    "quantity family declares the same root origin twice",
                ));
            }
        }
        by_name.insert(unit.symbol.text.as_str(), index);
    }
    let mut states = alloc::vec![0u8;units.len()];
    let mut resolver = Resolver {
        declarations: units,
        prefixes,
        canonical_prefixes: &prefix_groups.canonical_prefixes,
        decimal_exponents: &prefix_groups.decimal_exponents,
        binary_exponents: &prefix_groups.binary_exponents,
        by_name,
        states: &mut states,
        result: &mut result,
    };
    for index in 0..units.len() {
        resolver.unit(index)?;
    }
    // Synthesize the complete finite namespace and refuse collisions even if
    // no source expression happens to use the conflicting spelling.
    for index in 0..units.len() {
        let definition = resolver.result.units[&units[index].symbol.text];
        for (prefix_index, prefix) in prefixes.iter().enumerate() {
            if !enabled(units[index].prefixes, &prefix.group.text) {
                continue;
            }
            let canonical = &prefixes[prefix_groups.canonical_prefixes[prefix_index]];
            let symbol = alloc::format!("{}{}", prefix.symbol.text, units[index].symbol.text);
            let canonical_symbol =
                alloc::format!("{}{}", canonical.symbol.text, units[index].symbol.text);
            let value = prefixed(definition, &symbol, canonical)?;
            if resolver
                .result
                .units
                .insert(symbol.clone(), value)
                .is_some()
            {
                return Err(refusal(
                    prefix.span,
                    alloc::format!(
                        "synthesized prefix unit '{symbol}' conflicts with another unit"
                    ),
                ));
            }
            resolver
                .result
                .unit_spans
                .insert(symbol.clone(), units[index].span);
            resolver.result.aliases.insert(symbol, canonical_symbol);
            if resolver.result.units.len() > MAXIMUM_RESOLVED_UNITS {
                return Err(refusal(
                    prefix.span,
                    "resolved physical catalogue exceeds its bounded unit count",
                ));
            }
        }
    }
    Ok(result)
}

fn enabled(policy: PrefixPolicySyntax, group: &str) -> bool {
    (group == "si" && policy.si) || (group == "binary" && policy.binary)
}

fn prefixed(
    definition: UnitDefinition,
    symbol: &str,
    prefix: &PrefixDeclarationSyntax,
) -> Result<UnitDefinition> {
    let exponent = check_exact_scalar(&prefix.exponent)?
        .integer()
        .expect("checked prefix exponent");
    let value = if prefix.group.text == "si" {
        definition.with_decimal_prefix(symbol, exponent as i8)
    } else {
        definition.with_binary_prefix(symbol, exponent as u8)
    };
    value.map_err(|reason| {
        refusal(
            prefix.span,
            alloc::format!("invalid source prefix unit: {reason:?}"),
        )
    })
}

struct Resolver<'a> {
    declarations: &'a [UnitDeclarationSyntax],
    prefixes: &'a [PrefixDeclarationSyntax],
    canonical_prefixes: &'a [usize],
    decimal_exponents: &'a [i8],
    binary_exponents: &'a [u8],
    by_name: BTreeMap<&'a str, usize>,
    states: &'a mut [u8],
    result: &'a mut CheckedPhysicalCatalogue,
}

impl Resolver<'_> {
    fn reference(&mut self, name: &SpannedText) -> Result<UnitDefinition> {
        if let Some(&index) = self.by_name.get(name.text.as_str()) {
            return self.unit(index);
        }
        let mut candidates = Vec::new();
        for (index, prefix) in self.prefixes.iter().enumerate() {
            let Some(tail) = name.text.strip_prefix(&prefix.symbol.text) else {
                continue;
            };
            let Some(&base) = self.by_name.get(tail) else {
                continue;
            };
            if enabled(self.declarations[base].prefixes, &prefix.group.text) {
                candidates.push((index, base));
            }
        }
        if candidates.len() != 1 {
            return Err(refusal(
                name.span,
                "unknown or ambiguous source unit reference",
            ));
        }
        let (prefix_index, base) = candidates[0];
        let definition = self.unit(base)?;
        let canonical = &self.prefixes[self.canonical_prefixes[prefix_index]];
        prefixed(definition, &name.text, canonical)
    }

    fn transform(
        &mut self,
        syntax: &UnitTransformSyntax,
        family: QuantityFamilyDefinition,
        role: QuantityRole,
        symbol: &str,
        policy: PrefixPolicy,
    ) -> Result<UnitDefinition> {
        let scale = scalar(&syntax.scale)?;
        let offset = syntax
            .offset
            .as_ref()
            .map(scalar)
            .transpose()?
            .unwrap_or_else(zero);
        let value = match &syntax.reference {
            UnitReferenceSyntax::Origin(_) => {
                UnitDefinition::new_exact_role(family, symbol, role, scale, offset, policy)
            }
            UnitReferenceSyntax::NamedOrigin { name, .. } => {
                UnitDefinition::new_exact_role(family, symbol, role, scale, offset, policy)
                    .and_then(|definition| definition.with_named_origin(&name.text))
            }
            UnitReferenceSyntax::Unit(name) => {
                let reference = self.reference(name)?;
                if reference.family() != family {
                    return Err(refusal(
                        name.span,
                        "referenced unit belongs to another quantity family",
                    ));
                }
                UnitDefinition::related_exact_role(reference, symbol, role, scale, offset, policy)
            }
        };
        value.map_err(|reason| {
            refusal(
                syntax.span,
                alloc::format!("invalid exact unit relationship: {reason:?}"),
            )
        })
    }

    fn unit(&mut self, index: usize) -> Result<UnitDefinition> {
        let declaration = &self.declarations[index];
        match self.states[index] {
            1 => return Err(refusal(declaration.span, "unit definition cycle")),
            2 => return Ok(self.result.units[&declaration.symbol.text]),
            _ => {}
        }
        self.states[index] = 1;
        let quantity = self.result.quantities[&declaration.quantity_type.text].clone();
        let power = if let Some(power) = &declaration.prefix_power {
            check_exact_scalar(power)?
                .integer()
                .ok_or_else(|| refusal(power.span(), "prefix power must be an exact integer"))?
        } else {
            1
        };
        let power = u8::try_from(power)
            .map_err(|_| refusal(declaration.span, "prefix power is outside bounded range"))?;
        if declaration.prefix_power.is_some()
            && !declaration.prefixes.si
            && !declaration.prefixes.binary
        {
            return Err(refusal(
                declaration.span,
                "prefix power requires explicit prefix opt-in",
            ));
        }
        let policy = PrefixPolicy::new(
            if declaration.prefixes.si {
                self.decimal_exponents
            } else {
                &[]
            },
            if declaration.prefixes.binary {
                self.binary_exponents
            } else {
                &[]
            },
            power,
        )
        .map_err(|reason| {
            refusal(
                declaration.span,
                alloc::format!("invalid prefix policy: {reason:?}"),
            )
        })?;
        let value = self.transform(
            &declaration.transform,
            quantity.family,
            quantity.role,
            &declaration.symbol.text,
            policy,
        )?;
        if let Some(delta) = &declaration.difference {
            let delta_quantity = self
                .result
                .quantities
                .get(&delta.quantity.text)
                .ok_or_else(|| refusal(delta.quantity.span, "unknown difference quantity Type"))?;
            if delta_quantity.family != quantity.family
                || delta_quantity.role != QuantityRole::Delta
            {
                return Err(refusal(
                    delta.quantity.span,
                    "difference quantity does not match the point family",
                ));
            }
            let mut derivative = self.transform(
                &delta.transform,
                quantity.family,
                QuantityRole::Delta,
                &declaration.symbol.text,
                policy,
            )?;
            let paired_root = match (&declaration.transform.reference, &delta.transform.reference) {
                (UnitReferenceSyntax::Origin(_), UnitReferenceSyntax::Origin(_)) => true,
                (
                    UnitReferenceSyntax::NamedOrigin { name: point, .. },
                    UnitReferenceSyntax::NamedOrigin { name: delta, .. },
                ) => point.text == delta.text,
                _ => false,
            };
            if paired_root {
                derivative = derivative.with_reference_origin(value).map_err(|reason| {
                    refusal(
                        delta.span,
                        alloc::format!("invalid paired root origin: {reason:?}"),
                    )
                })?;
            }
            if !value.exact_scale().equivalent(derivative.exact_scale())
                || value.reference_anchor() != derivative.reference_anchor()
            {
                return Err(refusal(delta.span,"declared difference relationship does not equal the point unit's exact derivative law"));
            }
        }
        self.result
            .units
            .insert(declaration.symbol.text.clone(), value);
        self.result
            .unit_spans
            .insert(declaration.symbol.text.clone(), declaration.span);
        self.states[index] = 2;
        Ok(value)
    }
}
