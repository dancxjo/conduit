//! Explicit checked Look selection for a direct spoken Mask opening and reading.
use crate::{
    direct_spoken_mask::PreparedDirectSpokenMask,
    spoken_template_reading::PreparedSpokenTemplateReading, StdHost,
};
use conduit_presentation::{Presentation, SpokenTemplateWording};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DirectSpokenTemplateLook {
    Brief,
    Expanded,
}
impl DirectSpokenTemplateLook {
    pub fn entry(self) -> &'static str {
        match self {
            Self::Brief => "spoken/brief-template",
            Self::Expanded => "spoken/expanded-template",
        }
    }
    pub fn mask_name(self) -> &'static str {
        match self {
            Self::Brief => "owner_spoken_brief_template_mask",
            Self::Expanded => "owner_spoken_expanded_template_mask",
        }
    }
}

/// Opaque internally evaluated script and exact selected ordinary Mask Plan.
/// Caller-supplied wording receipts cannot construct or replace this selection.
pub struct PreparedDirectSpokenTemplateMask {
    look: DirectSpokenTemplateLook,
    mask: PreparedDirectSpokenMask,
    reading: PreparedSpokenTemplateReading,
}
impl PreparedDirectSpokenTemplateMask {
    pub fn look(&self) -> DirectSpokenTemplateLook {
        self.look
    }
    pub fn mask(&self) -> &PreparedDirectSpokenMask {
        &self.mask
    }
    pub fn wording(&self) -> &SpokenTemplateWording {
        self.reading.receipt()
    }
    pub fn opening_wording(&self) -> &str {
        self.reading.opening_wording()
    }
    /// Consume only this opaque exact selection. The opening is one bounded
    /// Mask Play; remaining frames retain this same private source selection.
    #[allow(clippy::too_many_arguments)]
    pub fn into_preparation(
        self,
        host: &StdHost,
        face: &Presentation,
        front_subject: String,
        target_subject: String,
        prepared_sign: conduit_core::SignId,
        available_sign: conduit_core::SignId,
    ) -> Result<
        (
            conduit_core::PlanFragment,
            crate::direct_spoken_mask_runtime::DirectSpokenMaskPreparation,
            PreparedSpokenTemplateReading,
        ),
        String,
    > {
        self.validate_current(host, face)?;
        let preparation = crate::direct_spoken_mask_runtime::DirectSpokenMaskPreparation::new_checked_template_opening(
            host, face.clone(), self.mask.planned_mask, front_subject, target_subject,
            prepared_sign, available_sign, &self.reading,
        )?;
        let [fragment] = self.mask.plan.fragments.as_slice() else {
            return Err("checked spoken template Mask requires one local fragment".into());
        };
        Ok((fragment.clone(), preparation, self.reading))
    }

    pub fn validate_current(&self, host: &StdHost, face: &Presentation) -> Result<(), String> {
        self.reading.validate_current(host, face)?;
        let current = host.prepare_direct_spoken_mask_named(face, self.look.mask_name())?;
        if current.planned_mask != self.mask.planned_mask
            || current.authority_grants != self.mask.authority_grants
        {
            return Err("checked spoken template Mask Plan or grants changed".into());
        }
        Ok(())
    }
}
impl StdHost {
    /// No effect: select checked wording and an exact Mask before any opening Play.
    pub fn prepare_direct_spoken_template_mask(
        &self,
        face: &Presentation,
        look: DirectSpokenTemplateLook,
    ) -> Result<PreparedDirectSpokenTemplateMask, String> {
        let reading = PreparedSpokenTemplateReading::prepare(self, face, look.entry())?;
        let mask = self.prepare_direct_spoken_mask_named(face, look.mask_name())?;
        if mask.mask.plot_name != look.mask_name() {
            return Err("checked spoken template selected a different Mask".into());
        }
        Ok(PreparedDirectSpokenTemplateMask {
            look,
            mask,
            reading,
        })
    }
}

#[cfg(test)]
#[path = "direct_spoken_template_mask/tests.rs"]
mod tests;
