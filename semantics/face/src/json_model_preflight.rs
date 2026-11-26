//! Allocation-free JSON shape and selected owning-model admission inputs.
//! These helpers neither validate an entire Face/Plan nor grant authority.
mod census;
mod memory;
pub(crate) use census::generic_mask_plan_json_shape;
pub use census::{generic_face_json_shape, json_shape, JsonModelShape};
pub use memory::face_model_preparation_bytes;

pub const MAX_FACE_JSON_BYTES: usize = 512 * 1024;
/// Hard scanner ceiling for a bounded outer carrier containing Face and route.
/// Each selected caller must enforce its own independent, narrower profile.
pub const MAX_JSON_MODEL_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonModelRefusal {
    Capacity,
    Json,
    Unsupported,
}
impl JsonModelRefusal {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Capacity => "face-model-capacity",
            Self::Json => "face-model-json",
            Self::Unsupported => "face-model-unsupported",
        }
    }
}
#[cfg(test)]
#[path = "json_model_preflight/census_tests.rs"]
mod tests;
