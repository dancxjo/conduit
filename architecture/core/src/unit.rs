//! Public Unit is the captured immutable mathematical definition itself.
use crate::{
    BUILTIN_PREFIX_DEFINITIONS, BUILTIN_UNIT_DEFINITIONS, DimensionDefinition, MathValueRef,
    PhysicalDefinitionRefusal, QuantityFamilyDefinition, QuantityRole, UnitDefinition,
    UnitDefinitionRef, UnitPrefix,
};
use alloc::string::{String, ToString};
pub const UNIT_INFO_ID: &str = "value/unit@2";
pub const MAX_UNIT_ENCODED_BYTES: usize = crate::MAX_UNIT_DEFINITION_BYTES;
pub const UNIT_MAX_SOURCE_BYTES: usize = 128;
pub type Unit = UnitDefinition;
pub type UnitRef<'a> = UnitDefinitionRef<'a>;
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum UnitRefusal {
    SourceTooLong,
    UnknownSymbol,
    AmbiguousSymbol,
    Definition(PhysicalDefinitionRefusal),
}
impl Unit {
    pub fn from_resolved(value: crate::ResolvedQuantitySuffix<'_>) -> Self {
        value.unit()
    }
    pub fn from_definition(definition: UnitDefinition) -> Self {
        definition
    }
    pub fn definition(&self) -> UnitDefinition {
        self.clone()
    }
    pub fn default_for_builtin_family(family: QuantityFamilyDefinition) -> Option<Self> {
        crate::BUILTIN_DEFAULT_UNIT_DEFINITIONS
            .iter()
            .find(|(_, unit)| unit.family().identity() == family.identity())
            .map(|(_, unit)| unit.clone())
    }
    pub const fn dimension(&self) -> DimensionDefinition {
        self.family().dimension()
    }
    pub fn definition_identity(&self) -> [u8; 32] {
        self.identity()
    }
    pub fn exact_offset(
        &self,
        role: QuantityRole,
    ) -> Result<MathValueRef<'_>, PhysicalDefinitionRefusal> {
        if !self.admits_role(role) {
            return Err(PhysicalDefinitionRefusal::InvalidRoles);
        }
        Ok(if role == QuantityRole::Point {
            self.exact_point_offset()
        } else {
            MathValueRef::validate(&[1, 1, 0, 0, 0, 0, 0, 0, 0, 0]).expect("canonical zero")
        })
    }
    pub fn canonical_symbol(&self) -> String {
        self.symbol().to_string()
    }
    pub fn plot_suffix(&self) -> String {
        self.canonical_symbol()
    }
    pub fn matches_source_evidence(&self, source: &str) -> bool {
        source.as_bytes() == self.symbol().as_bytes()
    }
    pub fn semantic_id(&self) -> String {
        use core::fmt::Write;
        let mut result = String::from("physical/unit/");
        for byte in self.identity() {
            write!(result, "{byte:02x}").expect("String write");
        }
        result
    }
    pub fn semantic_digest(&self) -> [u8; 32] {
        crate::semantic_digest(UNIT_INFO_ID, &self.identity())
    }
    pub fn from_plot_suffix(source: &str) -> Result<Self, UnitRefusal> {
        Self::resolve(source)
    }
    pub fn resolve(source: &str) -> Result<Self, UnitRefusal> {
        if source.len() > UNIT_MAX_SOURCE_BYTES {
            return Err(UnitRefusal::SourceTooLong);
        }
        if let Some((_, definition)) = BUILTIN_UNIT_DEFINITIONS
            .iter()
            .find(|(symbol, _)| *symbol == source)
        {
            return Ok(definition.clone());
        }
        let mut admitted = None;
        for &(group, symbol, exponent, _) in BUILTIN_PREFIX_DEFINITIONS {
            let Some(base) = source.strip_prefix(symbol) else {
                continue;
            };
            for (_, definition) in BUILTIN_UNIT_DEFINITIONS {
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
                    admitted = Some(derived);
                }
            }
        }
        admitted.ok_or(UnitRefusal::UnknownSymbol)
    }
}
impl UnitRef<'_> {
    pub const fn dimension(self) -> DimensionDefinition {
        self.family().dimension()
    }
    pub fn matches_source_evidence(self, source: &str) -> bool {
        source.as_bytes() == self.symbol().as_bytes()
    }
}
impl serde::Serialize for Unit {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(self.encoded(), s)
    }
}
impl<'de> serde::Deserialize<'de> for Unit {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let bytes =
            crate::physical_definition::deserialize_bounded::<D, MAX_UNIT_ENCODED_BYTES>(d)?;
        Self::decode(&bytes)
            .map_err(|_| serde::de::Error::custom("invalid captured mathematical Unit"))
    }
}
#[allow(non_upper_case_globals)]
impl Unit {
    pub const TurnPerSecond: Self = crate::BUILTIN_TURN_PER_SECOND;
    pub const RadianPerSecond: Self = crate::BUILTIN_RADIAN_PER_SECOND;

    pub const Acre: Self = crate::BUILTIN_ACRE;
    pub const Ampere: Self = crate::BUILTIN_AMPERE;
    pub const AmpereHour: Self = crate::BUILTIN_AMPERE_HOUR;
    pub const Angstrom: Self = crate::BUILTIN_ANGSTROM;
    pub const Arcminute: Self = crate::BUILTIN_ARCMINUTE;
    pub const Arcsecond: Self = crate::BUILTIN_ARCSECOND;
    pub const AstronomicalUnit: Self = crate::BUILTIN_ASTRONOMICAL_UNIT;
    pub const Atmosphere: Self = crate::BUILTIN_ATMOSPHERE;
    pub const Bar: Self = crate::BUILTIN_BAR;
    pub const BasisPoint: Self = crate::BUILTIN_BASIS_POINT;
    pub const Bit: Self = crate::BUILTIN_BIT;
    pub const Byte: Self = crate::BUILTIN_BYTE;
    pub const Calorie: Self = crate::BUILTIN_CALORIE;
    pub const Celsius: Self = crate::BUILTIN_CELSIUS;
    pub const Centimeter: Self = crate::BUILTIN_CENTIMETER;
    pub const Chain: Self = crate::BUILTIN_CHAIN;
    pub const CubicMeter: Self = crate::BUILTIN_CUBIC_METER;
    pub const Day: Self = crate::BUILTIN_DAY;
    pub const Degree: Self = crate::BUILTIN_DEGREE;
    pub const Fahrenheit: Self = crate::BUILTIN_FAHRENHEIT;
    pub const Foot: Self = crate::BUILTIN_FOOT;
    pub const Fortnight: Self = crate::BUILTIN_FORTNIGHT;
    pub const Furlong: Self = crate::BUILTIN_FURLONG;
    pub const Gal: Self = crate::BUILTIN_GAL;
    pub const Gibibyte: Self = crate::BUILTIN_GIBIBYTE;
    pub const Gigabyte: Self = crate::BUILTIN_GIGABYTE;
    pub const Gigahertz: Self = crate::BUILTIN_GIGAHERTZ;
    pub const Gradian: Self = crate::BUILTIN_GRADIAN;
    pub const Gram: Self = crate::BUILTIN_GRAM;
    pub const Hand: Self = crate::BUILTIN_HAND;
    pub const Hectare: Self = crate::BUILTIN_HECTARE;
    pub const Hertz: Self = crate::BUILTIN_HERTZ;
    pub const Hour: Self = crate::BUILTIN_HOUR;
    pub const Inch: Self = crate::BUILTIN_INCH;
    pub const Joule: Self = crate::BUILTIN_JOULE;
    pub const JulianYear: Self = crate::BUILTIN_JULIAN_YEAR;
    pub const Kelvin: Self = crate::BUILTIN_KELVIN;
    pub const Kibibyte: Self = crate::BUILTIN_KIBIBYTE;
    pub const Kiloampere: Self = crate::BUILTIN_KILOAMPERE;
    pub const Kilobyte: Self = crate::BUILTIN_KILOBYTE;
    pub const Kilocalorie: Self = crate::BUILTIN_KILOCALORIE;
    pub const Kilogram: Self = crate::BUILTIN_KILOGRAM;
    pub const Kilohertz: Self = crate::BUILTIN_KILOHERTZ;
    pub const Kilojoule: Self = crate::BUILTIN_KILOJOULE;
    pub const Kilometer: Self = crate::BUILTIN_KILOMETER;
    pub const KilometerPerHour: Self = crate::BUILTIN_KILOMETER_PER_HOUR;
    pub const Kilonewton: Self = crate::BUILTIN_KILONEWTON;
    pub const Kilopascal: Self = crate::BUILTIN_KILOPASCAL;
    pub const Kilovolt: Self = crate::BUILTIN_KILOVOLT;
    pub const Kilowatt: Self = crate::BUILTIN_KILOWATT;
    pub const KilowattHour: Self = crate::BUILTIN_KILOWATT_HOUR;
    pub const Knot: Self = crate::BUILTIN_KNOT;
    pub const League: Self = crate::BUILTIN_LEAGUE;
    pub const Liter: Self = crate::BUILTIN_LITER;
    pub const Mebibyte: Self = crate::BUILTIN_MEBIBYTE;
    pub const Megabyte: Self = crate::BUILTIN_MEGABYTE;
    pub const Megahertz: Self = crate::BUILTIN_MEGAHERTZ;
    pub const Megajoule: Self = crate::BUILTIN_MEGAJOULE;
    pub const Megapascal: Self = crate::BUILTIN_MEGAPASCAL;
    pub const Megawatt: Self = crate::BUILTIN_MEGAWATT;
    pub const Meter: Self = crate::BUILTIN_METER;
    pub const MeterPerSecond: Self = crate::BUILTIN_METER_PER_SECOND;
    pub const MeterPerSecondSquared: Self = crate::BUILTIN_METER_PER_SECOND_SQUARED;
    pub const Microampere: Self = crate::BUILTIN_MICROAMPERE;
    pub const MicroampereHour: Self = crate::BUILTIN_MICROAMPERE_HOUR;
    pub const Microdegree: Self = crate::BUILTIN_MICRODEGREE;
    pub const Microgram: Self = crate::BUILTIN_MICROGRAM;
    pub const Microliter: Self = crate::BUILTIN_MICROLITER;
    pub const Micrometer: Self = crate::BUILTIN_MICROMETER;
    pub const Microradian: Self = crate::BUILTIN_MICRORADIAN;
    pub const Microsecond: Self = crate::BUILTIN_MICROSECOND;
    pub const Microvolt: Self = crate::BUILTIN_MICROVOLT;
    pub const Mile: Self = crate::BUILTIN_MILE;
    pub const MilePerHour: Self = crate::BUILTIN_MILE_PER_HOUR;
    pub const MilliCelsius: Self = crate::BUILTIN_MILLI_CELSIUS;
    pub const MilliFahrenheit: Self = crate::BUILTIN_MILLI_FAHRENHEIT;
    pub const Milliampere: Self = crate::BUILTIN_MILLIAMPERE;
    pub const MilliampereHour: Self = crate::BUILTIN_MILLIAMPERE_HOUR;
    pub const Millibar: Self = crate::BUILTIN_MILLIBAR;
    pub const Millidegree: Self = crate::BUILTIN_MILLIDEGREE;
    pub const Milligram: Self = crate::BUILTIN_MILLIGRAM;
    pub const Millihertz: Self = crate::BUILTIN_MILLIHERTZ;
    pub const Millijoule: Self = crate::BUILTIN_MILLIJOULE;
    pub const Millikelvin: Self = crate::BUILTIN_MILLIKELVIN;
    pub const Milliliter: Self = crate::BUILTIN_MILLILITER;
    pub const Millimeter: Self = crate::BUILTIN_MILLIMETER;
    pub const MillimeterPerSecond: Self = crate::BUILTIN_MILLIMETER_PER_SECOND;
    pub const Millinewton: Self = crate::BUILTIN_MILLINEWTON;
    pub const Millionth: Self = crate::BUILTIN_MILLIONTH;
    pub const Milliradian: Self = crate::BUILTIN_MILLIRADIAN;
    pub const Millisecond: Self = crate::BUILTIN_MILLISECOND;
    pub const Millivolt: Self = crate::BUILTIN_MILLIVOLT;
    pub const Milliwatt: Self = crate::BUILTIN_MILLIWATT;
    pub const Minute: Self = crate::BUILTIN_MINUTE;
    pub const Moment: Self = crate::BUILTIN_MOMENT;
    pub const Nanoampere: Self = crate::BUILTIN_NANOAMPERE;
    pub const Nanometer: Self = crate::BUILTIN_NANOMETER;
    pub const Nanosecond: Self = crate::BUILTIN_NANOSECOND;
    pub const Nanovolt: Self = crate::BUILTIN_NANOVOLT;
    pub const NauticalMile: Self = crate::BUILTIN_NAUTICAL_MILE;
    pub const Newton: Self = crate::BUILTIN_NEWTON;
    pub const One: Self = crate::BUILTIN_ONE;
    pub const Ounce: Self = crate::BUILTIN_OUNCE;
    pub const PartPerBillion: Self = crate::BUILTIN_PART_PER_BILLION;
    pub const Pascal: Self = crate::BUILTIN_PASCAL;
    pub const Percent: Self = crate::BUILTIN_PERCENT;
    pub const Permille: Self = crate::BUILTIN_PERMILLE;
    pub const Picosecond: Self = crate::BUILTIN_PICOSECOND;
    pub const Pixel: Self = crate::BUILTIN_PIXEL;
    pub const Pound: Self = crate::BUILTIN_POUND;
    pub const Radian: Self = crate::BUILTIN_RADIAN;
    pub const Rod: Self = crate::BUILTIN_ROD;
    pub const Second: Self = crate::BUILTIN_SECOND;
    pub const Shake: Self = crate::BUILTIN_SHAKE;
    pub const SquareCentimeter: Self = crate::BUILTIN_SQUARE_CENTIMETER;
    pub const SquareKilometer: Self = crate::BUILTIN_SQUARE_KILOMETER;
    pub const SquareMeter: Self = crate::BUILTIN_SQUARE_METER;
    pub const SquareMillimeter: Self = crate::BUILTIN_SQUARE_MILLIMETER;
    pub const StandardGravity: Self = crate::BUILTIN_STANDARD_GRAVITY;
    pub const Stone: Self = crate::BUILTIN_STONE;
    pub const Tebibyte: Self = crate::BUILTIN_TEBIBYTE;
    pub const Terabyte: Self = crate::BUILTIN_TERABYTE;
    pub const Tonne: Self = crate::BUILTIN_TONNE;
    pub const Torr: Self = crate::BUILTIN_TORR;
    pub const Turn: Self = crate::BUILTIN_TURN;
    pub const Volt: Self = crate::BUILTIN_VOLT;
    pub const Watt: Self = crate::BUILTIN_WATT;
    pub const WattHour: Self = crate::BUILTIN_WATT_HOUR;
    pub const Week: Self = crate::BUILTIN_WEEK;
    pub const Yard: Self = crate::BUILTIN_YARD;
}

impl<'a> UnitRef<'a> {
    pub fn exact_offset(
        self,
        role: QuantityRole,
    ) -> Result<MathValueRef<'a>, PhysicalDefinitionRefusal> {
        if !self.admits_role(role) {
            return Err(PhysicalDefinitionRefusal::InvalidRoles);
        }
        Ok(if role == QuantityRole::Point {
            self.exact_point_offset()
        } else {
            MathValueRef::validate(&[1, 1, 0, 0, 0, 0, 0, 0, 0, 0]).expect("canonical zero")
        })
    }
}
