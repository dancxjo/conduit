//! Authenticated intent correlated with actual admitted tables and Root sequence.
//! A reservation is neither native possession nor an installed/started Body.
use crate::native_authority::RootIssuer;
use crate::native_owner_control::{AuthenticatedRequest, Operation};
use conduit_core as c;
use conduit_environment_runtime::appliance::assigned::ValidatedAssignedAppliance;
use sha2::{Digest, Sha256};

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
/// Sender proof checked against the retained reservation. Not a start permit.
pub struct PreparationPermit {
    scope: [u8; 32],
}
impl PreparationPermit {
    pub fn matches(&self, reservation: &OwnerReservation) -> bool {
        self.scope == reservation.scope_digest()
    }
}
impl OwnerReservation {
    fn scope_digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/root-reservation@1");
        for id in [
            self.active.host_id.as_str(),
            self.active.boot_id.as_str(),
            &self.body,
            &self.wake,
            &self.body_plan,
            &self.body_play,
            self.active.plan_id.as_str(),
            self.fragment.as_str(),
            self.active.active_play_id.as_str(),
        ] {
            hash.update((id.len() as u16).to_le_bytes());
            hash.update(id.as_bytes());
        }
        hash.update(self.assignment_seal);
        hash.finalize().into()
    }

    pub fn authorize_preparation(
        &self,
        authenticated: AuthenticatedRequest<'_>,
    ) -> Result<PreparationPermit, &'static str> {
        self.authorize_operation(authenticated, Operation::Prepare)?;
        Ok(PreparationPermit {
            scope: self.scope_digest(),
        })
    }
    pub fn authorize_operation(
        &self,
        authenticated: AuthenticatedRequest<'_>,
        operation: Operation,
    ) -> Result<(), &'static str> {
        let request = authenticated.into_request();
        self.check_request(&request, operation)
    }
    pub(super) fn check_request(
        &self,
        request: &crate::native_owner_control::Request<'_>,
        operation: Operation,
    ) -> Result<(), &'static str> {
        if request.operation != operation {
            return Err("wrong-operation");
        }
        if request.host != self.active.host_id.as_str()
            || request.boot != self.active.boot_id.as_str()
            || request.body != self.body
            || request.wake != self.wake
            || request.body_plan != self.body_plan
            || request.body_play != self.body_play
            || request.plan != self.active.plan_id.as_str()
            || request.fragment != self.fragment.as_str()
            || request.fragment_play != self.active.active_play_id.as_str()
            || request.assignment_seal != self.assignment_seal
        {
            return Err("reservation-scope");
        }
        Ok(())
    }
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
