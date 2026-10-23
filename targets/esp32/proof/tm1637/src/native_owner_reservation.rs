//! Authenticated intent correlated with actual admitted tables and Root sequence.
//! A reservation is neither native possession nor an installed/started Body.
use crate::native_authority::RootIssuer;
use crate::native_owner_control::{AuthenticatedRequest, Operation};
use conduit_core as c;
use conduit_environment_runtime::appliance::assigned::ValidatedAssignedAppliance;

#[derive(serde::Serialize)]
pub struct OwnerReservation {
    pub body: alloc::string::String,
    pub wake: alloc::string::String,
    pub body_plan: alloc::string::String,
    pub body_play: alloc::string::String,
    pub fragment: c::FragmentId,
    pub active: c::ActivePlayIdentity,
    pub assignment_seal: [u8; 32],
}
impl OwnerReservation {
    pub fn reserve(
        authenticated: AuthenticatedRequest<'_>,
        assignment: &ValidatedAssignedAppliance,
        actual_seal: &[u8; 32],
        issuer: &mut RootIssuer,
    ) -> Result<Self, &'static str> {
        if authenticated.operation() != Operation::Reserve {
            return Err("wrong-operation");
        }
        let request = authenticated.into_request();
        if request.assignment_seal != *actual_seal {
            return Err("assignment-seal");
        }
        let plan = c::PlanId::from(request.plan);
        let fragment = c::FragmentId::from(request.fragment);
        let active = issuer
            .reserve_assigned_play_matching(assignment, &plan, &fragment, request.fragment_play)
            .map_err(|_| "assignment-or-play")?;
        Ok(Self {
            body: request.body.into(),
            wake: request.wake.into(),
            body_plan: request.body_plan.into(),
            body_play: request.body_play.into(),
            fragment,
            active,
            assignment_seal: *actual_seal,
        })
    }
}
