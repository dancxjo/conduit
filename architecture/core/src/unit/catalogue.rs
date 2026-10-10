//! Reviewed named Unit constants and canonical descriptor construction.
use super::{CatalogUnit, Unit};

macro_rules! catalogue_units {
    ($($name:ident => $base:ident, $exponent:expr;)*) => {
        #[allow(non_upper_case_globals)]
        impl Unit {
            $(pub const $name: Self = Self { base: CatalogUnit::$base, prefix_exponent: $exponent };)*
            pub const fn from_catalogue(unit: CatalogUnit) -> Self {
                match unit { $(CatalogUnit::$name => Self::$name,)* }
            }
        }
    }
}

catalogue_units! {
    Picosecond => Second, -12;
    Nanosecond => Second, -9;
    Shake => Shake, 0;
    Microsecond => Second, -6;
    Millisecond => Second, -3;
    Second => Second, 0;
    Minute => Minute, 0;
    Moment => Moment, 0;
    Hour => Hour, 0;
    Day => Day, 0;
    Week => Week, 0;
    Fortnight => Fortnight, 0;
    JulianYear => JulianYear, 0;
    Millihertz => Hertz, -3;
    Hertz => Hertz, 0;
    Kilohertz => Hertz, 3;
    Megahertz => Hertz, 6;
    Gigahertz => Hertz, 9;
    Nanovolt => Volt, -9;
    Microvolt => Volt, -6;
    Millivolt => Volt, -3;
    Volt => Volt, 0;
    Kilovolt => Volt, 3;
    Nanoampere => Ampere, -9;
    Microampere => Ampere, -6;
    Milliampere => Ampere, -3;
    Ampere => Ampere, 0;
    Kiloampere => Ampere, 3;
    Millikelvin => Kelvin, -3;
    Kelvin => Kelvin, 0;
    MilliCelsius => MilliCelsius, 0;
    Celsius => Celsius, 0;
    MilliFahrenheit => MilliFahrenheit, 0;
    Fahrenheit => Fahrenheit, 0;
    MicroampereHour => AmpereHour, -6;
    MilliampereHour => AmpereHour, -3;
    AmpereHour => AmpereHour, 0;
    Micrometer => Meter, -6;
    Nanometer => Meter, -9;
    Millimeter => Meter, -3;
    Centimeter => Meter, -2;
    Meter => Meter, 0;
    Kilometer => Meter, 3;
    Angstrom => Angstrom, 0;
    Inch => Inch, 0;
    Hand => Hand, 0;
    Foot => Foot, 0;
    Yard => Yard, 0;
    Rod => Rod, 0;
    Chain => Chain, 0;
    Furlong => Furlong, 0;
    Mile => Mile, 0;
    League => League, 0;
    NauticalMile => NauticalMile, 0;
    AstronomicalUnit => AstronomicalUnit, 0;
    Microdegree => Microdegree, 0;
    Millidegree => Millidegree, 0;
    Degree => Degree, 0;
    Arcsecond => Arcsecond, 0;
    Arcminute => Arcminute, 0;
    Gradian => Gradian, 0;
    Turn => Turn, 0;
    Microradian => Radian, -6;
    Milliradian => Radian, -3;
    Radian => Radian, 0;
    Millionth => Millionth, 0;
    PartPerBillion => PartPerBillion, 0;
    BasisPoint => BasisPoint, 0;
    Permille => Permille, 0;
    Percent => Percent, 0;
    One => One, 0;
    Bit => Bit, 0;
    Byte => Byte, 0;
    Kilobyte => Byte, 3;
    Megabyte => Byte, 6;
    Gigabyte => Byte, 9;
    Kibibyte => Kibibyte, 0;
    Mebibyte => Mebibyte, 0;
    Gibibyte => Gibibyte, 0;
    Terabyte => Byte, 12;
    Tebibyte => Tebibyte, 0;
    Microgram => Gram, -6;
    Milligram => Gram, -3;
    Gram => Gram, 0;
    Kilogram => Gram, 3;
    Tonne => Tonne, 0;
    Ounce => Ounce, 0;
    Pound => Pound, 0;
    Stone => Stone, 0;
    SquareMillimeter => SquareMeter, -3;
    SquareCentimeter => SquareMeter, -2;
    SquareMeter => SquareMeter, 0;
    Hectare => Hectare, 0;
    SquareKilometer => SquareMeter, 3;
    Acre => Acre, 0;
    Microliter => Liter, -6;
    Milliliter => Liter, -3;
    Liter => Liter, 0;
    CubicMeter => CubicMeter, 0;
    MillimeterPerSecond => MeterPerSecond, -3;
    MeterPerSecond => MeterPerSecond, 0;
    KilometerPerHour => KilometerPerHour, 0;
    MilePerHour => MilePerHour, 0;
    Knot => Knot, 0;
    MeterPerSecondSquared => MeterPerSecondSquared, 0;
    StandardGravity => StandardGravity, 0;
    Gal => Gal, 0;
    Millinewton => Newton, -3;
    Newton => Newton, 0;
    Kilonewton => Newton, 3;
    Millijoule => Joule, -3;
    Joule => Joule, 0;
    Kilojoule => Joule, 3;
    Megajoule => Joule, 6;
    WattHour => WattHour, 0;
    KilowattHour => KilowattHour, 0;
    Calorie => Calorie, 0;
    Kilocalorie => Kilocalorie, 0;
    Milliwatt => Watt, -3;
    Watt => Watt, 0;
    Kilowatt => Watt, 3;
    Megawatt => Watt, 6;
    Pascal => Pascal, 0;
    Kilopascal => Pascal, 3;
    Megapascal => Pascal, 6;
    Bar => Bar, 0;
    Millibar => Millibar, 0;
    Atmosphere => Atmosphere, 0;
    Torr => Torr, 0;
    Pixel => Pixel, 0;
}
