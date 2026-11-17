//! Complete-set membership, fixed causal scope and actual funded preparation.
use super::*;
use crate::installed_std::body_kernel::funded::TickPreparationLayout;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
impl StdHost {
    pub fn prepare_delegated_body<T: TimerAdapter>(
        &mut self,
        request: DelegatedBodyRequest<'_>,
        timer: &mut T,
    ) -> Result<PreparedDelegatedBody, String> {
        // This borrowed descriptor and scope check precede every owned receipt.
        let run = BodyRunRequest {
            wake: request.wake,
            plan: request.plan,
            control: request.control,
            keyboard: None,
        };
        let layout = TickPreparationLayout::prepare_delegated(
            &run,
            request.global_play,
            &self.advertisement,
            &self.kernel_resources,
            timer,
            self.playback.as_ref(),
        )?;
        let memory = self
            .kernel_resources
            .fund_tick_preparation(layout.peak_bytes)?;
        request
            .plan
            .validate_for_borrowed(request.wake)
            .map_err(|e| format!("delegated Body Plan: {e:?}"))?;
        let scope =
            super::super::startup::PreparedStartupScope::new(request.plan, request.global_play)?;
        // Every actual Plot and Mask partition must be selected on this same
        // member incarnation. No local-fragments subset is promoted to a set.
        for part in request.plan.plots.iter().map(|p| &p.plan).chain(
            request
                .plan
                .mask_topologies
                .iter()
                .flat_map(|t| &t.chains)
                .map(|c| &c.plan),
        ) {
            for fragment in &part.fragments {
                if fragment.host_id != self.advertisement.host_id
                    || fragment.boot_id != self.advertisement.boot_id
                    || fragment.offer_generation != self.advertisement.offer_generation
                {
                    return Err("delegated workset is not entirely on this current member".into());
                }
            }
        }
        self.next_kernel_play_sequence
            .checked_add(1)
            .ok_or("member Play sequence exhausted")?;
        let member_play = conduit_core::bind_active_play(
            &request.plan.plan_id,
            &self.advertisement.host_id,
            &self.advertisement.boot_id,
            self.next_kernel_play_sequence,
        );
        let mut hash = Sha256::new();
        for value in [
            request.global_play.active_play_id.as_str(),
            member_play.active_play_id.as_str(),
            request.plan.plan_id.as_str(),
        ] {
            hash.update((value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        }
        hash.update(self.advertisement.offer_generation.0.to_le_bytes());
        let mut preparation_id = String::with_capacity(64);
        for byte in hash.finalize() {
            write!(&mut preparation_id, "{byte:02x}").expect("String formatting");
        }
        let receipt = DelegatedBodyPreparationReceipt {
            preparation_id,
            global_play: request.global_play.clone(),
            member_play,
            host_id: self.advertisement.host_id.clone(),
            boot_id: self.advertisement.boot_id.clone(),
            offer_generation: self.advertisement.offer_generation,
            plan_id: request.plan.plan_id.clone(),
        };
        let terminal = DelegatedBodyTerminalReceipt {
            preparation_id: receipt.preparation_id.clone(),
            global_play: receipt.global_play.clone(),
            member_play: receipt.member_play.clone(),
            host_id: receipt.host_id.clone(),
            boot_id: receipt.boot_id.clone(),
            offer_generation: receipt.offer_generation,
            plan_id: receipt.plan_id.clone(),
            terminal: TerminalDisposition::Completed,
            kernel_failure: None,
            failure: None,
            cleanup_failure: None,
            kernel_signs: 0,
            startup_chimes: Vec::new(),
        };
        let mut kernel = BodyKernel::prepare_funded(
            &request.plan.plots,
            &receipt.member_play.active_play_id,
            false,
            &layout,
            &self.advertisement,
            self.playback.as_ref(),
        )?;
        kernel.require_supported_execution()?;
        let first = &request.plan.plots[0].plan;
        let mut plans = [(first, false); conduit_body::MAX_BODY_PLOTS];
        for (entry, part) in plans.iter_mut().zip(&request.plan.plots) {
            *entry = (&part.plan, false);
        }
        let extra =
            preparation_backing_bytes(request.plan, request.global_play, &self.advertisement)?;
        let reservations = super::super::prepared::reserve_prepared_kernel(
            &mut kernel,
            timer,
            &self.advertisement,
            &mut self.kernel_resources,
            &plans[..request.plan.plots.len()],
            extra,
            layout.clock_identity_bytes,
            &memory,
        )?;
        let execution = kernel.execution_state(None, None);
        Ok(PreparedDelegatedBody {
            receipt,
            terminal,
            scope,
            kernel: Some(kernel),
            execution: Some(execution),
            reservations,
            memory,
            committed: false,
            finished: false,
            cancelled: false,
            released: false,
        })
    }
}
