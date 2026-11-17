//! Original owner's lifecycle acknowledgment after real member preparation.
use super::*;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
impl StdHost {
    /// Cooperative owner trust boundary: caller must verify this exact receipt
    /// came from its retained actual prepared member over the authenticated
    /// carrier. A deserialized receipt alone supplies no such authority.
    /// This advances only the owner's canonical lifecycle, never a local kernel.
    pub fn acknowledge_delegated_body_preparation(
        &mut self,
        wake: &Wake,
        plan: &BodyPlan,
        receipt: &DelegatedBodyPreparationReceipt,
    ) -> Result<(BodyPlayIdentity, Wake), String> {
        plan.validate_for_borrowed(wake)
            .map_err(|e| format!("delegated owner Body Plan: {e:?}"))?;
        let next = self
            .next_kernel_play_sequence
            .checked_add(1)
            .ok_or("owner Body Play sequence exhausted")?;
        let play = BodyPlayIdentity::bind(plan, self.next_kernel_play_sequence);
        if receipt.global_play != play
            || receipt.plan_id != plan.plan_id
            || receipt.member_play.plan_id != plan.plan_id
            || receipt.member_play.host_id != receipt.host_id
            || receipt.member_play.boot_id != receipt.boot_id
            || receipt.offer_generation.0 == 0
            || receipt.member_play.play_sequence.checked_add(1).is_none()
        {
            return Err("authenticated preparation differs from original owner scope".into());
        }
        let member = conduit_core::bind_active_play(
            &plan.plan_id,
            &receipt.host_id,
            &receipt.boot_id,
            receipt.member_play.play_sequence,
        );
        if member != receipt.member_play {
            return Err("prepared member Play binding differs".into());
        }
        for part in plan.plots.iter().map(|p| &p.plan).chain(
            plan.mask_topologies
                .iter()
                .flat_map(|t| &t.chains)
                .map(|c| &c.plan),
        ) {
            for fragment in &part.fragments {
                if fragment.host_id != receipt.host_id
                    || fragment.boot_id != receipt.boot_id
                    || fragment.offer_generation != receipt.offer_generation
                {
                    return Err(
                        "authenticated member preparation omits original workset scope".into(),
                    );
                }
            }
        }
        let mut hash = Sha256::new();
        for value in [
            play.active_play_id.as_str(),
            member.active_play_id.as_str(),
            plan.plan_id.as_str(),
        ] {
            hash.update((value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        }
        hash.update(receipt.offer_generation.0.to_le_bytes());
        let mut preparation_id = String::with_capacity(64);
        for byte in hash.finalize() {
            write!(&mut preparation_id, "{byte:02x}").expect("String formatting");
        }
        if preparation_id != receipt.preparation_id {
            return Err("authenticated prepared set identity differs".into());
        }
        let sign = |sequence| {
            bind_sign(
                &self.advertisement.host_id,
                &self.advertisement.boot_id,
                Some(&play.active_play_id),
                sequence,
            )
            .sign_id
        };
        let started = wake
            .body_plan_ready_borrowed(plan, sign(0))
            .and_then(|wake| wake.body_play_started_borrowed(plan, &play, sign(1)))
            .map_err(|e| format!("delegated owner lifecycle: {e:?}"))?;
        self.next_kernel_play_sequence = next;
        Ok((play, started))
    }
}
