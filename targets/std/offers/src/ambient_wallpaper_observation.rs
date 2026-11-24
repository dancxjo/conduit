//! Selected worker Back for the independently authored wallpaper observation.
use super::ambient_clock_observation::resident_ambient_source_offer;
use conduit_core::{semantic_digest, CapabilityOffer};
use conduit_semantic_catalog::{resident_wallpaper_kind, RESIDENT_WALLPAPER_BYTES};

pub const AMBIENT_WALLPAPER_SOURCE_IMPLEMENTATION: &str = "std/ambient-wallpaper-worker@1";
pub const AMBIENT_WALLPAPER_SOURCE_CALL: &str = "conduit.host/ambient-wallpaper-observation@1";
pub const AMBIENT_WALLPAPER_SOURCE_AUTHORITY: &str =
    "conduit.authority/ambient-wallpaper-observation@1";
pub const AMBIENT_WALLPAPER_SOURCE_RESOURCE: &str = "conduit.resource/ambient-wallpaper-source@1";

pub fn ambient_wallpaper_source_offer() -> Result<CapabilityOffer, String> {
    let digest = semantic_digest(
        AMBIENT_WALLPAPER_SOURCE_IMPLEMENTATION,
        concat!(
            include_str!("ambient_wallpaper_observation.rs"),
            include_str!("ambient_clock_observation.rs"),
            include_str!("../../src/ambient_wallpaper.rs"),
            include_str!("../../src/ambient_wallpaper/acquisition.rs"),
            include_str!("../../src/ambient_wallpaper/observation.rs"),
            include_str!("../../src/ambient_wallpaper/selection_wire.rs"),
            include_str!("../../src/selected_native_source/worker.rs"),
            include_str!("../../src/selected_native_source/selection.rs"),
            include_str!("../../src/installed_std/body_kernel/native_source.rs"),
            include_str!("../../../../semantics/catalog/src/ambient_display.rs"),
            include_str!("../../../../plots/ambient-display/types.conduit"),
        )
        .as_bytes(),
    );
    Ok(resident_ambient_source_offer(
        resident_wallpaper_kind()?,
        AMBIENT_WALLPAPER_SOURCE_IMPLEMENTATION,
        AMBIENT_WALLPAPER_SOURCE_CALL,
        AMBIENT_WALLPAPER_SOURCE_AUTHORITY,
        AMBIENT_WALLPAPER_SOURCE_RESOURCE,
        RESIDENT_WALLPAPER_BYTES,
        digest,
    ))
}
