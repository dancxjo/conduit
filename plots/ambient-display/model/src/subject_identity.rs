//! Producer-scoped references shared by projection and prospective preparation.
//! These identities confer no publication authority and do not survive a new
//! checked producer, Plan or Play. No observation or Face is constructed here.
use crate::AmbientFaceRefusal;
use alloc::{format, string::String};
use conduit_face::FaceContributionBasis;
use conduit_plot::rust_binding::semantic_core as conduit_core;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmbientClockSubjectRefs {
    pub clock: String,
    pub weather: String,
    pub image: String,
}

/// Predict the existing producer's three semantic child references. Home uses
/// its one basis for both arguments; separate facets use their own exact bases.
/// The owner must recheck the prospective Play before admitting or starting it.
pub fn ambient_clock_subject_refs(
    clock_weather_basis: &FaceContributionBasis,
    wallpaper_basis: &FaceContributionBasis,
) -> Result<AmbientClockSubjectRefs, AmbientFaceRefusal> {
    let clock_weather = subject_root(clock_weather_basis, "clock-weather")?;
    let wallpaper = subject_root(wallpaper_basis, "wallpaper")?;
    Ok(AmbientClockSubjectRefs {
        clock: child_identity(&clock_weather, "clock"),
        weather: child_identity(&clock_weather, "weather"),
        image: child_identity(&wallpaper, "image"),
    })
}

pub(crate) fn subject_root(
    basis: &FaceContributionBasis,
    role: &str,
) -> Result<String, AmbientFaceRefusal> {
    for identity in [
        basis.checked_plot_id.as_str(),
        basis.plan_id.as_str(),
        basis.active_play_id.as_str(),
    ] {
        if identity.is_empty() || identity.len() > conduit_face::MAX_FACE_ID_BYTES {
            return Err(AmbientFaceRefusal::InvalidBasis);
        }
    }
    let digest = conduit_core::semantic_digest(
        "ambient-face-subject@1",
        format!("{:?}", (basis, role)).as_bytes(),
    );
    let suffix: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(crate::projection_memory::subject_identity(&suffix, role))
}

pub(crate) fn child_identity(root: &str, key: &str) -> String {
    let mut subject = String::with_capacity(crate::projection_memory::IDENTITY_BYTES);
    subject.push_str(root);
    subject.push('/');
    subject.push_str(key);
    subject
}

#[cfg(test)]
#[path = "subject_identity_tests.rs"]
mod tests;
