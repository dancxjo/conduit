//! Allocation-free structural observations of retained canonical frames.
//! These observations carry no parser, Native-law or Source admission authority.
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue};
pub(crate) type View<'a> = ValidatedCanonicalStructuredValue<'a>;
#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameRefusal;
pub(crate) fn view(bytes: &[u8]) -> Result<View<'_>, FrameRefusal> {
    validate_canonical_structured_value(bytes).map_err(|_| FrameRefusal)
}
pub(crate) fn field<'a>(value: View<'a>, name: &str) -> Result<View<'a>, FrameRefusal> {
    value.record_field(name).map_err(|_| FrameRefusal)?.ok_or(FrameRefusal)
}
pub(crate) fn path<'a>(mut value: View<'a>, names: &[&str]) -> Result<View<'a>, FrameRefusal> {
    for name in names { value = field(value, name)?; }
    Ok(value)
}
pub(crate) fn unsigned(value: View<'_>) -> Result<u64, FrameRefusal> {
    let bytes = value.primitive_bytes("value/u64").map_err(|_| FrameRefusal)?;
    Ok(u64::from_le_bytes(bytes.try_into().map_err(|_| FrameRefusal)?))
}
pub(crate) fn boolean(value: View<'_>) -> Result<bool, FrameRefusal> {
    match value.primitive_bytes("value/bool").map_err(|_| FrameRefusal)? {
        [0] => Ok(false), [1] => Ok(true), _ => Err(FrameRefusal),
    }
}
