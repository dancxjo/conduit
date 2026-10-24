//! Diagnostic provenance for the DOM view, not another configuration authority.
//! The full requested Look remains in the retained checked Mask configuration;
//! the full selected Look is already carried separately for actual rendering.
use conduit_face::ConfiguredMaskConfiguration;
use serde::{ser::SerializeStruct, Serialize, Serializer};
use sha2::{Digest, Sha256};

#[derive(Serialize)]
struct RetainedLook<'a> {
    kind: &'static str,
    profile: &'a conduit_core::KindId,
    canonical_bytes: usize,
    canonical_sha256: String,
}

pub(super) fn configuration<S: Serializer>(
    value: &ConfiguredMaskConfiguration,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let bytes = value.look.canonical_value();
    let look = RetainedLook {
        kind: "retained-startup",
        profile: value.look.profile(),
        canonical_bytes: bytes.len(),
        canonical_sha256: format!("sha256:{:x}", Sha256::digest(bytes)),
    };
    let mut view = serializer.serialize_struct(
        "ConfiguredMaskProvenance",
        9 + usize::from(value.layout.is_some()),
    )?;
    view.serialize_field("component", &value.component)?;
    view.serialize_field("invocation_path", &value.invocation_path)?;
    view.serialize_field("placement_id", &value.placement_id)?;
    view.serialize_field("host_id", &value.host_id)?;
    view.serialize_field("boot_id", &value.boot_id)?;
    view.serialize_field("execution_profile_id", &value.execution_profile_id)?;
    view.serialize_field("implementation_id", &value.implementation_id)?;
    view.serialize_field("recipe", &value.recipe)?;
    view.serialize_field("look", &look)?;
    if let Some(layout) = &value.layout {
        view.serialize_field("layout", layout)?;
    }
    view.end()
}
