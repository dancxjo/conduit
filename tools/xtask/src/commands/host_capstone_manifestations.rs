//! Exact replacement evidence for independently realized Manifestations.

use conduit_core::{BootId, Plan, SignId};
use conduit_presentation::{
    Manifestation, ManifestationAdmission, ManifestationLifecycle, ManifestationSet, Presentation,
};

pub(super) fn mark_replaced(
    presentation: &Presentation,
    plan: &Plan,
    admission: &ManifestationAdmission,
    available: &ManifestationSet,
) -> Result<ManifestationSet, Box<dyn std::error::Error>> {
    let replacements = available
        .manifestations
        .iter()
        .map(|item| {
            item.transition(
                ManifestationLifecycle::Replaced,
                SignId::from(format!(
                    "capstone/revised/{}/replaced",
                    item.host_id.as_str()
                )),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(super::debug_error)?;
    ManifestationSet::new(presentation, replacements, plan, admission).map_err(super::debug_error)
}

pub(super) fn identity_refusals(
    presentation: &Presentation,
    plan: &Plan,
    set: &ManifestationSet,
) -> (bool, bool, bool) {
    let native = &set.manifestations[0];
    let browser = &set.manifestations[1];
    let mut stale_boot = native.clone();
    stale_boot.boot_id = BootId::from("boot/stale");
    let mut stale_generation = native.clone();
    stale_generation.offer_generation.0 += 1;
    let mut cross_wired = native.clone();
    cross_wired.host_id = browser.host_id.clone();
    (
        stale_boot.validate_against(presentation, plan).is_err(),
        stale_generation
            .validate_against(presentation, plan)
            .is_err(),
        cross_wired.validate_against(presentation, plan).is_err(),
    )
}

pub(super) fn manifestation_for<'a>(
    set: &'a ManifestationSet,
    implementation: &str,
) -> Result<&'a Manifestation, Box<dyn std::error::Error>> {
    set.manifestations
        .iter()
        .find(|item| item.presenter_implementation_id.as_str() == implementation)
        .ok_or_else(|| format!("missing Manifestation for {implementation}").into())
}
