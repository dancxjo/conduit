//! Reviewed history Mask selection over the original immutable Face.
//! The required slot fixes identity and property types; the selected history
//! renderer separately validates schema values, quantities and temporal order.
pub const HISTORY_COMPONENT_MASK_ROOT: &str = "environment-history-mask";
pub const HISTORY_COMPONENT_RECIPE_ID: &str = "conduit.face/environment-history@1";
pub const HISTORY_COMPONENT_MASK_SOURCE: &str =
    include_str!("../components/environment-history-mask.conduit");
