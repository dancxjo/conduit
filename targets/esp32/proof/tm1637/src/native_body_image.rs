//! Trusted installed-image boundary. Membership is separate from Play.
use alloc::boxed::Box;
use conduit_body::{BodyBiographyEvidence, BodyLifecycleSession};

pub fn load(
    root: &crate::native_root::NativeRoot,
) -> Result<Box<BodyLifecycleSession>, &'static str> {
    let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/body-biography.json"));
    if bytes.len() > 8192 {
        return Err("body-image-bound");
    }
    let evidence: BodyBiographyEvidence =
        serde_json::from_slice(bytes).map_err(|_| "body-image-representation")?;
    let initialized = root.advertisement().map_err(|_| "body-image-root-scope")?;
    let owner =
        conduit_environment_runtime::body_image::open_installed_image(evidence, &initialized)
            .map_err(|_| "body-image-installation")?;
    // This is metadata from ordinary biography, not an inferred execution state.
    let report = serde_json::json!({
        "event":"body-image-loaded",
        "body_id":owner.evidence().body_id,
        "friendly_name":owner.evidence().friendly_name,
        "member_admitted":true,
        "play_started":false,
    });
    esp_println::println!("{}", report);
    Ok(Box::new(owner))
}
