//! Authenticated cold ordinary Mask preparation before the sampler owns RAM.
//! Body workload ownership stays with the admitted Linux owner; no effect/start.
use super::NativeRoot;
use crate::{native_owner_control as owner, native_owner_receipt as ack};
use alloc::boxed::Box;
use conduit_core as c;
#[path = "native_mask_source.rs"]
mod source_check;
pub(super) mod material {
    include!(concat!(env!("OUT_DIR"), "/mask-source.rs"));
}
impl NativeRoot {
    pub fn mask_policy(&self, output: &mut [u8]) -> Result<usize, ()> {
        #[derive(serde::Serialize)]
        struct Policy<'a> {
            event: &'static str,
            host_id: &'a c::HostId,
            boot_id: &'a c::BootId,
            offer_generation: c::OfferGeneration,
            source: c::PlotIdentity,
            authority_grants: [c::AuthorityGrant; 1],
            capabilities: alloc::vec::Vec<c::CapabilityOffer>,
            context_staged: bool,
            kernel_prepared: bool,
            physical_mask_plot_installed: bool,
        }
        let ready = self.display.renderer().ok_or(())?;
        let mut software = self.advertisement()?;
        let start = software.capabilities.len();
        self.display.append_software_offers(&mut software);
        let capabilities = software.capabilities.split_off(start);
        serde_json_core::to_slice(
            &Policy {
                event: "native-mask-policy",
                host_id: &self.host,
                boot_id: &self.boot,
                offer_generation: c::OfferGeneration(self.offer_generation),
                source: material::source(),
                authority_grants: [self.issuer.grant_mask(ready.identity())?],
                capabilities,
                context_staged: self.display.context_staged(),
                kernel_prepared: self.display.kernel_prepared(),
                physical_mask_plot_installed: false,
            },
            output,
        )
        .map_err(|_| ())
    }
    pub fn retire_mask_context(
        &mut self,
        token: owner::AuthenticatedRequest<'_>,
    ) -> Result<ack::Receipt, &'static str> {
        let request = token.into_request();
        let reservation = self.display_reservation.as_ref().ok_or("mask-unreserved")?;
        reservation.check_request(&request, owner::Operation::Retire)?;
        let generation = self
            .offer_generation
            .checked_add(1)
            .ok_or("mask-offer-generation-exhausted")?;
        self.display.retire_preparation()?;
        self.issuer.retire_mask_context(&reservation.active)?;
        self.display_reservation = None;
        self.offer_generation = generation;
        Ok(ack::Receipt {
            command: request.transcript().map_err(|_| "mask-command-scope")?,
            state: ack::State::Retired,
            snapshot: None,
        })
    }
    pub fn stage_mask_context(
        &mut self,
        token: owner::AuthenticatedRequest<'_>,
        wire: Box<[u8]>,
    ) -> Result<ack::Receipt, &'static str> {
        if token.operation() != owner::Operation::Prepare {
            return Err("mask-prepare-operation");
        }
        if self.appliance_kernel.is_some()
            || self.execution.is_some()
            || self.worker.is_some()
            || self.display.context_staged()
            || self.display.kernel_prepared()
            || self.display_reservation.is_some()
        {
            return Err("mask-context-must-precede-sampler");
        }
        let request = token.into_request();
        if request.offer_generation != self.offer_generation {
            return Err("mask-current-generation");
        }
        let parent = self
            .owner_reservation
            .as_ref()
            .ok_or("sampler-unreserved")?;
        if request.body != parent.body
            || request.wake != parent.wake
            || request.body_plan != parent.body_plan
            || request.body_play != parent.body_play
        {
            return Err("mask-body-scope");
        }
        if c::assigned_plan_payload_digest(&wire) != request.assignment_seal {
            return Err("mask-plan-wire-seal");
        }
        let withdrawn = self
            .offer_generation
            .checked_add(1)
            .ok_or("mask-offer-generation-exhausted")?;
        let stats = esp_alloc::HEAP.stats();
        if stats.size.saturating_sub(stats.current_usage) < 56 * 1024 {
            return Err("mask-cold-preparation-capacity");
        }
        let mut advertisement = self.advertisement().map_err(|_| "mask-current-offer")?;
        self.display.append_software_offers(&mut advertisement);
        let ready = self.display.renderer().ok_or("mask-provider-unavailable")?;
        let grants = [self
            .issuer
            .grant_mask(ready.identity())
            .map_err(|_| "mask-grant")?];
        let role = material::role().map_err(|_| "mask-resident-role")?;
        let mut prepared = None;
        let mut scope = None;
        let next = self
            .issuer
            .next_play_sequence()
            .ok_or("mask-play-exhausted")?;
        let context = conduit_face::PreparedMaskShowContext::from_wire_checked(
            wire,
            8192,
            32768,
            |planned| {
                if planned.mask != role
                    || !source_check::matches(
                        planned,
                        &material::CONFIGURATIONS,
                        &material::CONNECTIONS,
                    )
                    || conduit_face::validate_local_mask_offer(&advertisement, planned, &grants)
                        .is_err()
                {
                    return false;
                }
                let [fragment] = planned.plan.fragments.as_slice() else {
                    return false;
                };
                let active =
                    c::bind_active_play(&planned.plan.plan_id, &self.host, &self.boot, next);
                if request.plan != planned.plan.plan_id.as_str()
                    || request.fragment != fragment.fragment_id.as_str()
                    || request.fragment_play != active.active_play_id.as_str()
                {
                    return false;
                }
                let Ok(kernel) =
                    conduit_environment_runtime::mask_kernel::PreparedMaskKernel::prepare(planned)
                else {
                    return false;
                };
                prepared = Some(kernel);
                scope = Some((fragment.fragment_id.clone(), active));
                true
            },
        )
        .map_err(|_| "mask-context-or-source-admission")?;
        let (fragment, active) = scope.ok_or("mask-context-scope")?;
        self.issuer.reserve_mask_context(&active)?;
        self.display
            .retain_preparation(context, prepared.ok_or("mask-kernel-unavailable")?)?;
        self.offer_generation = withdrawn;
        self.display_reservation = Some(crate::native_owner_reservation::OwnerReservation {
            offer_generation: request.offer_generation,
            body: request.body.into(),
            wake: request.wake.into(),
            body_plan: request.body_plan.into(),
            body_play: request.body_play.into(),
            fragment,
            active,
            assignment_seal: request.assignment_seal,
        });
        Ok(ack::Receipt {
            command: request.transcript().map_err(|_| "mask-command-scope")?,
            state: ack::State::MaskContextStaged,
            snapshot: None,
        })
    }
}
