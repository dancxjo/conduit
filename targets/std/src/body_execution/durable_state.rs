//! Explicit Root-selected residence for sealed Body-lived State.
use super::*;
use conduit_core::{AuthorityGrant, ResourceOffer};

/// Root retains this directory-to-resource mapping and its finite disk envelope.
/// The sealed Plan and original Host ledger remain the only execution admission.
/// Authority is the exact independently retained list (the std State offer has
/// no authority requirement); absence does not manufacture another grant.
pub struct BodyDurableStateSelection<'a> {
    pub root: &'a Path,
    pub resource: &'a ResourceOffer,
    pub authorities: &'a [AuthorityGrant],
    pub body_id: &'a conduit_body::BodyId,
    pub plan_id: &'a conduit_core::PlanId,
}
impl StdHost {
    pub fn run_body_plan_with_durable_state_to_with_start<'a, W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        fore: BodyForeExchange<'a>,
        selection: BodyDurableStateSelection<'a>,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        if selection.body_id != &request.plan.body_id
            || selection.plan_id != &request.plan.plan_id
            || !self.advertisement.resources.contains(selection.resource)
            || selection.authorities.iter().any(|grant| {
                grant.host_id != self.advertisement.host_id
                    || grant.boot_id != self.advertisement.boot_id
            })
        {
            return Err(
                "Body State selection differs from exact Body/Plan/Host/Boot/resource".into(),
            );
        }
        self.run_body_plan_to_with_start_and_clock_storage(
            request,
            output,
            timer,
            None,
            None,
            Some((fore.inputs, false, fore.output)),
            None,
            None,
            None,
            #[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
            None,
            Some(selection),
            started,
        )
    }
}
