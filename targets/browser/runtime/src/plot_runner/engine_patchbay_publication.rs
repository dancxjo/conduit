//! Prepare canonical delivery only for an exact scheduler-owned resident/Mask plan.
use crate::patchbay_ingress::{
    offer, PatchbayPublication, PlannedPatchbayIngress, PreparedPatchbayIngress,
};
use crate::plot_runner::{engine::TourScheduler, patchbay_face::PatchbayObservation};
use crate::workspace_mask::BrowserMaskEffect;
use conduit_core::{ActivePlayId, BootId, HostId};
use conduit_face::FaceContributionBasis;

pub(in crate::plot_runner) struct PreparedPatchbayPublication {
    pub(in crate::plot_runner) publication: PatchbayPublication,
    pub(in crate::plot_runner) effect: BrowserMaskEffect,
}

pub(in crate::plot_runner) fn prepare_publication(
    scheduler: &TourScheduler,
    context: &PatchbayObservation,
    host: &HostId,
    boot: &BootId,
    play: &ActivePlayId,
    sequence: u64,
) -> Result<Option<PreparedPatchbayPublication>, String> {
    if !context.has_composed_mask()? {
        return Ok(None);
    }
    let owner = match scheduler
        .applications
        .get(usize::from(context.owner_node.0))
        .and_then(|application| application.as_deref())
    {
        Some(super::super::super::application_state::PreparedApplication::Patchbay(owner)) => owner,
        _ => return Err("canonical publication has no exact prepared Patchbay owner".into()),
    };
    let budget = scheduler
        ._prepared_storage
        .publication_value_budget
        .as_ref()
        .ok_or("canonical publication has no prepared parent value budget")?;
    // Install this Back for the exact retained-owner route during preparation.
    // It is not a general browser offer or a conversion of the legacy Fore.
    let mut installation = crate::installed_browser::advertisement(host.clone(), boot.clone());
    installation.capabilities.push(offer());
    let ingress =
        PreparedPatchbayIngress::prepare(PlannedPatchbayIngress::prepare(&installation)?)?;
    let basis = FaceContributionBasis {
        checked_plot_id: context.resident.checked_plot_id.clone(),
        plan_id: context.plan.plan_id.clone(),
        active_play_id: play.clone(),
        required_interaction_context: None,
    };
    let fragment = owner
        .port
        .face_fragment(basis.clone(), &context.plan.body_id, true)
        .map_err(|error| format!("canonical Patchbay Face: {error:?}"))?;
    let face = context.project_fragment(fragment, owner.port.revision())?;
    let (mask, effect) = context.prepare_mask(face, host.clone(), boot.clone(), sequence)?;
    let publication = PatchbayPublication::prepare(mask, ingress, basis, budget)?;
    Ok(Some(PreparedPatchbayPublication {
        publication,
        effect,
    }))
}

pub(in crate::plot_runner) fn interact_publication(
    scheduler: &mut TourScheduler,
    node: conduit_kernel::NodeId,
    publication: &mut PatchbayPublication,
    input: &crate::patchbay_ingress::BrowserMaskInteraction,
) -> Result<(), String> {
    match scheduler
        .applications
        .get_mut(usize::from(node.0))
        .and_then(|app| app.as_deref_mut())
    {
        Some(super::super::super::application_state::PreparedApplication::Patchbay(owner)) => {
            publication.interact(owner, input)?;
            Ok(())
        }
        _ => Err("canonical interaction has no exact prepared Patchbay owner".into()),
    }
}
