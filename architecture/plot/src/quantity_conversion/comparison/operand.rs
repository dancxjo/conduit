//! Source-preserving role-aware operands share the Core comparison law.
use super::*;

enum Coordinate {
    Quantity(Quantity),
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
    pub(super) fn from_checked(
        profile: ConversionProfile,
        value: &'a ConfigurationValue,
    ) -> Result<Self, ExactQuantityConversionRequestRefusal> {
        let (original, coordinate) = match (profile, value) {
            (ConversionProfile::Quantity, ConfigurationValue::Quantity(value)) => {
                (value.source(), Coordinate::Quantity(value.value()))
            }
            (
                ConversionProfile::TemperatureDifference,
                ConfigurationValue::TemperatureDifference(value),
            ) => (value.source(), Coordinate::Difference(value.value())),
            _ => return Err(ExactQuantityConversionRequestRefusal::SourceEvidenceMismatch),
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
                ("base", text(self.suffix.unit().base_unit().plot_suffix())?),
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

pub(super) fn from_receipt(
    profile: ConversionProfile,
    receipt: &StructuredInfoValue,
    name: &str,
    original: &str,
) -> Result<ConfigurationValue, QuantityConversionPreparationRefusal> {
    use QuantityConversionPreparationRefusal as R;
    let StructuredInfoValueShape::Record(fields) = receipt.shape() else {
        return Err(R::ForgedReceipt);
    };
    let operand = fields
        .iter()
        .find(|f| f.name() == name)
        .ok_or(R::ForgedReceipt)?
        .value();
    let StructuredInfoValueShape::Record(fields) = operand.shape() else {
        return Err(R::ForgedReceipt);
    };
    let coordinate = fields
        .iter()
        .find(|f| f.name() == "coordinate")
        .ok_or(R::ForgedReceipt)?
        .value();
    match profile {
        ConversionProfile::Quantity => {
            let StructuredInfoValueShape::Leaf(bytes) = coordinate.shape() else {
                return Err(R::ForgedReceipt);
            };
            let value = Quantity::decode(bytes).map_err(|_| R::ForgedReceipt)?;
            QuantityConfigurationValue::new(value, original.into())
                .map(ConfigurationValue::Quantity)
                .ok_or(R::ForgedReceipt)
        }
        ConversionProfile::TemperatureDifference => {
            let value = temperature_difference::validate_source_value(coordinate)?;
            ExactTemperatureDifferenceConfigurationValue::new(value, original.into())
                .map(ConfigurationValue::TemperatureDifference)
                .ok_or(R::ForgedReceipt)
        }
    }
}
