//! Personal appearance resolution over an immutable author proposal.
//! Caller binds the local resource independently; these values grant no authority.
use crate::*;
use alloc::{format, vec::Vec};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
use sha2::{Digest, Sha256};
mod contrast;
mod fields;
pub use contrast::LocalLookContrastFailure;
pub const LOCAL_LOOK_SOURCE: &str = include_str!("../local-look.conduit");

#[derive(Debug)]
pub enum LocalLookRefusal {
    Native(NativeBindingRefusal),
    ResourceBound,
    AuthorHasNoVisualFacet,
    DuplicateField(&'static str),
    Visual(LookVisualAdmissionRefusal),
    Contrast {
        pair: LocalLookContrastFailure,
        foreground: LocalLookValueOrigin,
        background: LocalLookValueOrigin,
    },
}
impl From<NativeBindingRefusal> for LocalLookRefusal {
    fn from(value: NativeBindingRefusal) -> Self {
        Self::Native(value)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalLookValueOrigin {
    Author,
    Local,
    Accessibility,
    Fallback,
    Platform,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalLookContrastOutcome {
    CheckedNominalTokens,
    PlatformColorsRequired,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedLocalLook {
    pub author: LookInfo,
    pub local: LocalLookInfo,
    pub admitted: AdmittedVisualLook,
    pub contrast: LocalLookContrastOutcome,
    overridden: Vec<&'static str>,
}
impl ResolvedLocalLook {
    /// Named finite expression values only, never Face/DOM selectors.
    pub fn overridden_fields(&self) -> &[&'static str] {
        &self.overridden
    }
    pub fn field_origin(&self, field: &str) -> Option<LocalLookValueOrigin> {
        if !fields::known(field) {
            return None;
        }
        if self.admitted.visual.platform_forced_colors && is_color(field) {
            return Some(LocalLookValueOrigin::Platform);
        }
        if (field == "text_scale_percent"
            && self.admitted.visual.accessibility_overrides.text_scale)
            || (field == "motion" && self.admitted.visual.accessibility_overrides.reduced_motion)
        {
            return Some(LocalLookValueOrigin::Accessibility);
        }
        if matches!(
            self.admitted.visual.outcome,
            VisualExpressionOutcome::PhosphorFallback { .. }
        ) {
            return Some(LocalLookValueOrigin::Fallback);
        }
        Some(origin(&self.overridden, field))
    }
}
fn is_color(field: &str) -> bool {
    matches!(
        field,
        "background"
            | "reading_paper"
            | "workbench_canvas"
            | "bootstrap_surface"
            | "surface"
            | "structure_primary"
            | "structure_secondary"
            | "text_primary"
            | "text_secondary"
            | "emphasis"
            | "focus"
            | "warning"
            | "failure"
            | "success"
            | "muted"
    )
}
fn origin(overridden: &[&str], field: &str) -> LocalLookValueOrigin {
    if overridden.contains(&field) {
        LocalLookValueOrigin::Local
    } else {
        LocalLookValueOrigin::Author
    }
}
/// Author → one local proposal → existing accessibility and Host data admission.
/// This does not select a Back, satisfy font/raster capacity or acknowledge a Show.
/// Unset values inherit exactly; repeated keys refuse instead of gaining order
/// precedence. Nominal contrast checks do not certify a whole Mask or device.
pub fn resolve_local_visual_look(
    author: &LookInfo,
    local: &LocalLookInfo,
    user: VisualUserConstraints,
) -> Result<ResolvedLocalLook, LocalLookRefusal> {
    let author_bytes = author
        .clone()
        .into_structured()?
        .canonical_bytes()
        .map_err(NativeBindingRefusal::InvalidValue)?;
    let local_bytes = local
        .clone()
        .into_structured()?
        .canonical_bytes()
        .map_err(NativeBindingRefusal::InvalidValue)?;
    if author_bytes.len() > MAX_LOOK_BYTES || local_bytes.len() > MAX_LOOK_BYTES {
        return Err(LocalLookRefusal::ResourceBound);
    }
    let LookVisualFacetOption::Some(visual) = author.visual() else {
        return Err(LocalLookRefusal::AuthorHasNoVisualFacet);
    };
    let mut proposal = fields::Proposal::new(visual);
    let overridden = proposal.apply(local.visual().as_slice())?;
    let requested = if overridden.is_empty() {
        author.clone()
    } else {
        let mut hash = Sha256::new();
        hash.update(b"conduit.look/local-resolution@1\0");
        hash.update((author_bytes.len() as u64).to_le_bytes());
        hash.update(author_bytes);
        hash.update(local_bytes);
        let identity = format!("conduit.look/resolved-{:x}", hash.finalize());
        LookInfo::new(
            author.generative().clone(),
            identity.clone(),
            author.spoken().clone(),
            2,
            proposal.finish(visual, &identity)?,
        )?
    };
    let admitted = admit_visual_look(&requested, user).map_err(LocalLookRefusal::Visual)?;
    let contrast = if admitted.visual.platform_forced_colors {
        LocalLookContrastOutcome::PlatformColorsRequired
    } else {
        let LookVisualFacetOption::Some(selected) = admitted.selected.visual() else {
            return Err(LocalLookRefusal::AuthorHasNoVisualFacet);
        };
        contrast::check(selected.tokens(), local.contrast()).map_err(|pair| {
            LocalLookRefusal::Contrast {
                foreground: if matches!(
                    admitted.visual.outcome,
                    VisualExpressionOutcome::PhosphorFallback { .. }
                ) {
                    LocalLookValueOrigin::Fallback
                } else {
                    origin(&overridden, pair.foreground)
                },
                background: if matches!(
                    admitted.visual.outcome,
                    VisualExpressionOutcome::PhosphorFallback { .. }
                ) {
                    LocalLookValueOrigin::Fallback
                } else {
                    origin(&overridden, pair.background)
                },
                pair,
            }
        })?;
        LocalLookContrastOutcome::CheckedNominalTokens
    };
    Ok(ResolvedLocalLook {
        author: author.clone(),
        local: local.clone(),
        admitted,
        contrast,
        overridden,
    })
}
