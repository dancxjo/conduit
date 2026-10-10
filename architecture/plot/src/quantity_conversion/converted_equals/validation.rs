//! Borrowed receipt re-admission checks all retained facts against typed values.
use super::*;
type R = QuantityConversionPreparationRefusal;
type View<'a> = ValidatedCanonicalStructuredValue<'a>;
fn field<'a>(value: View<'a>, name: &str) -> Result<View<'a>, R> {
    value
        .record_field(name)
        .map_err(|_| R::ForgedReceipt)?
        .ok_or(R::ForgedReceipt)
}
fn bytes<'a>(value: View<'a>, name: &str, kind: &str) -> Result<&'a [u8], R> {
    field(value, name)?
        .primitive_bytes(kind)
        .map_err(|_| R::ForgedReceipt)
}
fn text<'a>(value: View<'a>, name: &str) -> Result<&'a str, R> {
    core::str::from_utf8(bytes(value, name, TEXT_INFO_ID)?).map_err(|_| R::ForgedReceipt)
}
fn number(value: View<'_>, name: &str) -> Result<i128, R> {
    Ok(i128::from_le_bytes(
        bytes(value, name, "value/i128")?
            .try_into()
            .map_err(|_| R::ForgedReceipt)?,
    ))
}
fn exponent(value: View<'_>, name: &str) -> Result<i16, R> {
    Ok(i16::from_le_bytes(
        bytes(value, name, "value/i16")?
            .try_into()
            .map_err(|_| R::ForgedReceipt)?,
    ))
}
fn require(condition: bool) -> Result<(), R> {
    condition.then_some(()).ok_or(R::ForgedReceipt)
}

pub(super) fn validate(receipt: View<'_>) -> Result<(Quantity, bool), R> {
    let source = Quantity::decode(bytes(receipt, "source", QUANTITY_INFO_ID)?)
        .map_err(|_| R::ForgedReceipt)?;
    require(source.matches_literal_evidence(text(receipt, "original")?))?;
    let target =
        Unit::decode(bytes(receipt, "target-unit", UNIT_INFO_ID)?).map_err(|_| R::ForgedReceipt)?;
    require(target.matches_source_evidence(text(receipt, "target")?))?;
    require(text(receipt, "profile")? == QUANTITY_INFO_ID)?;
    let result = field(receipt, "result")?;
    let tag = result.variant_tag().map_err(|_| R::ForgedReceipt)?;
    let payload = result
        .variant_payload(tag)
        .map_err(|_| R::ForgedReceipt)?
        .ok_or(R::ForgedReceipt)?;
    match source.convert_to_unit(target) {
        Ok((coefficient, power)) => {
            require(tag == "converted")?;
            require(
                number(payload, "coefficient")? == coefficient
                    && exponent(payload, "exponent")? == power,
            )?;
            Ok((source, true))
        }
        Err(refusal) => {
            require(tag == "refused")?;
            let reason = core::str::from_utf8(
                payload
                    .primitive_bytes(TEXT_INFO_ID)
                    .map_err(|_| R::ForgedReceipt)?,
            )
            .map_err(|_| R::ForgedReceipt)?;
            require(reason == refusal_reason(refusal))?;
            Ok((source, false))
        }
    }
}
