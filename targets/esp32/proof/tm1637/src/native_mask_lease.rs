//! Root-created finite child leases within one admitted local Body Play.
//! No received descriptor or public constructor can create this authority.
use super::RootIssuer;
use alloc::{boxed::Box, sync::Arc};
use conduit_core as c;
use conduit_environment_runtime::mask_renderer as r;
use conduit_tm1637_wire::OpenDrain;
use core::sync::atomic::{AtomicBool, Ordering};
use sha2::{Digest, Sha256};

type Lease = (
    c::ActivePlayIdentity,
    c::BaseCapabilityTable,
    c::BaseCapabilityHandle,
    c::BaseOperationClaim,
);
const CHILDREN: u64 = 960 * 60_000 / 3_000;
const HORIZON_MS: u64 = 960 * 60_000;
/// Root retains this revocation witness while the worker owns issuance. Every
/// Body mutation must invalidate it before changing the current sealed scope.
pub struct MaskScopeGuard(Arc<AtomicBool>);
impl Drop for MaskScopeGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
impl MaskScopeGuard {
    pub fn revoke(&self) {
        self.0.store(false, Ordering::Release);
    }
}
pub struct MaskLeaseIssuer {
    key: [u8; 32],
    native: r::RendererIdentity,
    grant: c::AuthorityGrant,
    plan: c::PlanId,
    next: u64,
    end: u64,
    pending: Option<c::ActivePlayIdentity>,
    staged: Option<c::ActivePlayIdentity>,
    first: Option<Lease>,
    current: Arc<AtomicBool>,
    start_ms: Option<u64>,
    issuance_origin_ms: Option<u64>,
    issued_children: u64,
    parent: Option<conduit_body::BodyPlayIdentity>,
}
impl RootIssuer {
    pub fn prepare_mask_leases<P: OpenDrain>(
        &mut self,
        owner: &r::ReadyRenderer<P>,
        configuration: &[c::ConfigurationEntry],
        selected: conduit_plan_lowering::selected_operation::SelectedVerifiedForeOperationPlan<'_>,
    ) -> Result<
        (
            MaskLeaseIssuer,
            r::PreparedRendererInvocation,
            MaskScopeGuard,
        ),
        &'static str,
    > {
        let active = selected.operation.active;
        self.prepare_mask_leases_with(owner, active, |table, claim| {
            owner.prepare_verified_invocation(table, claim, configuration, selected)
        })
    }
    #[cfg(any(feature = "target-mask-memory-check", test))]
    pub fn prepare_mask_leases_direct<P: OpenDrain>(
        &mut self,
        owner: &r::ReadyRenderer<P>,
        configuration: &r::RendererSourceConfiguration,
        selected: r::DirectRendererSelection<'_>,
    ) -> Result<
        (
            MaskLeaseIssuer,
            r::PreparedRendererInvocation,
            MaskScopeGuard,
        ),
        &'static str,
    > {
        let active = selected.active;
        self.prepare_mask_leases_with(owner, active, |table, claim| {
            owner.prepare_direct_invocation(table, claim, configuration, selected)
        })
    }
    fn prepare_mask_leases_with<P: OpenDrain>(
        &mut self,
        owner: &r::ReadyRenderer<P>,
        active: &c::ActivePlayIdentity,
        prepare: impl FnOnce(
            &c::BaseCapabilityTable,
            &c::BaseOperationClaim,
        ) -> Result<r::PreparedRendererInvocation, &'static str>,
    ) -> Result<
        (
            MaskLeaseIssuer,
            r::PreparedRendererInvocation,
            MaskScopeGuard,
        ),
        &'static str,
    > {
        let native = owner.identity();
        if self.display_issued
            || self.reserved_display_play.is_some()
            || native.host_id != self.host
            || native.boot_id != self.boot
            || Some(active.play_sequence) != self.next_play_sequence()
            || c::bind_active_play(
                &active.plan_id,
                &self.host,
                &self.boot,
                active.play_sequence,
            ) != *active
        {
            return Err("mask-parent-reservation");
        }
        let end = active
            .play_sequence
            .checked_add(CHILDREN)
            .ok_or("mask-child-sequence-exhausted")?;
        let witness = Arc::new(AtomicBool::new(false));
        let mut leases = MaskLeaseIssuer {
            key: self.key,
            native: native.clone(),
            grant: self.grant_mask(native).map_err(|_| "mask-grant")?,
            plan: active.plan_id.clone(),
            next: active.play_sequence,
            end,
            pending: None,
            staged: None,
            first: None,
            current: witness.clone(),
            start_ms: None,
            issuance_origin_ms: None,
            issued_children: 0,
            parent: None,
        };
        let first = leases.issue()?;
        // Exact actual native/Fore/configuration/ordinary numeric checks run
        // before consuming Root's range or publishing this private issuer.
        let capsule = prepare(&first.1, &first.3)?;
        leases.pending = None;
        leases.first = Some(first);
        self.last_play_sequence = end - 1;
        self.display_issued = true;
        self.display_generation = native.provider_generation;
        self.issued_display_play = Some(Box::new(active.clone()));
        Ok((leases, capsule, MaskScopeGuard(witness)))
    }
}
pub(crate) struct RetirementFacts {
    pub issued: u64,
    pub first: u64,
    pub end: u64,
    pub next: u64,
    pub revoked: bool,
    pub pending: bool,
    pub first_retained: bool,
}
impl MaskLeaseIssuer {
    pub(crate) fn fresh_for_task(&self) -> bool {
        self.issued_children == 0
            && self.first.is_some()
            && self.pending.is_none()
            && self.staged.is_none()
            && !self.current.load(Ordering::Acquire)
            && self.start_ms.is_none()
            && self.parent.is_none()
            && self.issuance_origin_ms.is_none()
    }
    pub(crate) fn retirement_facts<P: OpenDrain>(
        &self,
        owner: &r::ReadyRenderer<P>,
    ) -> Result<RetirementFacts, &'static str> {
        if owner.identity() != &self.native
            || self.parent.is_none()
            || self.start_ms.is_none()
            || self.issuance_origin_ms.is_none()
        {
            return Err("mask-terminal-owner-scope");
        }
        Ok(RetirementFacts {
            issued: self.issued_children,
            first: self
                .end
                .checked_sub(CHILDREN)
                .ok_or("mask-terminal-range")?,
            end: self.end,
            next: self.next,
            revoked: !self.current.load(Ordering::Acquire),
            pending: self.pending.is_some() || self.staged.is_some(),
            first_retained: self.first.is_some(),
        })
    }
    pub(crate) fn matches_current_body<P: OpenDrain>(
        &self,
        owner: &r::ReadyRenderer<P>,
        body: &conduit_body::SealedBodyLifecycleSession,
        initial: &c::ActivePlayIdentity,
    ) -> bool {
        let Some(realization) = body.realization() else {
            return false;
        };
        let Some(parent) = realization.play.as_ref() else {
            return false;
        };
        let Some(first) = self.end.checked_sub(CHILDREN) else {
            return false;
        };
        owner.identity() == &self.native
            && self.parent.as_ref() == Some(parent)
            && parent.validate_for(&realization.plan)
            && *initial
                == c::bind_active_play(
                    &self.plan,
                    &self.native.host_id,
                    &self.native.boot_id,
                    first,
                )
            && realization
                .plan
                .masks()
                .iter()
                .flat_map(|m| m.chains())
                .any(|chain| chain.plan().plan_id() == &self.plan)
            && body.evidence().membership.parts.iter().any(|part| {
                part.is_present()
                    && part.current.as_ref().is_some_and(|host| {
                        host.host_id == self.native.host_id && host.boot_id == self.native.boot_id
                    })
            })
    }
    pub(crate) fn plan_id(&self) -> &c::PlanId {
        &self.plan
    }
    pub fn revocation_guard(&self) -> MaskScopeGuard {
        MaskScopeGuard(self.current.clone())
    }
    /// Called by Root only after both actual held tasks were admitted and the
    /// canonical Body.started transition (and durable checkpoint) succeeded.
    pub fn activate(
        &mut self,
        body: &conduit_body::SealedBodyLifecycleSession,
        now_ms: u64,
    ) -> Result<(), &'static str> {
        if self.start_ms.is_some() || self.current.load(Ordering::Acquire) {
            return Err("mask-parent-already-active");
        }
        let realization = body.realization().ok_or("mask-parent-not-planned")?;
        let play = realization.play.as_ref().ok_or("mask-parent-not-started")?;
        if !play.validate_for(&realization.plan)
            || !realization
                .plan
                .masks()
                .iter()
                .flat_map(|topology| topology.chains())
                .any(|chain| chain.plan().plan_id() == &self.plan)
            || !body.evidence().membership.parts.iter().any(|part| {
                part.is_present()
                    && part.current.as_ref().is_some_and(|host| {
                        host.host_id == self.native.host_id && host.boot_id == self.native.boot_id
                    })
            })
        {
            return Err("mask-parent-scope");
        }
        self.parent = Some(play.clone());
        self.start_ms = Some(now_ms);
        self.current.store(true, Ordering::Release);
        Ok(())
    }
    pub fn next<P: OpenDrain>(
        &mut self,
        owner: &r::ReadyRenderer<P>,
        now_ms: u64,
    ) -> Result<Lease, &'static str> {
        let start = self.start_ms.ok_or("mask-parent-held")?;
        if !self.current.load(Ordering::Acquire)
            || self.parent.is_none()
            || now_ms < start
            || now_ms - start >= HORIZON_MS
            || self.pending.is_some()
            || owner.identity() != &self.native
        {
            return Err("mask-parent-not-current");
        }
        // Fixed windows permit ordinary executor jitter without sliding the
        // cadence. A whole missed window fails closed; no burst catch-up.
        let origin = self.issuance_origin_ms.unwrap_or(now_ms);
        let due = self
            .issued_children
            .checked_mul(3000)
            .and_then(|offset| origin.checked_add(offset))
            .ok_or("mask-child-clock")?;
        if now_ms < due || now_ms.saturating_sub(due) >= 3000 {
            return Err("mask-child-window");
        }
        let lease = match self.first.take() {
            Some(first) => first,
            None => self.issue()?,
        };
        self.pending = Some(lease.0.clone());
        self.issuance_origin_ms = Some(origin);
        self.issued_children += 1;
        Ok(lease)
    }
    pub fn validate_retired<P: OpenDrain>(
        &mut self,
        owner: &r::ReadyRenderer<P>,
        active: &c::ActivePlayIdentity,
        inspection: &c::CapabilityInspection,
    ) -> Result<(), &'static str> {
        if self.pending.as_ref() != Some(active)
            || owner.identity() != &self.native
            || inspection.lifecycle != c::CapabilityLifecycle::Revoked
            || inspection.in_flight_operations != 0
            || inspection.completed_operations > 2
            || inspection.scope != self.scope(active)
        {
            return Err("mask-child-not-retired");
        }
        self.pending = None;
        Ok(())
    }
    /// One successor only, while the old actual child retains cleanup. This
    /// grants no second physical owner and cannot slide the fixed cadence.
    pub fn stage_successor(
        &mut self,
        native: &r::RendererIdentity,
        predecessor: &c::ActivePlayIdentity,
        now_ms: u64,
    ) -> Result<Option<Lease>, &'static str> {
        let start = self.start_ms.ok_or("mask-parent-held")?;
        if !self.current.load(Ordering::Acquire)
            || self.parent.is_none()
            || now_ms < start
            || now_ms - start >= HORIZON_MS
            || self.pending.as_ref() != Some(predecessor)
            || self.staged.is_some()
            || self.first.is_some()
            || native != &self.native
        {
            return Err("mask-successor-scope");
        }
        if self.next >= self.end {
            return Ok(None);
        }
        let due = self
            .issued_children
            .checked_mul(3000)
            .and_then(|n| self.issuance_origin_ms?.checked_add(n))
            .ok_or("mask-child-clock")?;
        if now_ms < due || now_ms - due >= 3000 {
            return Err("mask-child-window");
        }
        let lease = self.issue()?;
        self.staged = Some(lease.0.clone());
        self.issued_children += 1;
        Ok(Some(lease))
    }
    pub fn validate_replaced(
        &mut self,
        native: &r::RendererIdentity,
        previous: &c::ActivePlayIdentity,
        previous_inspection: &c::CapabilityInspection,
        current: &c::CapabilityInspection,
    ) -> Result<(), &'static str> {
        let next = self.staged.as_ref().ok_or("mask-successor-unavailable")?;
        if native != &self.native
            || self.pending.as_ref() != Some(previous)
            || previous_inspection.scope != self.scope(previous)
            || previous_inspection.lifecycle != c::CapabilityLifecycle::Revoked
            || previous_inspection.completed_operations != 1
            || previous_inspection.in_flight_operations != 0
            || current.scope != self.scope(next)
            || !matches!(
                (current.lifecycle, current.completed_operations),
                (c::CapabilityLifecycle::Issued, 1) | (c::CapabilityLifecycle::Revoked, 2)
            )
            || current.in_flight_operations != 0
        {
            return Err("mask-successor-not-accepted");
        }
        self.pending = self.staged.take();
        Ok(())
    }
    fn scope(&self, active: &c::ActivePlayIdentity) -> c::BaseCapabilityScope {
        let native = &self.native;
        c::BaseCapabilityScope {
            host_id: native.host_id.clone(),
            boot_id: native.boot_id.clone(),
            base_instance_id: native.provider_instance_id.clone(),
            base_provider_generation: native.provider_generation,
            plan_id: self.plan.clone(),
            active_play_id: active.active_play_id.clone(),
            authority_grant_id: self.grant.grant_id.clone(),
            authority_contract_id: self.grant.contract_id.clone(),
            capability_id: r::CAPABILITY.into(),
            implementation_id: native.implementation_id.clone(),
            operation_contract_id: r::CALL.into(),
            subject_kind: conduit_face::RENDERER_KIND.into(),
            resource_pool_id: native.resource_pool_id.clone(),
            resource_generation_id: native.resource_generation_id.clone(),
            envelope_id: native.envelope_id.clone(),
            maximum_parameter_bytes: 8192,
            maximum_result_bytes: 8192,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 2,
        }
    }
    fn issue(&mut self) -> Result<Lease, &'static str> {
        if self.next >= self.end {
            return Err("mask-child-horizon-exhausted");
        }
        let active = c::bind_active_play(
            &self.plan,
            &self.native.host_id,
            &self.native.boot_id,
            self.next,
        );
        let scope = self.scope(&active);
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/mask-child-possession@1");
        hash.update(self.key);
        hash.update(active.active_play_id.as_str().as_bytes());
        let mut table = c::BaseCapabilityTable::new(
            scope.host_id.clone(),
            scope.boot_id.clone(),
            scope.base_instance_id.clone(),
            scope.base_provider_generation,
            hash.finalize().into(),
            1,
        )
        .map_err(|_| "mask-child-table")?;
        let authority = c::BaseCapabilityAuthority {
            grant: self.grant.clone(),
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            operation_contract_id: scope.operation_contract_id.clone(),
            envelope_id: scope.envelope_id.clone(),
            maximum_parameter_bytes: 8192,
            maximum_result_bytes: 8192,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 2,
        };
        let claim = c::BaseOperationClaim {
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            plan_id: scope.plan_id.clone(),
            active_play_id: scope.active_play_id.clone(),
            implementation_id: scope.implementation_id.clone(),
            operation_contract_id: scope.operation_contract_id.clone(),
            subject_kind: scope.subject_kind.clone(),
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            envelope_id: scope.envelope_id.clone(),
            parameter_bytes: 8192,
            work_units: 1,
        };
        let handle = table
            .issue(c::CapabilityIssueRequest { scope, authority })
            .map_err(|_| "mask-child-issue")?;
        self.next += 1;
        Ok((active, table, handle, claim))
    }
}
#[cfg(any(target_arch = "xtensa", feature = "local-appliance"))]
impl crate::native_display::LeaseSource for MaskLeaseIssuer {
    fn current(&self, now_ms: u64) -> bool {
        self.current.load(Ordering::Acquire)
            && self.parent.is_some()
            && self
                .start_ms
                .is_some_and(|start| now_ms >= start && now_ms - start < HORIZON_MS)
    }
    fn next(
        &mut self,
        owner: &r::ReadyRenderer<crate::electrical::DisplayPins<'static>>,
        now_ms: u64,
    ) -> Result<Option<crate::native_display::Lease>, &'static str> {
        if self.next >= self.end && self.first.is_none() {
            return Ok(None);
        }
        MaskLeaseIssuer::next(self, owner, now_ms).map(Some)
    }
    fn stage_successor(
        &mut self,
        native: &r::RendererIdentity,
        predecessor: &c::ActivePlayIdentity,
        now_ms: u64,
    ) -> Result<Option<crate::native_display::Lease>, &'static str> {
        MaskLeaseIssuer::stage_successor(self, native, predecessor, now_ms)
    }
    fn replaced(
        &mut self,
        native: &r::RendererIdentity,
        previous: &c::ActivePlayIdentity,
        inspection: &c::CapabilityInspection,
        current: &c::CapabilityInspection,
    ) -> Result<(), &'static str> {
        self.validate_replaced(native, previous, inspection, current)
    }
    fn retired(
        &mut self,
        owner: &r::ReadyRenderer<crate::electrical::DisplayPins<'static>>,
        active: &c::ActivePlayIdentity,
        inspection: &c::CapabilityInspection,
    ) -> Result<(), &'static str> {
        self.validate_retired(owner, active, inspection)
    }
}

#[path = "native_mask_finish.rs"]
mod finish;
