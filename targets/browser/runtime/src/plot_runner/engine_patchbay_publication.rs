//! Prepare canonical delivery only for an exact scheduler-owned resident/Mask plan.
use crate::patchbay_ingress::{
    PatchbayPublication, PlannedPatchbayIngress, PreparedPatchbayIngress,
};
use crate::plot_runner::{engine::TourScheduler, patchbay_face::PatchbayObservation};
use crate::workspace_mask::BrowserMaskEffect;
use conduit_core::{ActivePlayId, BootId, HostId};
use conduit_face::FaceContributionBasis;

pub(in crate::plot_runner) struct PreparedPatchbayPublication {
    pub(in crate::plot_runner) publication: PatchbayPublication,
    pub(in crate::plot_runner) effect: BrowserMaskEffect,
    pub(in crate::plot_runner) sequence: u64,
    pub(in crate::plot_runner) previous_dispatch: Option<RetainedPatchbayDispatch>,
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
    let basis = FaceContributionBasis {
        checked_plot_id: context.resident.checked_plot_id.clone(),
        plan_id: context.plan.plan_id.clone(),
        active_play_id: play.clone(),
        required_interaction_context: None,
    };
    let fragment = owner
        .canonical_face_fragment(basis.clone(), &context.plan.body_id)
        .map_err(|error| format!("canonical Patchbay Face: {error:?}"))?;
    let face = context.project_fragment(fragment, owner.port.revision())?;
    let (mask, effect) = context.prepare_mask(face, host.clone(), boot.clone(), sequence)?;
    let publication = if scheduler.has_patchbay_fore(context.owner_node) {
        scheduler.validate_patchbay_fore(context.owner_node, &mask.observation().mask_show)?;
        PatchbayPublication::prepare_resident(mask, basis, budget)?
    } else {
        // Legacy fixture/profile compatibility; canonical Source uses its root Fore.
        let installation = crate::installed_browser::advertisement(host.clone(), boot.clone());
        let ingress =
            PreparedPatchbayIngress::prepare(PlannedPatchbayIngress::prepare(&installation)?)?;
        PatchbayPublication::prepare(mask, ingress, basis, budget)?
    };
    Ok(Some(PreparedPatchbayPublication {
        publication,
        effect,
        sequence,
        previous_dispatch: None,
    }))
}

pub(in crate::plot_runner) fn interact_publication(
    scheduler: &mut TourScheduler,
    node: conduit_kernel::NodeId,
    publication: &mut PatchbayPublication,
    input: &crate::patchbay_ingress::BrowserMaskInteraction,
) -> Result<(), String> {
    if publication.is_resident() {
        return interact_resident(scheduler, node, publication, input);
    }
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

fn interact_resident(
    scheduler: &mut TourScheduler,
    node: conduit_kernel::NodeId,
    publication: &mut PatchbayPublication,
    input: &crate::patchbay_ingress::BrowserMaskInteraction,
) -> Result<(), String> {
    let current = publication.observation();
    scheduler.validate_patchbay_fore(node, &current.mask_show)?;
    let bytes = match scheduler
        .applications
        .get_mut(usize::from(node.0))
        .and_then(|app| app.as_deref_mut())
    {
        Some(super::super::super::application_state::PreparedApplication::Patchbay(owner)) => {
            publication.emit_resident(owner, input)?
        }
        _ => return Err("resident Patchbay actor absent".into()),
    };
    let call = scheduler.admit_patchbay_fore(node, &bytes)?;
    let plan = scheduler.patchbay_input_plan(node)?;
    let delivered = scheduler
        .kernel
        .host_value(call.input.value)
        .map_err(super::super::debug_error)?;
    let owner = match scheduler
        .applications
        .get_mut(usize::from(node.0))
        .and_then(|app| app.as_deref_mut())
    {
        Some(super::super::super::application_state::PreparedApplication::Patchbay(owner)) => owner,
        _ => return Err("resident Patchbay actor absent".into()),
    };
    let (mut receipt, outcome) = crate::patchbay_ingress::apply_to_owner(
        plan,
        owner,
        delivered,
        &current.body_plan.body_id,
        publication.basis(),
        &current.face,
        &current.mask_show,
    )?;
    receipt.transport_completion = scheduler.complete_patchbay_fore(call, outcome);
    publication.record_resident(receipt);
    Ok(())
}

pub(in crate::plot_runner) struct RetainedPatchbayDispatch {
    pub(in crate::plot_runner) show_id: String,
    pub(in crate::plot_runner) face_id: String,
    pub(in crate::plot_runner) face_revision: u64,
    pub(in crate::plot_runner) receipt: crate::patchbay_ingress::PatchbayIngressDispatch,
}

impl PreparedPatchbayPublication {
    pub(in crate::plot_runner) fn dispatch_observation(&self) -> Option<serde_json::Value> {
        let (show, face, revision, receipt) = if let Some(receipt) = self.publication.receipt() {
            (
                &self.effect.show_id,
                &self.effect.face_id,
                self.effect.face_revision,
                receipt,
            )
        } else {
            let retained = self.previous_dispatch.as_ref()?;
            (
                &retained.show_id,
                &retained.face_id,
                retained.face_revision,
                &retained.receipt,
            )
        };
        Some(serde_json::json!({
            "show_id": show, "face_id": face, "face_revision": revision,
            "ingress_plan_id": receipt.ingress_plan_id,
            "owner_accepted": receipt.owner_result.is_ok(),
            "owner_refusal": receipt.owner_result.as_ref().err().map(|error| format!("{error:?}")),
            "request_delivery": receipt.request_delivery,
            "transport_completed": receipt.transport_completion.is_ok(),
            "transport_error": receipt.transport_completion.as_ref().err(),
        }))
    }
}
