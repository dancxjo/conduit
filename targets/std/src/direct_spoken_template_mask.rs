//! Explicit checked Look selection for a direct spoken Mask opening and reading.
use crate::{
    direct_spoken_mask::PreparedDirectSpokenMask,
    spoken_template_reading::PreparedSpokenTemplateReading, StdHost,
};
use conduit_face::{Face, SpokenTemplateWording};
#[path = "direct_spoken_template_mask/remote.rs"]
mod remote;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DirectSpokenTemplateLook {
    Brief,
    Expanded,
    Control,
}
impl DirectSpokenTemplateLook {
    pub fn entry(self) -> &'static str {
        match self {
            Self::Brief => "spoken/brief-template",
            Self::Expanded => "spoken/expanded-template",
            Self::Control => "spoken/control-template",
        }
    }
    pub fn mask_name(self) -> &'static str {
        match self {
            Self::Brief => "owner_spoken_brief_template_mask",
            Self::Expanded => "owner_spoken_expanded_template_mask",
            Self::Control => "owner_spoken_control_template_mask",
        }
    }
}

/// Opaque internally evaluated script and exact selected ordinary Mask Plan.
/// Caller-supplied wording receipts cannot construct or replace this selection.
pub struct PreparedDirectSpokenTemplateMask {
    look: DirectSpokenTemplateLook,
    mask: PreparedDirectSpokenMask,
    reading: PreparedSpokenTemplateReading,
    remote_route: Option<conduit_face::RemoteOwnerMaskRouteSeal>,
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
        face: &Face,
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

    pub fn validate_current(&self, host: &StdHost, face: &Face) -> Result<(), String> {
        self.reading.validate_current(host, face)?;
        let current = host.prepare_direct_spoken_mask_named(face, self.look.mask_name())?;
        if let Some(route) = &self.remote_route {
            remote::validate(host, face, &current.planned_mask, route)?;
            if self.mask.planned_mask != route.planned_mask {
                return Err("checked spoken selection differs from its owner route".into());
            }
        } else if current.planned_mask != self.mask.planned_mask {
            return Err("checked spoken template Mask Plan changed".into());
        }
        if current.authority_grants != self.mask.authority_grants {
            return Err("checked spoken template Mask Plan or grants changed".into());
        }
        Ok(())
    }
}
impl StdHost {
    /// No effect: select checked wording and an exact Mask before any opening Play.
    pub fn prepare_direct_spoken_template_mask(
        &self,
        face: &Face,
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
            remote_route: None,
        })
    }
}

#[cfg(test)]
#[path = "direct_spoken_template_mask/tests.rs"]
mod tests;

/// Planning-only reproduction against the admitted peer's observed offer.
/// Disclosed grants are checked by the planner and must still be possessed by
/// the actual executing Host; this function grants no authority.
pub fn plan_template_for_offer(
    face: &Face,
    host: &conduit_core::HostAdvertisement,
    look: DirectSpokenTemplateLook,
    grants: Vec<conduit_core::AuthorityGrant>,
) -> Result<PreparedDirectSpokenMask, String> {
    crate::flow_activation::prepare_wording_for_offer(host, face, look.entry())?;
    crate::direct_spoken_mask::plan_for_offer(face, host, look.mask_name(), grants)
}
