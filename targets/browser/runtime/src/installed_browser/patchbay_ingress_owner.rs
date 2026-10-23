//! Dispatch the emitted typed Host Call against independently retained owner evidence.
use super::PreparedPatchbayIngress;
use conduit_body::BodyId;
use conduit_face::{Face, FaceContributionBasis, MaskShow};
use conduit_kernel::{Failure, FailureCode, HostCallDisposition, HostCallOutcome};
use patchbay_application::{
    PatchbayApplicationPort, PatchbayApplicationRequest, PatchbayCommandRefusal,
};

/// The owner result survives an uncertain transport completion. In particular,
/// a returned Edit/Mask request must not be lost or automatically replayed.
#[must_use = "retain the owner request and inspect transport completion before proceeding"]
#[derive(Debug)]
pub struct PatchbayIngressDispatch {
    pub ingress_plan_id: conduit_core::PlanId,
    /// A request is either returned here or transferred to the installing
    /// owner's existing slot, as recorded by request_delivery.
    pub owner_result: Result<Option<PatchbayApplicationRequest>, PatchbayCommandRefusal>,
    pub request_delivery: PatchbayRequestDelivery,
    pub transport_completion: Result<(), String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum PatchbayRequestDelivery {
    NotRequested,
    Returned,
    Retained { application_revision: u32 },
}

/// Borrow the actual retained actor and retain delivery metadata in its existing
/// request slot. Implementing this interface does not install an owner route.
pub trait PatchbayIngressOwner {
    /// Refuse occupied delivery storage before either Mask or actor execution.
    fn port(&mut self) -> Result<&mut PatchbayApplicationPort, String>;
    fn record_dispatch(&mut self, dispatch: &mut PatchbayIngressDispatch);
}

impl PatchbayIngressOwner for PatchbayApplicationPort {
    fn port(&mut self) -> Result<&mut PatchbayApplicationPort, String> {
        Ok(self)
    }
    fn record_dispatch(&mut self, _dispatch: &mut PatchbayIngressDispatch) {}
}

/// The installing owner supplies its actual retained port and current evidence.
/// This function does not install an owner route, replace a Show, or dispatch an
/// Edit/Mask effect. It handles only bytes emitted by the prepared kernel call.
/// An error before the receipt means the owner callback was never invoked.
pub fn dispatch_to_owner(
    ingress: &mut PreparedPatchbayIngress,
    owner: &mut impl PatchbayIngressOwner,
    body: &BodyId,
    basis: &FaceContributionBasis,
    face: &Face,
    show: &MaskShow,
) -> Result<PatchbayIngressDispatch, String> {
    ingress.validate_local_show(show)?;
    let input = ingress.pending_input()?;
    let (mut dispatch, outcome) = apply_to_owner(
        ingress.planned().plan().plan_id.clone(),
        owner,
        input,
        body,
        basis,
        face,
        show,
    )?;
    dispatch.transport_completion = ingress.complete(outcome);
    Ok(dispatch)
}

/// Apply bytes delivered by an exact admitted kernel call. Transport completion
/// is separate so the resident root Fore can retain its sequence across Shows.
pub(crate) fn apply_to_owner(
    ingress_plan_id: conduit_core::PlanId,
    owner: &mut impl PatchbayIngressOwner,
    input: &[u8],
    body: &BodyId,
    basis: &FaceContributionBasis,
    face: &Face,
    show: &MaskShow,
) -> Result<(PatchbayIngressDispatch, HostCallOutcome), String> {
    let owner_result = owner
        .port()?
        .apply_face_interaction(input, body, basis, face, show);
    let (disposition, failure) = match &owner_result {
        Ok(_) => (HostCallDisposition::Completed, None),
        Err(PatchbayCommandRefusal::WrongOwner | PatchbayCommandRefusal::InvalidContribution) => (
            HostCallDisposition::Denied,
            Some(Failure {
                code: FailureCode::HostCallDenied,
                detail: 71,
            }),
        ),
        Err(PatchbayCommandRefusal::Domain(
            patchbay_application::PatchbayApplicationRefusal::RevisionExhausted,
        )) => (
            HostCallDisposition::Failed,
            Some(Failure {
                code: FailureCode::IdentityCapacityExhausted,
                detail: 71,
            }),
        ),
        Err(_) => (
            HostCallDisposition::Failed,
            Some(Failure {
                code: FailureCode::InvalidInput,
                detail: 71,
            }),
        ),
    };
    let outcome = HostCallOutcome {
        disposition,
        output: None,
        failure,
    };
    let request_delivery = if matches!(&owner_result, Ok(Some(_))) {
        PatchbayRequestDelivery::Returned
    } else {
        PatchbayRequestDelivery::NotRequested
    };
    let mut dispatch = PatchbayIngressDispatch {
        ingress_plan_id,
        owner_result,
        request_delivery,
        transport_completion: Err("owner result retained before transport completion".into()),
    };
    owner.record_dispatch(&mut dispatch);
    Ok((dispatch, outcome))
}
