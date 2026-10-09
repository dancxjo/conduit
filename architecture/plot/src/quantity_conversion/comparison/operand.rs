//! Source-preserving role-aware operands share the Core comparison law.
use super::*;

enum Coordinate {
    Quantity(ExactDecimalQuantity),
    Difference(ExactTemperatureDifference),
}
pub(super) struct Operand<'a> {
    original: &'a str,
    coordinate: Coordinate,
    suffix: ResolvedQuantitySuffix<'a>,
}

pub(super) fn value_type(profile: ConversionProfile) -> StructuredInfoType {
    let identity = match profile {
        ConversionProfile::Quantity => "quantity/exact-comparison-operand@1",
        ConversionProfile::TemperatureDifference => {
            "quantity/exact-temperature-difference-comparison-operand@1"
        }
    };
    StructuredInfoType::record(
        kind_id(identity),
        vec![
            field("original", leaf(TEXT_INFO_ID)),
            field("coordinate", profile.source_type()),
            field("suffix", leaf(TEXT_INFO_ID)),
            field("base", leaf(TEXT_INFO_ID)),
            field("prefix", leaf(TEXT_INFO_ID)),
            field("prefix-exponent", leaf("value/i16")),
            field("composed-exponent", leaf("value/i16")),
            field("dimension", leaf(TEXT_INFO_ID)),
            field("scale", leaf("value/i128")),
            field("offset", leaf("value/i128")),
            field("denominator", leaf("value/i128")),
        ],
    )
    .expect("finite operand")
}
impl<'a> Operand<'a> {
    pub(super) fn parse(
        profile: ConversionProfile,
        original: &'a str,
    ) -> Result<Self, ExactQuantityConversionRequestRefusal> {
        let coordinate = match profile {
            ConversionProfile::Quantity => Coordinate::Quantity(
                ExactDecimalQuantity::parse_plot_literal(original)
                    .map_err(ExactQuantityConversionRequestRefusal::Source)?,
            ),
            ConversionProfile::TemperatureDifference => Coordinate::Difference(
                ExactTemperatureDifference::parse_plot_literal(original)
                    .map_err(ExactQuantityConversionRequestRefusal::TemperatureDifferenceSource)?,
            ),
        };
        let start = original
            .char_indices()
            .find_map(|(index, character)| {
                (!(character.is_ascii_digit()
                    || character == '.'
                    || (index == 0 && character == '-')))
                    .then_some(index)
            })
            .expect("parsed quantity has a suffix");
        let suffix = ResolvedQuantitySuffix::resolve(&original[start..])
            .expect("the checked parser resolved this exact suffix");
        Ok(Self {
            original,
            coordinate,
            suffix,
        })
    }
    pub(super) fn compare(&self, other: &Self) -> Result<Ordering, QuantityConversionRefusal> {
        match (&self.coordinate, &other.coordinate) {
            (Coordinate::Quantity(left), Coordinate::Quantity(right)) => left.compare(*right),
            (Coordinate::Difference(left), Coordinate::Difference(right)) => left.compare(*right),
            _ => Err(QuantityConversionRefusal::IncompatibleDimensions),
        }
    }
    pub(super) fn encode(
        &self,
        profile: ConversionProfile,
    ) -> Result<StructuredInfoValue, QuantityConversionPreparationRefusal> {
        use QuantityConversionPreparationRefusal as R;
        let (coordinate, (scale, offset, denominator)) = match self.coordinate {
            Coordinate::Quantity(value) => (value, value.reference_transform()),
            Coordinate::Difference(value) => (value.storage_coordinate(), value.transform()),
        };
        let value = |identity: &str, bytes: Vec<u8>| {
            StructuredInfoValue::leaf(leaf(identity), bytes).map_err(R::Receipt)
        };
        StructuredInfoValue::record(
            value_type(profile),
            [
                ("original", text(self.original)?),
                (
                    "coordinate",
                    profile.source_value(coordinate).map_err(R::Receipt)?,
                ),
                ("suffix", text(self.suffix.source())?),
                (
                    "base",
                    text(
                        self.suffix
                            .base()
                            .map(|base| base.unit())
                            .unwrap_or(coordinate.unit())
                            .plot_suffix(),
                    )?,
                ),
                (
                    "prefix",
                    text(self.suffix.prefix().map_or("", |prefix| prefix.symbol()))?,
                ),
                (
                    "prefix-exponent",
                    value(
                        "value/i16",
                        i16::from(self.suffix.prefix().map_or(0, |prefix| prefix.exponent()))
                            .to_le_bytes()
                            .to_vec(),
                    )?,
                ),
                (
                    "composed-exponent",
                    value(
                        "value/i16",
                        self.suffix
                            .decimal_exponent()
                            .unwrap_or(0)
                            .to_le_bytes()
                            .to_vec(),
                    )?,
                ),
                ("dimension", text(dimension_name(coordinate.dimension()))?),
                ("scale", value("value/i128", scale.to_le_bytes().to_vec())?),
                (
                    "offset",
                    value("value/i128", offset.to_le_bytes().to_vec())?,
                ),
                (
                    "denominator",
                    value("value/i128", denominator.to_le_bytes().to_vec())?,
                ),
            ]
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<Vec<_>, _>>()
            .map_err(R::Receipt)?,
        )
        .map_err(R::Receipt)
    }
}
