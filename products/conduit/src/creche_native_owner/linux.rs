//! Compose the actual Host-attached Linux output with the ordinary native Mask.
use super::{LocalCrecheNativeLimits, LocalCrecheNativeOwner};
use crate::creche_encounter::CrecheMaskShowCustody;
use conduit_linux_display::Scanout;
use conduit_std_host::StdHost;
use conduitos::{native_face_mask::NativeFaceMask, product_bases::NativeSurfaceProvider};

/// Retain exclusive scanout custody after checking its exact live Host attachment.
/// This prepares a Mask; the Body still must issue its checked Play before any
/// Face can be presented. The caller supplies verified code provenance (an exact installed executable
/// identity or target build identity), distinct from child-image Make selection.
#[allow(clippy::type_complexity)]
pub fn prepare_linux_creche_output<'display, 'surface>(
    host: &StdHost,
    scanout: &'display mut Scanout<'surface>,
    build_id: &str,
    limits: LocalCrecheNativeLimits,
) -> Result<
    (
        LocalCrecheNativeOwner<&'display mut Scanout<'surface>>,
        Box<dyn CrecheMaskShowCustody>,
    ),
    String,
> {
    let ready = host
        .ready_native_display()
        .ok_or("native Creche requires a current initialized Host display")?;
    if !ready.initialized().is_kmsdrm()
        || !scanout.matches_initialized(ready.initialized())
        || build_id.is_empty()
        || build_id.len() > 256
        || build_id.chars().any(char::is_control)
    {
        return Err("native Creche display custody or build identity mismatch".into());
    }
    let mut issuer_key = [0; 32];
    getrandom::fill(&mut issuer_key)
        .map_err(|error| format!("native Creche display issuer entropy: {error}"))?;
    let provider =
        NativeSurfaceProvider::from_initialized_output(ready.entry().clone(), issuer_key, scanout);
    issuer_key.fill(0);
    let provider =
        provider.map_err(|error| format!("native Creche display provider: {error:?}"))?;
    let surface_id = format!(
        "linux/kmsdrm/{}/{}",
        ready.initialized().window_id(),
        ready.initialized().instance_nonce()
    );
    let mask = NativeFaceMask::prepare(
        ready.host_id().clone(),
        ready.boot_id().clone(),
        host.advertisement().offer_generation,
        build_id,
        ready.entry().base_id.clone(),
        &surface_id,
        &provider,
    )
    .map_err(|error| error.as_str().to_owned())?;
    LocalCrecheNativeOwner::new(mask, scanout, limits)
}
