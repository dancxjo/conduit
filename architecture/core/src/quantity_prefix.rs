//! Immutable decimal-prefix law, separate from legacy quantity storage.
//!
//! A descriptor establishes semantic scale, not runtime representation eligibility.
//! This catalogue does not yet extend the authored literal parser. Symbols match
//! exact UTF-8 bytes: aliases are a unit-resolver policy, never Unicode folding.

use crate::{QuantityDimension, QuantityUnit};

/// Version of the reviewed prefix/base-unit policy, not a Quantity wire version.
pub const QUANTITY_PREFIX_CATALOG_ID: &str = "quantity/decimal-prefix-catalog@1";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct DecimalPrefix {
    name: &'static str,
    symbol: &'static str,
    exponent: i8,
}

impl DecimalPrefix {
    pub const fn name(self) -> &'static str {
        self.name
    }
    pub const fn symbol(self) -> &'static str {
        self.symbol
    }
    pub const fn exponent(self) -> i8 {
        self.exponent
    }

    /// Resolve only canonical SI symbols. Neither ASCII `u` nor Greek mu is
    /// admitted here; a source resolver must explicitly retain any alias.
    pub fn from_symbol(symbol: &str) -> Option<Self> {
        DECIMAL_PREFIXES
            .iter()
            .copied()
            .find(|prefix| prefix.symbol == symbol)
    }
}

/// All 24 official SI prefixes, ordered from largest to smallest.
/// Authority: https://www.bipm.org/en/measurement-units/si-prefixes
pub const DECIMAL_PREFIXES: [DecimalPrefix; 24] = [
    DecimalPrefix {
        name: "quetta",
        symbol: "Q",
        exponent: 30,
    },
    DecimalPrefix {
        name: "ronna",
        symbol: "R",
        exponent: 27,
    },
    DecimalPrefix {
        name: "yotta",
        symbol: "Y",
        exponent: 24,
    },
    DecimalPrefix {
        name: "zetta",
        symbol: "Z",
        exponent: 21,
    },
    DecimalPrefix {
        name: "exa",
        symbol: "E",
        exponent: 18,
    },
    DecimalPrefix {
        name: "peta",
        symbol: "P",
        exponent: 15,
    },
    DecimalPrefix {
        name: "tera",
        symbol: "T",
        exponent: 12,
    },
    DecimalPrefix {
        name: "giga",
        symbol: "G",
        exponent: 9,
    },
    DecimalPrefix {
        name: "mega",
        symbol: "M",
        exponent: 6,
    },
    DecimalPrefix {
        name: "kilo",
        symbol: "k",
        exponent: 3,
    },
    DecimalPrefix {
        name: "hecto",
        symbol: "h",
        exponent: 2,
    },
    DecimalPrefix {
        name: "deca",
        symbol: "da",
        exponent: 1,
    },
    DecimalPrefix {
        name: "deci",
        symbol: "d",
        exponent: -1,
    },
    DecimalPrefix {
        name: "centi",
        symbol: "c",
        exponent: -2,
    },
    DecimalPrefix {
        name: "milli",
        symbol: "m",
        exponent: -3,
    },
    DecimalPrefix {
        name: "micro",
        symbol: "µ",
        exponent: -6,
    },
    DecimalPrefix {
        name: "nano",
        symbol: "n",
        exponent: -9,
    },
    DecimalPrefix {
        name: "pico",
        symbol: "p",
        exponent: -12,
    },
    DecimalPrefix {
        name: "femto",
        symbol: "f",
        exponent: -15,
    },
    DecimalPrefix {
        name: "atto",
        symbol: "a",
        exponent: -18,
    },
    DecimalPrefix {
        name: "zepto",
        symbol: "z",
        exponent: -21,
    },
    DecimalPrefix {
        name: "yocto",
        symbol: "y",
        exponent: -24,
    },
    DecimalPrefix {
        name: "ronto",
        symbol: "r",
        exponent: -27,
    },
    DecimalPrefix {
        name: "quecto",
        symbol: "q",
        exponent: -30,
    },
];

/// Reviewed prefix position within a complete unit suffix.
/// Prefixing a power applies the exponent to the entire powered base symbol.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PrefixableUnit {
    unit: QuantityUnit,
    prefix_power: u8,
}

impl PrefixableUnit {
    pub const fn unit(self) -> QuantityUnit {
        self.unit
    }
    pub const fn prefix_power(self) -> u8 {
        self.prefix_power
    }

    pub const fn dimension(self) -> QuantityDimension {
        self.unit.dimension()
    }

    /// Exact decimal exponent relative to the unprefixed reviewed unit.
    /// No magnitude is materialized, so even cubic quetta units remain bounded.
    pub const fn composed_exponent(self, prefix: DecimalPrefix) -> i16 {
        prefix.exponent as i16 * self.prefix_power as i16
    }

    /// Admit an exact reviewed base, never an already prefixed unit.
    pub fn for_unit(unit: QuantityUnit) -> Option<Self> {
        PREFIXABLE_UNITS
            .iter()
            .copied()
            .find(|base| base.unit == unit)
    }
}

/// Explicit policy: all 24 prefixes are meaningful at these reviewed positions.
/// Gram is the mass base; kilogram cannot be prefixed again. Liter and byte are
/// approved non-SI bases. Compound prefixes apply to the numerator only, except
/// powers of meter where the prefix is raised to that power. Other legacy units
/// remain available through their existing exact suffix table, not this policy.
/// Affine Celsius/Fahrenheit and legacy milli variants require a separate
/// point/difference contract and are deliberately absent here.
pub const PREFIXABLE_UNITS: [PrefixableUnit; 19] = [
    PrefixableUnit {
        unit: QuantityUnit::Second,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::Hertz,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::Volt,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::Ampere,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::Kelvin,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::Gram,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::Meter,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::SquareMeter,
        prefix_power: 2,
    },
    PrefixableUnit {
        unit: QuantityUnit::CubicMeter,
        prefix_power: 3,
    },
    PrefixableUnit {
        unit: QuantityUnit::Liter,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::Radian,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::Byte,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::Newton,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::Joule,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::Watt,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::Pascal,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::MeterPerSecond,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::MeterPerSecondSquared,
        prefix_power: 1,
    },
    PrefixableUnit {
        unit: QuantityUnit::AmpereHour,
        prefix_power: 1,
    },
];
