//! One Root-selected finite immutable snapshot under an ordinary Body Play.
//! Directory custody and independently retained resource/grant are explicit;
//! paths, content references and transport delivery establish no authority.
use super::{BodyForeExchange, BodyRunReport, BodyRunRequest};
use crate::{StdHost, TimerAdapter};
use conduit_body::{BodyId, BodyPlayIdentity, Wake};
use conduit_core::{AuthorityGrant, PlanId, ResourceOffer};
use conduit_linux_snapshot_store::SnapshotDirectory;
use std::io::Write;

/// Actual descriptor and exact admitted pool/grant for one selected operation.
/// Root must retain the directory-to-pool mapping and disk capacity separately.
/// The existing Host ledger admits the complete exact Plan before start; this
/// selection neither issues a grant nor makes another reservation.
pub struct BodySnapshotSelection<'a> {
    pub directory: &'a SnapshotDirectory,
    pub resource: &'a ResourceOffer,
    pub authority: &'a AuthorityGrant,
    pub body_id: &'a BodyId,
    pub plan_id: &'a PlanId,
}
impl StdHost {
    /// Complete the original selected snapshot Host Call only after `started`
    /// accepts the exact issued Body Play. Output Fore remains ordinary kernel
    /// output; returning a reference does not imply downstream catalog commitment.
    pub fn run_body_plan_with_snapshot_to_with_start<'a, W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        fore: BodyForeExchange<'a>,
        selection: BodySnapshotSelection<'a>,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        if selection.body_id != &request.plan.body_id
            || selection.plan_id != &request.plan.plan_id
            || selection.authority.host_id != self.advertisement.host_id
            || selection.authority.boot_id != self.advertisement.boot_id
            || !self.advertisement.resources.contains(selection.resource)
        {
            return Err(
                "Body snapshot selection differs from admitted Body/Plan/Host/Boot/resource".into(),
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
            Some(selection),
            started,
        )
    }
}
