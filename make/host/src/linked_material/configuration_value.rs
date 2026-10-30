//! Make-only expressions for exact checked startup values. Target execution
//! reconstructs the original value without a configuration enum JSON decoder.
use conduit_core as c;
use std::fmt;

pub fn emit(value: &c::ConfigurationValue) -> Result<String, fmt::Error> {
    use c::ConfigurationValue as V;
    Ok(match value {
        V::Bool(value) => format!("c::ConfigurationValue::Bool({value:?})"),
        V::U64(value) => format!("c::ConfigurationValue::U64({value}u64)"),
        V::I64(value) => format!("c::ConfigurationValue::I64({value}i64)"),
        V::Text(value) => format!(
            "c::ConfigurationValue::Text(c::ConfigurationText::from_static({:?}))",
            value.as_str()
        ),
        V::Structured(value) => format!(
            "c::ConfigurationValue::Structured(c::StructuredConfigurationValue::from_static({:?}.into(), &{:?}).expect(\"Make-checked structured configuration\"))",
            value.profile().as_str(), value.canonical_value()
        ),
        V::Quantity(value) => format!(
            "c::ConfigurationValue::Quantity(c::QuantityConfigurationValue::new(c::Quantity::decode(&{:?}).expect(\"Make-checked quantity encoding\"), {:?}.into()).expect(\"Make-checked quantity evidence\"))",
            value.canonical_value(), value.source()
        ),
        V::Unit(value) => format!(
            "c::ConfigurationValue::Unit(c::UnitConfigurationValue::new(c::Unit::decode(&{:?}).expect(\"Make-checked unit encoding\"), {:?}.into()).expect(\"Make-checked unit evidence\"))",
            value.canonical_value(), value.source()
        ),
        V::TemperatureDifference(value) => format!(
            "c::ConfigurationValue::TemperatureDifference(c::ExactTemperatureDifferenceConfigurationValue::new(c::ExactTemperatureDifference::from_quantity(c::Quantity::decode(&{:?}).expect(\"Make-checked difference encoding\")).expect(\"Make-checked difference dimension\"), {:?}.into()).expect(\"Make-checked difference evidence\"))",
            value.canonical_value(), value.source()
        ),
    })
}
