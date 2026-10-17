//! Source-preserving role-aware operands share the Core comparison law.
use super::*;

enum Coordinate {
    Quantity(Quantity),
    Difference(ExactTemperatureDifference),
}
pub(super) struct Operand<'a> {
    original: &'a str,
    coordinate: Coordinate,
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
        let value = match coordinate {
            Coordinate::Quantity(value) => value,
            Coordinate::Difference(value) => value.storage_coordinate(),
        };
        if !value.matches_literal_evidence(original) {
            return Err(ExactQuantityConversionRequestRefusal::SourceEvidenceMismatch);
        }
        Ok(Self {
            original,
            coordinate,
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
        let coordinate = match self.coordinate {
            Coordinate::Quantity(value) => value,
            Coordinate::Difference(value) => value.storage_coordinate(),
        };
        StructuredInfoValue::record(
            value_type(profile),
            [
                ("original", text(self.original)?),
                (
                    "coordinate",
                    profile.source_value(coordinate).map_err(R::Receipt)?,
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
