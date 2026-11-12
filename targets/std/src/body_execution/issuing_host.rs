//! Original issuer's local participation; no whole-Body start or settlement.
use super::*;
use conduit_body::PreparedBodyFragment;
use conduit_core::{HostAdvertisement, Plan};
// Whole-Body lifecycle owns 0/1/2 in the same parent ActivePlay scope.
pub(super) const FIRST_LOCAL_SIGN_SEQUENCE: u64 = 3;

/// Private borrowed containment, never a portable execution grant. The original
/// issuer's cursor and actual local resources are rechecked before stepping.
pub(super) struct IssuingHostScope<'a> {
    body: &'a conduit_body::BodyPlan,
    issued: &'a BodyPlayIdentity,
    anchor: &'a conduit_core::PlanFragment,
    plans: [&'a Plan; conduit_body::MAX_BODY_PLOTS],
    count: usize,
}
impl<'a> IssuingHostScope<'a> {
    fn prepare(
        request: &BodyRunRequest<'_>,
        issued: &'a BodyPlayIdentity,
        body: &'a conduit_body::BodyPlan,
        host: &HostAdvertisement,
    ) -> Result<Self, String> {
        let anchor = body
            .plots
            .iter()
            .map(|p| &p.plan)
            .chain(
                body.mask_topologies
                    .iter()
                    .flat_map(|t| t.chains.iter().map(|c| &c.plan)),
            )
            .flat_map(|p| &p.fragments)
            .find(|f| f.host_id == host.host_id)
            .ok_or("issued Body Plan has no assignment for this Host")?;
        let original = PreparedBodyFragment::prepare(request.wake, body, issued, anchor)
            .map_err(|_| "issued local assignment lacks original full Body containment")?;
        let mut selected = Self {
            body,
            issued,
            anchor,
            plans: [original.plot_plan(); conduit_body::MAX_BODY_PLOTS],
            count: 0,
        };
        for sibling in original.local_fragments() {
            let sibling = sibling.map_err(|_| "ambiguous original local Body assignment")?;
            let f = sibling.fragment();
            if f.boot_id != host.boot_id || f.offer_generation != host.offer_generation {
                return Err(
                    "issued local assignment differs from current Host Boot or generation".into(),
                );
            }
            // Existing local lowering supports whole single-Host Plot/Mask
            // Plans. Refuse unsupported splits instead of dropping siblings.
            if sibling.plot_plan().fragments.len() != 1 {
                return Err(
                    "issued local profile requires whole single-Host constituent Plans".into(),
                );
            }
            if selected.count == selected.plans.len() {
                return Err(
                    "issued local Plot/Mask set exceeds the existing partition bound".into(),
                );
            }
            selected.plans[selected.count] = sibling.plot_plan();
            selected.count += 1;
        }
        Ok(selected)
    }
    pub(super) fn plans(&self) -> &[&'a Plan] {
        &self.plans[..self.count]
    }
    pub(super) fn recheck(
        &self,
        request: &BodyRunRequest<'_>,
        prospective: &BodyPlayIdentity,
        actual: &HostAdvertisement,
    ) -> Result<(), String> {
        if !core::ptr::eq(self.body, request.plan)
            || self.issued != prospective
            || self.anchor.host_id != actual.host_id
            || self.anchor.boot_id != actual.boot_id
            || self.anchor.offer_generation != actual.offer_generation
        {
            return Err(
                "issuing Host assignment lost its original Plan, Play or current Host".into(),
            );
        }
        Ok(())
    }
}

/// Local kernel evidence only. No owned whole-Body report conversion is
/// exposed; completing this work cannot settle remote Plays.
pub struct IssuingHostRunReport {
    local: BodyRunReport,
}
impl IssuingHostRunReport {
    /// Inspect all local kernel evidence, including interrupted delivery and
    /// child/cleanup failures. `wake_at_start` is the ORIGINAL unchanged Wake,
    /// not proof of global Ready/Started. This is a borrow, not Body settlement.
    pub fn local_execution(&self) -> &BodyRunReport {
        &self.local
    }
    pub fn play(&self) -> &BodyPlayIdentity {
        &self.local.play
    }
    pub fn local_terminal(&self) -> &TerminalDisposition {
        &self.local.terminal
    }
    pub fn local_terminal_sign(&self) -> &SignIdentity {
        &self.local.terminal_sign
    }
    pub fn failure(&self) -> Option<&str> {
        self.local.failure.as_deref()
    }
    pub fn cleanup_failure(&self) -> Option<&str> {
        self.local.cleanup_failure.as_deref()
    }
    pub fn fore_deliveries(&self) -> &[ExternalForeDelivery] {
        &self.local.fore_deliveries
    }
    pub fn local_partitions(&self) -> &[KernelIdentityMap] {
        &self.local.partitions
    }
}
impl StdHost {
    /// Execute this ORIGINAL ISSUER's complete local Plot/Mask assignment from
    /// the original full sealed Body Plan. Freeze all remote approvals for this
    /// same issued Play before calling: successful local admission consumes the
    /// issuer cursor once, so a subsequent approval would name a different Play.
    ///
    /// This is not receiving-Host delegation, global admission/Start aggregation,
    /// automatic retries or Body settlement. The callback retains a Host-local
    /// start Sign before effects; it must not call global started/Lull from this
    /// evidence. Current local lowering's unsupported profiles still refuse.
    #[allow(clippy::too_many_arguments)]
    pub fn run_issuing_host_plan_with_fore_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        issued: &BodyPlayIdentity,
        inputs: &[ExternalForeInput],
        fore_output: &mut dyn BodyForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
        mut started: F,
    ) -> Result<IssuingHostRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &SignIdentity) -> Result<(), String>,
    {
        let scope = IssuingHostScope::prepare(&request, issued, request.plan, &self.advertisement)?;
        let host = self.advertisement.host_id.clone();
        let boot = self.advertisement.boot_id.clone();
        let start_sequence = self
            .next_kernel_sign_sequence
            .max(FIRST_LOCAL_SIGN_SEQUENCE);
        let local = self.run_body_plan_to_with_start_and_clock_and_creche(
            request,
            output,
            timer,
            None,
            None,
            Some((inputs, false, fore_output)),
            None,
            None,
            None,
            None,
            |play, _| {
                started(
                    play,
                    &bind_sign(&host, &boot, Some(&play.active_play_id), start_sequence),
                )
            },
            None,
            Some(&scope),
        )?;
        Ok(IssuingHostRunReport { local })
    }
}
