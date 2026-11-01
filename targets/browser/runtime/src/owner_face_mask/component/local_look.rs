//! Host-local proposal resolution; original Source/Plan configuration stays intact.
use super::VisualConstraints;
use conduit_face::{
    admit_visual_look, resolve_local_visual_look, AdmittedVisualLook, LocalLookContrast,
    LocalLookContrastOutcome, LocalLookInfo, LookInfo, VisualUserConstraints, MAX_LOOK_BYTES,
};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Bounded diagnostics, not another resource binding or full duplicated Look.
#[derive(Clone, Serialize)]
pub(crate) struct LocalLookProjection {
    identity: String,
    canonical_bytes: usize,
    canonical_sha256: String,
    overridden_fields: Vec<&'static str>,
    required_contrast: &'static str,
    contrast: &'static str,
}

pub(super) fn admit(
    author: &LookInfo,
    constraints: Option<&VisualConstraints>,
    user: VisualUserConstraints,
) -> Result<(AdmittedVisualLook, Option<LocalLookProjection>), String> {
    let Some(constraints) =
        constraints.filter(|value| value.local_look.is_some() || value.enhanced_contrast)
    else {
        return admit_visual_look(author, user)
            .map(|admitted| (admitted, None))
            .map_err(|error| format!("browser visual capacity: {error:?}"));
    };
    let local = if let Some(bytes) = &constraints.local_look {
        if bytes.len() > MAX_LOOK_BYTES {
            return Err("browser local Look exceeds canonical resource bound".into());
        }
        LocalLookInfo::decode(bytes).map_err(|error| format!("browser local Look: {error:?}"))?
    } else {
        LocalLookInfo::new(
            LocalLookContrast::Standard,
            "conduit.look/host-empty-local@1".into(),
            1,
            BoundedSequence::try_from_iter([]).map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("browser empty local Look: {error:?}"))?
    };
    let resolved = resolve_local_visual_look(
        author,
        &local,
        user,
        if constraints.enhanced_contrast {
            LocalLookContrast::Enhanced
        } else {
            LocalLookContrast::Standard
        },
    )
    .map_err(|error| format!("browser local Look resolution: {error:?}"))?;
    let bytes = local
        .clone()
        .encode()
        .map_err(|error| format!("browser local Look encoding: {error:?}"))?;
    let projection = LocalLookProjection {
        identity: local.identity().clone(),
        canonical_bytes: bytes.len(),
        canonical_sha256: format!("sha256:{:x}", Sha256::digest(&bytes)),
        overridden_fields: resolved.overridden_fields().to_vec(),
        required_contrast: match resolved.required_contrast {
            LocalLookContrast::Standard => "standard",
            LocalLookContrast::Enhanced => "enhanced",
        },
        contrast: match resolved.contrast {
            LocalLookContrastOutcome::CheckedNominalTokens => "checked-nominal-tokens",
            LocalLookContrastOutcome::PlatformColorsRequired => "platform-colors-required",
        },
    };
    Ok((resolved.admitted, Some(projection)))
}

#[cfg(test)]
#[path = "local_look_tests.rs"]
mod tests;
