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
    pub owner_result: Result<Option<PatchbayApplicationRequest>, PatchbayCommandRefusal>,
    pub transport_completion: Result<(), String>,
}

/// The installing owner supplies its actual retained port and current evidence.
/// This function does not install an owner route, replace a Show, or dispatch an
/// Edit/Mask effect. It handles only bytes emitted by the prepared kernel call.
/// An error before the receipt means the owner callback was never invoked.
pub fn dispatch_to_owner(
    ingress: &mut PreparedPatchbayIngress,
    owner: &mut PatchbayApplicationPort,
    body: &BodyId,
    basis: &FaceContributionBasis,
    face: &Face,
    show: &MaskShow,
) -> Result<PatchbayIngressDispatch, String> {
    let input = ingress.pending_input()?;
    let owner_result = owner.apply_face_interaction(input, body, basis, face, show);
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
    let transport_completion = ingress.complete(HostCallOutcome {
        disposition,
        output: None,
        failure,
    });
    Ok(PatchbayIngressDispatch {
        ingress_plan_id: ingress.planned().plan().plan_id.clone(),
        owner_result,
        transport_completion,
    })
}
