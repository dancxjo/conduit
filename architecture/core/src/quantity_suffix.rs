//! Bounded whole-suffix resolution over reviewed unit and prefix catalogues.
//!
//! Resolution preserves authored bytes and establishes complete checked Unit
//! descriptors over finite reviewed catalogue composition positions.

use crate::{
    CatalogUnit, DecimalPrefix, PrefixableUnit, QuantityLiteralRefusal, DECIMAL_PREFIXES,
    PREFIXABLE_UNITS,
};

/// A source spelling is retained verbatim; aliases do not change that identity.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ResolvedQuantitySuffix<'a> {
    source: &'a str,
    catalogue_unit: Option<CatalogUnit>,
    base: Option<PrefixableUnit>,
    prefix: Option<DecimalPrefix>,
    alias: QuantitySuffixAlias,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuantitySuffixAlias {
    Canonical,
    AsciiMicro,
    AsciiPower,
    AsciiMicroAndPower,
    CatalogueReviewed,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuantitySuffixRefusal {
    Literal(QuantityLiteralRefusal),
    Ambiguous,
}

impl<'a> ResolvedQuantitySuffix<'a> {
    pub fn unit(self) -> crate::Unit {
        crate::Unit::from_resolved(self)
    }
    pub const fn source(self) -> &'a str {
        self.source
    }

    /// Optional named catalogue entry; generalized prefixes need no extra entry.
    pub const fn catalogue_unit(self) -> Option<CatalogUnit> {
        self.catalogue_unit
    }

    pub const fn base(self) -> Option<PrefixableUnit> {
        self.base
    }

    pub const fn prefix(self) -> Option<DecimalPrefix> {
        self.prefix
    }

    pub const fn alias(self) -> QuantitySuffixAlias {
        self.alias
    }

    /// Relative to the reviewed base, not its physical canonical reference.
    pub const fn decimal_exponent(self) -> Option<i16> {
        match (self.base, self.prefix) {
            (Some(base), Some(prefix)) => Some(base.composed_exponent(prefix)),
            (Some(_), None) => Some(0),
            _ => None,
        }
    }

    /// Resolve complete suffixes deterministically; expected Type never chooses
    /// a decomposition. At most 19 * 25 reviewed combinations are considered.
    /// ASCII `u` and powered `m2`/`m3` are explicit source aliases. Greek mu and
    /// compatibility normalization are not admitted.
    pub fn resolve(source: &'a str) -> Result<Self, QuantitySuffixRefusal> {
        let catalogue = CatalogUnit::from_plot_suffix(source);
        if matches!(
            catalogue,
            Err(QuantityLiteralRefusal::MissingUnit
                | QuantityLiteralRefusal::NonCanonicalUnit { .. })
        ) {
            return Err(QuantitySuffixRefusal::Literal(catalogue.unwrap_err()));
        }
        let mut resolved = None;
        for base in PREFIXABLE_UNITS {
            for prefix in core::iter::once(None).chain(DECIMAL_PREFIXES.iter().copied().map(Some)) {
                if let Some(alias) = matches_complete_source(source, base, prefix) {
                    if resolved.is_some() {
                        return Err(QuantitySuffixRefusal::Ambiguous);
                    }
                    resolved = Some(Self {
                        source,
                        catalogue_unit: catalogue.ok(),
                        base: Some(base),
                        prefix,
                        alias,
                    });
                }
            }
        }
        match (resolved, catalogue) {
            (Some(resolved), _) => Ok(resolved),
            (None, Ok(unit)) => Ok(Self {
                source,
                catalogue_unit: Some(unit),
                base: None,
                prefix: None,
                alias: if source == unit.plot_suffix() {
                    QuantitySuffixAlias::Canonical
                } else {
                    QuantitySuffixAlias::CatalogueReviewed
                },
            }),
            (None, Err(refusal)) => Err(QuantitySuffixRefusal::Literal(refusal)),
        }
    }
}

fn matches_complete_source(
    source: &str,
    base: PrefixableUnit,
    prefix: Option<DecimalPrefix>,
) -> Option<QuantitySuffixAlias> {
    let (suffix, micro_alias) = match prefix {
        None => (source, false),
        Some(prefix) => {
            if let Some(suffix) = source.strip_prefix(prefix.symbol()) {
                (suffix, false)
            } else if prefix.symbol() == "µ" {
                (source.strip_prefix('u')?, true)
            } else {
                return None;
            }
        }
    };
    let power_alias = if suffix == base.catalogue_unit().plot_suffix() {
        false
    } else {
        match (base.catalogue_unit(), suffix) {
            (CatalogUnit::SquareMeter, "m2")
            | (CatalogUnit::CubicMeter, "m3")
            | (CatalogUnit::MeterPerSecondSquared, "m/s2") => true,
            _ => return None,
        }
    };
    Some(match (micro_alias, power_alias) {
        (false, false) => QuantitySuffixAlias::Canonical,
        (true, false) => QuantitySuffixAlias::AsciiMicro,
        (false, true) => QuantitySuffixAlias::AsciiPower,
        (true, true) => QuantitySuffixAlias::AsciiMicroAndPower,
    })
}
