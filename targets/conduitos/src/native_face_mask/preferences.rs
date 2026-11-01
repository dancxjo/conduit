//! Independently bound Host preferences, resolved without changing author configuration.
use super::*;
use alloc::boxed::Box;
use conduit_face::{
    AdmittedVisualLook, LocalLookContrast, LocalLookInfo, LocalLookRefusal, LookInfo,
    ResolvedLocalLook, VisualUserConstraints, admit_visual_look, resolve_local_visual_look,
};

/// Distinct data-resolution refusals; this does not claim a usable surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeLocalLookRefusal {
    Native,
    ResourceBound,
    MissingVisualFacet,
    DuplicateField,
    Visual,
    Contrast,
    UnsupportedField(&'static str),
}
impl NativeLocalLookRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native-local-look-native-refused",
            Self::ResourceBound => "native-local-look-resource-bound",
            Self::MissingVisualFacet => "native-local-look-missing-visual-facet",
            Self::DuplicateField => "native-local-look-duplicate-field",
            Self::Visual => "native-local-look-visual-refused",
            Self::Contrast => "native-local-look-contrast-refused",
            Self::UnsupportedField(_) => "native-local-look-field-unsupported",
        }
    }
}
impl From<LocalLookRefusal> for NativeLocalLookRefusal {
    fn from(value: LocalLookRefusal) -> Self {
        match value {
            LocalLookRefusal::Native { .. } => Self::Native,
            LocalLookRefusal::ResourceBound(_) => Self::ResourceBound,
            LocalLookRefusal::AuthorHasNoVisualFacet => Self::MissingVisualFacet,
            LocalLookRefusal::DuplicateField(_) => Self::DuplicateField,
            LocalLookRefusal::Visual(_) => Self::Visual,
            LocalLookRefusal::Contrast { .. } => Self::Contrast,
        }
    }
}

pub(super) struct NativeLookPreferences {
    local: Option<LocalLookInfo>,
    minimum_contrast: LocalLookContrast,
}
impl Default for NativeLookPreferences {
    fn default() -> Self {
        Self {
            local: None,
            minimum_contrast: LocalLookContrast::Standard,
        }
    }
}

// Exactly one selected record. Preparation owns this finite allocation; the
// compositor borrows it, without duplicating the resolved selected Look.
pub(super) enum NativeVisualSelection {
    Author(Box<AdmittedVisualLook>),
    Local(Box<ResolvedLocalLook>),
}
impl NativeVisualSelection {
    pub(super) fn admitted(&self) -> &AdmittedVisualLook {
        match self {
            Self::Author(value) => value,
            Self::Local(value) => &value.admitted,
        }
    }
    #[cfg(test)]
    pub(super) fn into_admitted(self) -> AdmittedVisualLook {
        match self {
            Self::Author(value) => *value,
            Self::Local(value) => value.admitted,
        }
    }
}
impl NativeLookPreferences {
    pub(super) fn is_requested(&self) -> bool {
        self.local.is_some() || self.minimum_contrast == LocalLookContrast::Enhanced
    }
    pub(super) fn select(
        &self,
        author: &LookInfo,
        constraints: VisualUserConstraints,
    ) -> Result<NativeVisualSelection, NativeFaceMaskError> {
        if !self.is_requested() {
            return admit_visual_look(author, constraints)
                .map(|value| NativeVisualSelection::Author(Box::new(value)))
                .map_err(|_| NativeFaceMaskError::Face);
        }
        use conduit_plot::rust_binding::BoundedSequence;
        let empty;
        let local = if let Some(local) = &self.local {
            local
        } else {
            empty = LocalLookInfo::new(
                LocalLookContrast::Standard,
                "conduit.look/host-empty-local@1".into(),
                1,
                BoundedSequence::try_from_iter([]).map_err(|_| NativeFaceMaskError::Face)?,
            )
            .map_err(|_| NativeFaceMaskError::Face)?;
            &empty
        };
        let resolved = resolve_local_visual_look(author, local, constraints, self.minimum_contrast)
            .map_err(|error| NativeFaceMaskError::LocalLook(error.into()))?;
        // The current native path does not apply selected corner-radius tokens.
        // Do not acknowledge a personal request as though those fields rendered.
        if let Some(field) = resolved
            .overridden_fields()
            .iter()
            .find(|field| matches!(**field, "radius_control_px" | "radius_panel_px"))
        {
            return Err(NativeFaceMaskError::LocalLook(
                NativeLocalLookRefusal::UnsupportedField(field),
            ));
        }
        Ok(NativeVisualSelection::Local(Box::new(resolved)))
    }
}

impl NativeFaceMask {
    /// Bind personal data and an independent accessibility floor before an
    /// encounter. For the same immutable Face, use the ordinary fresh-Mask,
    /// encounter-transfer and retirement path; do not republish its old surface
    /// revision. Resolution can refuse before presentation; no action is granted.
    pub fn set_local_visual_look(
        &mut self,
        local: Option<LocalLookInfo>,
        minimum_contrast: LocalLookContrast,
    ) {
        self.look_preferences = NativeLookPreferences {
            local,
            minimum_contrast,
        };
    }
    /// Last prepared data resolution; availability remains the Show's fact.
    pub fn local_visual_look_resolution(&self) -> Option<&ResolvedLocalLook> {
        match self.visual_look.as_ref()? {
            NativeVisualSelection::Local(value) => Some(value),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "preferences_tests.rs"]
mod tests;
