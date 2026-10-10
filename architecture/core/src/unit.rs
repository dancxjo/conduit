//! Public Unit values carry source-admitted immutable physical definitions.
use crate::{
    DefinitionScalar, DimensionDefinition, PhysicalDefinitionRefusal, QuantityFamilyDefinition,
    QuantityRole, UnitDefinition, UnitPrefix, BUILTIN_PREFIX_DEFINITIONS, BUILTIN_UNIT_DEFINITIONS,
    UNIT_DEFINITION_ENCODED_LEN,
};
use alloc::string::{String, ToString};
pub const UNIT_INFO_ID: &str = "value/unit@1";
pub const UNIT_ENCODED_LEN: usize = UNIT_DEFINITION_ENCODED_LEN;
pub const UNIT_MAX_SOURCE_BYTES: usize = 128;
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Unit(UnitDefinition);
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum UnitRefusal {
    SourceTooLong,
    UnknownSymbol,
    AmbiguousSymbol,
    Definition(PhysicalDefinitionRefusal),
}
impl Unit {
    pub const fn from_resolved(value: crate::ResolvedQuantitySuffix<'_>) -> Self {
        value.unit()
    }

    /// Reference/default coordinate declared by a built-in family's ordinary
    /// source origin. Named independent origins do not replace this default.
    pub fn default_for_builtin_family(family: QuantityFamilyDefinition) -> Option<Self> {
        crate::BUILTIN_DEFAULT_UNIT_DEFINITIONS
            .iter()
            .find(|(_, unit)| unit.family().identity() == family.identity())
            .map(|(_, unit)| Self(*unit))
    }
    /// Physical Unit equality ignores spelling and prefix-admission policy.
    /// Definition identity still preserves those distinct source declarations.
    pub fn same_physical_definition(self, other: Self) -> bool {
        self.0.same_physical_definition(other.0)
    }
    pub const fn from_definition(definition: UnitDefinition) -> Self {
        Self(definition)
    }
    pub const fn definition(self) -> UnitDefinition {
        self.0
    }
    pub const fn family(self) -> QuantityFamilyDefinition {
        self.0.family()
    }
    pub const fn dimension(self) -> DimensionDefinition {
        self.0.family().dimension()
    }
    pub const fn admits_role(self, role: QuantityRole) -> bool {
        self.0.admits_role(role)
    }
    pub const fn declared_role(self) -> QuantityRole {
        self.0.declared_role()
    }
    pub const fn reference_anchor(self) -> [u8; 32] {
        self.0.reference_anchor()
    }
    pub const fn definition_identity(self) -> [u8; 32] {
        self.0.identity()
    }
    pub const fn decimal_exponent(self) -> i16 {
        self.0.decimal_exponent()
    }
    pub const fn binary_exponent(self) -> u16 {
        self.0.binary_exponent()
    }
    pub const fn offset_exponent(self) -> i16 {
        self.0.offset_exponent()
    }
    pub const fn prefix_exponent(self) -> i8 {
        match self.0.prefix() {
            UnitPrefix::Decimal(exponent) => exponent,
            _ => 0,
        }
    }
    pub const fn exact_scale(self) -> DefinitionScalar {
        self.0.exact_scale()
    }
    pub fn exact_offset(
        self,
        role: QuantityRole,
    ) -> Result<DefinitionScalar, PhysicalDefinitionRefusal> {
        self.0.exact_offset(role)
    }
    pub fn canonical_symbol(self) -> String {
        self.0.symbol().to_string()
    }
    pub fn plot_suffix(self) -> String {
        self.canonical_symbol()
    }
    pub fn symbol(&self) -> &str {
        self.0.symbol()
    }
    pub fn matches_source_evidence(&self, source: &str) -> bool {
        source.as_bytes() == self.0.symbol().as_bytes()
    }
    pub fn with_symbol_alias(self, symbol: &str) -> Result<Self, UnitRefusal> {
        self.0
            .with_symbol_alias(symbol)
            .map(Self)
            .map_err(UnitRefusal::Definition)
    }
    pub fn semantic_id(self) -> String {
        use core::fmt::Write;
        let mut result = String::from("physical/unit/");
        for byte in self.definition_identity() {
            write!(result, "{byte:02x}").expect("String write");
        }
        result
    }
    pub fn encode(self) -> [u8; UNIT_ENCODED_LEN] {
        self.0.encode()
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, UnitRefusal> {
        UnitDefinition::decode(bytes)
            .map(Self)
            .map_err(UnitRefusal::Definition)
    }
    pub fn semantic_digest(self) -> [u8; 32] {
        crate::semantic_digest(UNIT_INFO_ID, &self.encode())
    }
    pub fn from_plot_suffix(source: &str) -> Result<Self, UnitRefusal> {
        Self::resolve(source)
    }
    /// Built-in convenience resolution reads only generated language data. A
    /// document's declaration environment resolves custom symbols separately.
    pub fn resolve(source: &str) -> Result<Self, UnitRefusal> {
        if source.len() > UNIT_MAX_SOURCE_BYTES {
            return Err(UnitRefusal::SourceTooLong);
        }
        if let Some((_, definition)) = BUILTIN_UNIT_DEFINITIONS
            .iter()
            .find(|(symbol, _)| *symbol == source)
        {
            return Ok(Self(*definition));
        }
        let mut admitted = None;
        for &(group, symbol, exponent, _) in BUILTIN_PREFIX_DEFINITIONS {
            let Some(base) = source.strip_prefix(symbol) else {
                continue;
            };
            for &(_, definition) in BUILTIN_UNIT_DEFINITIONS {
                if definition.symbol() != base || definition.prefix() != UnitPrefix::None {
                    continue;
                }
                let derived = if group == "si" {
                    definition.with_decimal_prefix(source, exponent)
                } else {
                    definition.with_binary_prefix(source, exponent as u8)
                };
                if let Ok(derived) = derived {
                    if admitted.is_some() {
                        return Err(UnitRefusal::AmbiguousSymbol);
                    }
                    admitted = Some(Self(derived));
                }
            }
        }
        admitted.ok_or(UnitRefusal::UnknownSymbol)
    }
}
impl serde::Serialize for Unit {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(self.encode().as_slice(), s)
    }
}
impl<'de> serde::Deserialize<'de> for Unit {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let bytes = crate::physical_definition::deserialize_fixed::<D, UNIT_ENCODED_LEN>(d)?;
        Self::decode(&bytes)
            .map_err(|_| serde::de::Error::custom("invalid admitted Unit definition"))
    }
}

#[allow(non_upper_case_globals)]
impl Unit {
    pub const Acre: Self = Self(crate::BUILTIN_ACRE);
    pub const Ampere: Self = Self(crate::BUILTIN_AMPERE);
    pub const AmpereHour: Self = Self(crate::BUILTIN_AMPERE_HOUR);
    pub const Angstrom: Self = Self(crate::BUILTIN_ANGSTROM);
    pub const Arcminute: Self = Self(crate::BUILTIN_ARCMINUTE);
    pub const Arcsecond: Self = Self(crate::BUILTIN_ARCSECOND);
    pub const AstronomicalUnit: Self = Self(crate::BUILTIN_ASTRONOMICAL_UNIT);
    pub const Atmosphere: Self = Self(crate::BUILTIN_ATMOSPHERE);
    pub const Bar: Self = Self(crate::BUILTIN_BAR);
    pub const BasisPoint: Self = Self(crate::BUILTIN_BASIS_POINT);
    pub const Bit: Self = Self(crate::BUILTIN_BIT);
    pub const Byte: Self = Self(crate::BUILTIN_BYTE);
    pub const Calorie: Self = Self(crate::BUILTIN_CALORIE);
    pub const Celsius: Self = Self(crate::BUILTIN_CELSIUS);
    pub const Centimeter: Self = Self(crate::BUILTIN_CENTIMETER);
    pub const Chain: Self = Self(crate::BUILTIN_CHAIN);
    pub const CubicMeter: Self = Self(crate::BUILTIN_CUBIC_METER);
    pub const Day: Self = Self(crate::BUILTIN_DAY);
    pub const Degree: Self = Self(crate::BUILTIN_DEGREE);
    pub const Fahrenheit: Self = Self(crate::BUILTIN_FAHRENHEIT);
    pub const Foot: Self = Self(crate::BUILTIN_FOOT);
    pub const Fortnight: Self = Self(crate::BUILTIN_FORTNIGHT);
    pub const Furlong: Self = Self(crate::BUILTIN_FURLONG);
    pub const Gal: Self = Self(crate::BUILTIN_GAL);
    pub const Gibibyte: Self = Self(crate::BUILTIN_GIBIBYTE);
    pub const Gigabyte: Self = Self(crate::BUILTIN_GIGABYTE);
    pub const Gigahertz: Self = Self(crate::BUILTIN_GIGAHERTZ);
    pub const Gradian: Self = Self(crate::BUILTIN_GRADIAN);
    pub const Gram: Self = Self(crate::BUILTIN_GRAM);
    pub const Hand: Self = Self(crate::BUILTIN_HAND);
    pub const Hectare: Self = Self(crate::BUILTIN_HECTARE);
    pub const Hertz: Self = Self(crate::BUILTIN_HERTZ);
    pub const Hour: Self = Self(crate::BUILTIN_HOUR);
    pub const Inch: Self = Self(crate::BUILTIN_INCH);
    pub const Joule: Self = Self(crate::BUILTIN_JOULE);
    pub const JulianYear: Self = Self(crate::BUILTIN_JULIAN_YEAR);
    pub const Kelvin: Self = Self(crate::BUILTIN_KELVIN);
    pub const Kibibyte: Self = Self(crate::BUILTIN_KIBIBYTE);
    pub const Kiloampere: Self = Self(crate::BUILTIN_KILOAMPERE);
    pub const Kilobyte: Self = Self(crate::BUILTIN_KILOBYTE);
    pub const Kilocalorie: Self = Self(crate::BUILTIN_KILOCALORIE);
    pub const Kilogram: Self = Self(crate::BUILTIN_KILOGRAM);
    pub const Kilohertz: Self = Self(crate::BUILTIN_KILOHERTZ);
    pub const Kilojoule: Self = Self(crate::BUILTIN_KILOJOULE);
    pub const Kilometer: Self = Self(crate::BUILTIN_KILOMETER);
    pub const KilometerPerHour: Self = Self(crate::BUILTIN_KILOMETER_PER_HOUR);
    pub const Kilonewton: Self = Self(crate::BUILTIN_KILONEWTON);
    pub const Kilopascal: Self = Self(crate::BUILTIN_KILOPASCAL);
    pub const Kilovolt: Self = Self(crate::BUILTIN_KILOVOLT);
    pub const Kilowatt: Self = Self(crate::BUILTIN_KILOWATT);
    pub const KilowattHour: Self = Self(crate::BUILTIN_KILOWATT_HOUR);
    pub const Knot: Self = Self(crate::BUILTIN_KNOT);
    pub const League: Self = Self(crate::BUILTIN_LEAGUE);
    pub const Liter: Self = Self(crate::BUILTIN_LITER);
    pub const Mebibyte: Self = Self(crate::BUILTIN_MEBIBYTE);
    pub const Megabyte: Self = Self(crate::BUILTIN_MEGABYTE);
    pub const Megahertz: Self = Self(crate::BUILTIN_MEGAHERTZ);
    pub const Megajoule: Self = Self(crate::BUILTIN_MEGAJOULE);
    pub const Megapascal: Self = Self(crate::BUILTIN_MEGAPASCAL);
    pub const Megawatt: Self = Self(crate::BUILTIN_MEGAWATT);
    pub const Meter: Self = Self(crate::BUILTIN_METER);
    pub const MeterPerSecond: Self = Self(crate::BUILTIN_METER_PER_SECOND);
    pub const MeterPerSecondSquared: Self = Self(crate::BUILTIN_METER_PER_SECOND_SQUARED);
    pub const Microampere: Self = Self(crate::BUILTIN_MICROAMPERE);
    pub const MicroampereHour: Self = Self(crate::BUILTIN_MICROAMPERE_HOUR);
    pub const Microdegree: Self = Self(crate::BUILTIN_MICRODEGREE);
    pub const Microgram: Self = Self(crate::BUILTIN_MICROGRAM);
    pub const Microliter: Self = Self(crate::BUILTIN_MICROLITER);
    pub const Micrometer: Self = Self(crate::BUILTIN_MICROMETER);
    pub const Microradian: Self = Self(crate::BUILTIN_MICRORADIAN);
    pub const Microsecond: Self = Self(crate::BUILTIN_MICROSECOND);
    pub const Microvolt: Self = Self(crate::BUILTIN_MICROVOLT);
    pub const Mile: Self = Self(crate::BUILTIN_MILE);
    pub const MilePerHour: Self = Self(crate::BUILTIN_MILE_PER_HOUR);
    pub const MilliCelsius: Self = Self(crate::BUILTIN_MILLI_CELSIUS);
    pub const MilliFahrenheit: Self = Self(crate::BUILTIN_MILLI_FAHRENHEIT);
    pub const Milliampere: Self = Self(crate::BUILTIN_MILLIAMPERE);
    pub const MilliampereHour: Self = Self(crate::BUILTIN_MILLIAMPERE_HOUR);
    pub const Millibar: Self = Self(crate::BUILTIN_MILLIBAR);
    pub const Millidegree: Self = Self(crate::BUILTIN_MILLIDEGREE);
    pub const Milligram: Self = Self(crate::BUILTIN_MILLIGRAM);
    pub const Millihertz: Self = Self(crate::BUILTIN_MILLIHERTZ);
    pub const Millijoule: Self = Self(crate::BUILTIN_MILLIJOULE);
    pub const Millikelvin: Self = Self(crate::BUILTIN_MILLIKELVIN);
    pub const Milliliter: Self = Self(crate::BUILTIN_MILLILITER);
    pub const Millimeter: Self = Self(crate::BUILTIN_MILLIMETER);
    pub const MillimeterPerSecond: Self = Self(crate::BUILTIN_MILLIMETER_PER_SECOND);
    pub const Millinewton: Self = Self(crate::BUILTIN_MILLINEWTON);
    pub const Millionth: Self = Self(crate::BUILTIN_MILLIONTH);
    pub const Milliradian: Self = Self(crate::BUILTIN_MILLIRADIAN);
    pub const Millisecond: Self = Self(crate::BUILTIN_MILLISECOND);
    pub const Millivolt: Self = Self(crate::BUILTIN_MILLIVOLT);
    pub const Milliwatt: Self = Self(crate::BUILTIN_MILLIWATT);
    pub const Minute: Self = Self(crate::BUILTIN_MINUTE);
    pub const Moment: Self = Self(crate::BUILTIN_MOMENT);
    pub const Nanoampere: Self = Self(crate::BUILTIN_NANOAMPERE);
    pub const Nanometer: Self = Self(crate::BUILTIN_NANOMETER);
    pub const Nanosecond: Self = Self(crate::BUILTIN_NANOSECOND);
    pub const Nanovolt: Self = Self(crate::BUILTIN_NANOVOLT);
    pub const NauticalMile: Self = Self(crate::BUILTIN_NAUTICAL_MILE);
    pub const Newton: Self = Self(crate::BUILTIN_NEWTON);
    pub const One: Self = Self(crate::BUILTIN_ONE);
    pub const Ounce: Self = Self(crate::BUILTIN_OUNCE);
    pub const PartPerBillion: Self = Self(crate::BUILTIN_PART_PER_BILLION);
    pub const Pascal: Self = Self(crate::BUILTIN_PASCAL);
    pub const Percent: Self = Self(crate::BUILTIN_PERCENT);
    pub const Permille: Self = Self(crate::BUILTIN_PERMILLE);
    pub const Picosecond: Self = Self(crate::BUILTIN_PICOSECOND);
    pub const Pixel: Self = Self(crate::BUILTIN_PIXEL);
    pub const Pound: Self = Self(crate::BUILTIN_POUND);
    pub const Radian: Self = Self(crate::BUILTIN_RADIAN);
    pub const Rod: Self = Self(crate::BUILTIN_ROD);
    pub const Second: Self = Self(crate::BUILTIN_SECOND);
    pub const Shake: Self = Self(crate::BUILTIN_SHAKE);
    pub const SquareCentimeter: Self = Self(crate::BUILTIN_SQUARE_CENTIMETER);
    pub const SquareKilometer: Self = Self(crate::BUILTIN_SQUARE_KILOMETER);
    pub const SquareMeter: Self = Self(crate::BUILTIN_SQUARE_METER);
    pub const SquareMillimeter: Self = Self(crate::BUILTIN_SQUARE_MILLIMETER);
    pub const StandardGravity: Self = Self(crate::BUILTIN_STANDARD_GRAVITY);
    pub const Stone: Self = Self(crate::BUILTIN_STONE);
    pub const Tebibyte: Self = Self(crate::BUILTIN_TEBIBYTE);
    pub const Terabyte: Self = Self(crate::BUILTIN_TERABYTE);
    pub const Tonne: Self = Self(crate::BUILTIN_TONNE);
    pub const Torr: Self = Self(crate::BUILTIN_TORR);
    pub const Turn: Self = Self(crate::BUILTIN_TURN);
    pub const Volt: Self = Self(crate::BUILTIN_VOLT);
    pub const Watt: Self = Self(crate::BUILTIN_WATT);
    pub const WattHour: Self = Self(crate::BUILTIN_WATT_HOUR);
    pub const Week: Self = Self(crate::BUILTIN_WEEK);
    pub const Yard: Self = Self(crate::BUILTIN_YARD);
}
