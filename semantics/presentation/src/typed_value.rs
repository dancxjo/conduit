//! Finite inline semantic values shared by graphical and spoken Masks.
use alloc::{format, string::String};
use conduit_core::{primitive_info_kind, CheckedValueContract, PrimitiveInfoKind, Quantity, Unit};

pub const MAX_FACE_VALUE_BYTES: u32 = 4096;

pub(crate) fn validate(contract: &CheckedValueContract, bytes: &[u8]) -> Result<(), ()> {
    if contract.maximum_bytes == 0
        || contract.maximum_bytes > MAX_FACE_VALUE_BYTES
        || contract.value_kind.as_str().is_empty()
        || contract.value_kind.as_str().len() > crate::MAX_PRESENTATION_ID_BYTES
    {
        return Err(());
    }
    contract.validate_definition().map_err(|_| ())?;
    contract.validate(bytes).map_err(|_| ())
}

pub fn display_typed_value(contract: &CheckedValueContract, bytes: &[u8]) -> String {
    wording(contract, bytes, false)
}
pub fn spoken_typed_value(contract: &CheckedValueContract, bytes: &[u8]) -> String {
    wording(contract, bytes, true)
}
fn wording(contract: &CheckedValueContract, bytes: &[u8], spoken: bool) -> String {
    if validate(contract, bytes).is_err() {
        return "Invalid typed value".into();
    }
    let primitive = primitive_info_kind(contract.value_kind.as_str());
    if primitive == Some(PrimitiveInfoKind::MathematicalNumber) {
        return conduit_core::MathValue::decode(bytes)
            .and_then(|value| exact_wording(&value))
            .unwrap_or_else(|_| "Unrenderable exact mathematical value".into());
    }
    if primitive == Some(PrimitiveInfoKind::Unit) {
        let unit = Unit::decode(bytes).expect("validated canonical unit");
        return unit.symbol().into();
    }
    if matches!(
        primitive,
        Some(
            PrimitiveInfoKind::Quantity
                | PrimitiveInfoKind::Distance
                | PrimitiveInfoKind::Frequency
                | PrimitiveInfoKind::Duration
                | PrimitiveInfoKind::Voltage
                | PrimitiveInfoKind::Temperature
                | PrimitiveInfoKind::Angle
                | PrimitiveInfoKind::Ratio
                | PrimitiveInfoKind::PixelCount
        )
    ) {
        let quantity = Quantity::decode(bytes).expect("validated canonical quantity");
        let number = decimal(&quantity);
        let symbol = quantity.unit().symbol();
        let unit = match (symbol, spoken) {
            ("°C", true) => "degrees Celsius",
            ("°F", true) => "degrees Fahrenheit",
            _ => symbol,
        };
        return format!("{number} {unit}");
    }
    format!(
        "{}: {} canonical bytes",
        contract.value_kind.as_str(),
        bytes.len()
    )
}
fn decimal(quantity: &Quantity) -> String {
    quantity
        .coordinate()
        .to_owned()
        .and_then(|value| exact_wording(&value))
        .unwrap_or_else(|_| "Unrenderable exact quantity".into())
}

fn exact_wording(value: &conduit_core::MathValue) -> Result<String, conduit_core::NumericRefusal> {
    match value.terminating_decimal_literal(MAX_FACE_VALUE_BYTES as usize)? {
        Some(decimal) => Ok(decimal),
        None => value.source_expression(),
    }
}
