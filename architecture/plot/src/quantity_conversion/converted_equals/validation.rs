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
    let original = text(receipt, "original")?;
    require(Quantity::parse_plot_literal(original).ok() == Some(source))?;
    let suffix_start = original
        .char_indices()
        .find_map(|(i, c)| (!(c.is_ascii_digit() || c == '.' || (i == 0 && c == '-'))).then_some(i))
        .ok_or(R::ForgedReceipt)?;
    let suffix =
        ResolvedQuantitySuffix::resolve(&original[suffix_start..]).map_err(|_| R::ForgedReceipt)?;
    let target_text = text(receipt, "target")?;
    let target = Unit::resolve(target_text).map_err(|_| R::ForgedReceipt)?;
    let (ss, so, sd) = source.reference_transform();
    let (ts, to, td, te) = target.reference_transform();
    require(text(receipt, "source-suffix")? == suffix.source())?;
    require(text(receipt, "source-base")? == suffix.unit().base_unit().plot_suffix())?;
    require(text(receipt, "source-prefix")? == suffix.prefix().map_or("", |p| p.symbol()))?;
    require(
        exponent(receipt, "source-prefix-exponent")?
            == i16::from(suffix.prefix().map_or(0, |p| p.exponent())),
    )?;
    require(text(receipt, "source-dimension")? == dimension_name(source.dimension()))?;
    require(text(receipt, "target-dimension")? == dimension_name(target.dimension()))?;
    require(text(receipt, "profile")? == QUANTITY_INFO_ID)?;
    require(text(receipt, "catalogue")? == QUANTITY_PREFIX_CATALOG_ID)?;
    require(exponent(receipt, "target-exponent")? == te)?;
    for (name, value) in [
        ("source-scale", ss),
        ("source-offset", so),
        ("source-denominator", sd),
        ("target-scale", ts),
        ("target-offset", to),
        ("target-denominator", td),
    ] {
        require(number(receipt, name)? == value)?;
    }
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
            require(
                reason
                    == match refusal {
                        QuantityConversionRefusal::IncompatibleDimensions => {
                            "incompatible-dimensions"
                        }
                        QuantityConversionRefusal::Inexact => "inexact",
                        QuantityConversionRefusal::Overflow => "overflow",
                    },
            )?;
            Ok((source, false))
        }
    }
}
