//! Exact made values of ordinary Conduitese-authored Look definitions.
use super::*;
use conduit_plot::rust_binding::NativeRustBinding;

pub const FIELD_INSTRUMENT_DAY_LOOK_ID: &str = "conduit.look/field-instrument-day@1";
pub const FIELD_INSTRUMENT_NIGHT_LOOK_ID: &str = "conduit.look/field-instrument-night@1";
pub const FIELD_INSTRUMENT_LOOKS_SOURCE: &str =
    include_str!("../../field-instrument-looks.conduit");

/// Chalk, graphite and cobalt, authored through the existing checked Look Type.
/// Renderers separately admit the requested typography and spacing resources.
pub fn field_instrument_day_look() -> Result<LookInfo, NativeBindingRefusal> {
    LookInfo::decode(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/field-instrument-day.look"
    )))
}

/// The same visual family on graphite surfaces, independently selectable by a Mask.
pub fn field_instrument_night_look() -> Result<LookInfo, NativeBindingRefusal> {
    LookInfo::decode(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/field-instrument-night.look"
    )))
}
