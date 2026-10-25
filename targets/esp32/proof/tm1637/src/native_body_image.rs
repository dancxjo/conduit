//! Trusted installed-image boundary. Membership is separate from Play.
use alloc::boxed::Box;
use conduit_body::{BodyBiographyEvidence, SealedBodyLifecycleSession};

pub fn load(
    root: &crate::native_root::NativeRoot,
) -> Result<Box<SealedBodyLifecycleSession>, &'static str> {
    let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/body-biography.json"));
    if bytes.len() > 8192 {
        return Err("body-image-bound");
    }
    let evidence: BodyBiographyEvidence =
        serde_json::from_slice(bytes).map_err(|_| "body-image-representation")?;
    let initialized = root.advertisement().map_err(|_| "body-image-root-scope")?;
    let owner = Box::new(
        conduit_environment_runtime::body_image::open_installed_image_sealed(
            evidence,
            &initialized,
        )
        .map_err(|_| "body-image-installation")?,
    );
    drop(initialized);
    let memory = esp_alloc::HEAP.stats();
    let current = owner
        .evidence()
        .membership
        .parts
        .iter()
        .find_map(|part| part.current.as_ref())
        .ok_or("body-image-current-member")?;
    // This is metadata from ordinary biography, not an inferred execution state.
    let report = serde_json::json!({
        "event":"body-image-loaded",
        "body_id":owner.evidence().body_id,
        "friendly_name":owner.evidence().friendly_name,
        "host_id":current.host_id,
        "boot_id":current.boot_id,
        "offer_generation":current.offer_generation.0,
        "membership_revision":owner.evidence().membership.revision.0,
        "member_admitted":true,
        "root_heap_size_bytes":memory.size,
        "root_heap_used_bytes":memory.current_usage,
        "root_heap_peak_bytes":memory.max_usage,
        "play_started":false,
    });
    esp_println::println!("{}", report);
    Ok(owner)
}
