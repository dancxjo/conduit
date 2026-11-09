//! Exact generic typed outcome; immutable schemas are linked once, not decoded in Play.
include!(concat!(env!("OUT_DIR"), "/append-profile.rs"));
pub(crate) const RESULT_SCHEMA: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/append-result.type"));
pub fn result_type() -> Result<conduit_core::StructuredInfoType, conduit_core::StructuredInfoRefusal>
{
    conduit_core::StructuredInfoType::from_canonical_bytes(RESULT_SCHEMA)
}
