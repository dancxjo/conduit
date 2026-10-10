//! Finite inline semantic values shared by graphical and spoken Masks.
use alloc::{format, string::String};
use conduit_core::{
    CheckedValueContract, ExactDecimalQuantity, QuantityUnit, EXACT_DECIMAL_QUANTITY_INFO_ID,
};

pub const MAX_FACE_VALUE_BYTES: u32 = 1024;

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
    if contract.value_kind.as_str() == EXACT_DECIMAL_QUANTITY_INFO_ID {
        let quantity = ExactDecimalQuantity::decode(bytes).expect("validated canonical quantity");
        let number = decimal(quantity);
        let unit = match (quantity.unit(), spoken) {
            (QuantityUnit::Celsius, false) => "°C",
            (QuantityUnit::Celsius, true) => "degrees Celsius",
            (QuantityUnit::Fahrenheit, false) => "°F",
            (QuantityUnit::Fahrenheit, true) => "degrees Fahrenheit",
            (unit, _) => unit.semantic_id(),
        };
        return format!("{number} {unit}");
    }
    format!(
        "{}: {} canonical bytes",
        contract.value_kind.as_str(),
        bytes.len()
    )
}
fn decimal(quantity: ExactDecimalQuantity) -> String {
    let coefficient = quantity.coefficient();
    let mut digits = coefficient.unsigned_abs().to_string();
    let exponent = quantity.exponent();
    if exponent >= 0 {
        digits.extend(core::iter::repeat_n('0', exponent as usize));
    } else {
        let places = usize::from(exponent.unsigned_abs());
        if places >= digits.len() {
            digits = format!("0.{}{}", "0".repeat(places - digits.len()), digits);
        } else {
            digits.insert(digits.len() - places, '.');
        }
    }
    if coefficient < 0 {
        digits.insert(0, '-');
    }
    digits
}
use alloc::string::ToString;
