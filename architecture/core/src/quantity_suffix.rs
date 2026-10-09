//! Bounded whole-suffix resolution over reviewed unit and prefix catalogues.
//!
//! Resolution preserves authored bytes and separates recognized semantic units
//! from eligibility for the legacy one-byte unit representation.

use crate::{
    DecimalPrefix, PrefixableUnit, QuantityLiteralRefusal, QuantityUnit, DECIMAL_PREFIXES,
    PREFIXABLE_UNITS,
};

/// A source spelling is retained verbatim; aliases do not change that identity.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ResolvedQuantitySuffix<'a> {
    source: &'a str,
    legacy_unit: Option<QuantityUnit>,
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
    LegacyReviewed,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuantitySuffixRefusal {
    Literal(QuantityLiteralRefusal),
    Ambiguous,
}

impl<'a> ResolvedQuantitySuffix<'a> {
    pub const fn source(self) -> &'a str {
        self.source
    }

    /// None means recognized semantic scale without an existing legacy tag.
    pub const fn legacy_unit(self) -> Option<QuantityUnit> {
        self.legacy_unit
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
        let legacy = QuantityUnit::from_plot_suffix(source);
        if matches!(
            legacy,
            Err(QuantityLiteralRefusal::MissingUnit
                | QuantityLiteralRefusal::NonCanonicalUnit { .. })
        ) {
            return Err(QuantitySuffixRefusal::Literal(legacy.unwrap_err()));
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
                        legacy_unit: legacy.ok(),
                        base: Some(base),
                        prefix,
                        alias,
                    });
                }
            }
        }
        match (resolved, legacy) {
            (Some(resolved), _) => Ok(resolved),
            (None, Ok(unit)) => Ok(Self {
                source,
                legacy_unit: Some(unit),
                base: None,
                prefix: None,
                alias: if source == unit.plot_suffix() {
                    QuantitySuffixAlias::Canonical
                } else {
                    QuantitySuffixAlias::LegacyReviewed
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
    let power_alias = if suffix == base.unit().plot_suffix() {
        false
    } else {
        match (base.unit(), suffix) {
            (QuantityUnit::SquareMeter, "m2")
            | (QuantityUnit::CubicMeter, "m3")
            | (QuantityUnit::MeterPerSecondSquared, "m/s2") => true,
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
