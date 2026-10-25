//! Root permission and bounded possession for the retained display owner.
use super::*;
use conduit_tm1637_wire::{installation::*, WRITE_CALL, WRITE_KIND};
impl RootIssuer {
    pub fn grant_display(&self, native: &WriteNativeIdentity) -> Result<c::AuthorityGrant, ()> {
        if native.host_id != self.host || native.boot_id != self.boot {
            return Err(());
        }
        Ok(c::AuthorityGrant {
            grant_id: "grant/esp32/tm1637-write/1".into(),
            contract_id: WRITE_AUTHORITY.into(),
            host_call_contract_id: WRITE_CALL.into(),
            subject_kind: WRITE_KIND.into(),
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            capability_id: WRITE_CAPABILITY.into(),
        })
    }
}

#[cfg(any(
    test,
    any(feature = "display-ready-root", feature = "display-physical-check")
))]
impl RootIssuer {
    /// Reserve identity only for independently admitted output tables. Owner
    /// authentication, native possession and Body start remain separate gates.
    pub fn reserve_display_play_matching(
        &mut self,
        assignment: &conduit_environment_runtime::display_output::ValidatedAssignedDisplay,
        plan: &c::PlanId,
        fragment: &c::FragmentId,
        expected: &str,
    ) -> Result<c::ActivePlayIdentity, ()> {
        let identity = assignment.identity();
        let id = c::AssignedIdentity::from_text;
        if identity.host != id(self.host.as_str())
            || identity.boot != id(self.boot.as_str())
            || identity.plan != id(plan.as_str())
            || identity.fragment != id(fragment.as_str())
        {
            return Err(());
        }
        let next = self.next_play_sequence().ok_or(())?;
        let candidate = c::bind_active_play(plan, &self.host, &self.boot, next);
        if candidate.active_play_id.as_str() != expected {
            return Err(());
        }
        if self.reserved_display_play.is_some() || self.display_issued {
            return Err(());
        }
        self.last_play_sequence = next;
        self.reserved_display_play = Some(alloc::boxed::Box::new(candidate.clone()));
        Ok(candidate)
    }
}

#[cfg(any(
    test,
    any(feature = "display-ready-root", feature = "display-physical-check")
))]
impl RootIssuer {
    /// Finite Root-selected operation budget. This issues no Body start and
    /// performs no electrical operation; the actual owner is checked first.
    pub fn issue_assigned_display<P: conduit_tm1637_wire::OpenDrain>(
        &mut self,
        owner: &ReadyWriteBase<P>,
        assignment: &conduit_environment_runtime::display_output::ValidatedAssignedDisplay,
        selected: conduit_plan_lowering::selected_assigned_operation::SelectedAssignedForeOperationPlan<'_>,
        maximum_frames: u32,
        contract: &conduit_tm1637_wire::WriteContract,
    ) -> Result<
        (
            c::BaseCapabilityTable,
            c::BaseCapabilityHandle,
            c::BaseOperationClaim,
        ),
        (),
    > {
        let identity = assignment.identity();
        let active = selected.operation.active;
        let native = owner.native_identity();
        let id = c::AssignedIdentity::from_text;
        let fore = assignment.fore();
        if maximum_frames == 0
            || self.display_issued
            || native.provider_generation <= self.display_generation
            || self.reserved_display_play.as_deref() != Some(active)
            || *active
                != c::bind_active_play(
                    &active.plan_id,
                    &self.host,
                    &self.boot,
                    active.play_sequence,
                )
            || identity.host != id(self.host.as_str())
            || identity.boot != id(self.boot.as_str())
            || identity.plan != id(active.plan_id.as_str())
            || identity.fragment != id(selected.operation.fragment_id.as_str())
            || selected.fore.len() != 2
            || !fore.iter().all(|f| selected.fore.contains(f))
        {
            return Err(());
        }
        let grant = self.grant_display(native)?;
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/display-possession@1");
        hash.update(self.key);
        hash.update(native.provider_instance_id.as_str().as_bytes());
        let mut table = c::BaseCapabilityTable::new(
            self.host.clone(),
            self.boot.clone(),
            native.provider_instance_id.clone(),
            native.provider_generation,
            hash.finalize().into(),
            1,
        )
        .map_err(|_| ())?;
        let scope = c::BaseCapabilityScope {
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            base_instance_id: native.provider_instance_id.clone(),
            base_provider_generation: native.provider_generation,
            plan_id: active.plan_id.clone(),
            active_play_id: active.active_play_id.clone(),
            authority_grant_id: grant.grant_id.clone(),
            authority_contract_id: grant.contract_id.clone(),
            capability_id: WRITE_CAPABILITY.into(),
            implementation_id: native.implementation_id.clone(),
            operation_contract_id: WRITE_CALL.into(),
            subject_kind: WRITE_KIND.into(),
            resource_pool_id: native.resource_pool_id.clone(),
            resource_generation_id: native.resource_generation_id.clone(),
            envelope_id: native.envelope_id.clone(),
            maximum_parameter_bytes: conduit_tm1637_wire::MAXIMUM_BYTES,
            maximum_result_bytes: 0,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: maximum_frames,
        };
        let authority = c::BaseCapabilityAuthority {
            grant,
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            operation_contract_id: scope.operation_contract_id.clone(),
            envelope_id: scope.envelope_id.clone(),
            maximum_parameter_bytes: scope.maximum_parameter_bytes,
            maximum_result_bytes: 0,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: maximum_frames,
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
            parameter_bytes: scope.maximum_parameter_bytes,
            work_units: 1,
        };
        let handle = table
            .issue(c::CapabilityIssueRequest { scope, authority })
            .map_err(|_| ())?;
        owner
            .check_assigned_fore(&table, &claim, contract, selected)
            .map_err(|_| ())?;
        self.display_issued = true;
        self.display_generation = native.provider_generation;
        self.issued_display_play = Some(alloc::boxed::Box::new(active.clone()));
        Ok((table, handle, claim))
    }
}

#[cfg(any(
    test,
    any(feature = "display-ready-root", feature = "display-physical-check")
))]
impl RootIssuer {
    /// End issuance only with exact returned provider custody. Provider
    /// generation and the global Play sequence remain monotonic.
    pub fn finish_display_play<P>(
        &mut self,
        active: &c::ActivePlayIdentity,
        retired: &crate::native_retirement::RetiredDisplay<P>,
    ) -> Result<(), ()> {
        if !self.display_issued
            || self.issued_display_play.as_deref() != Some(active)
            || !retired.matches_active(active)
            || retired.provider_generation() != self.display_generation
        {
            return Err(());
        }
        self.display_issued = false;
        self.issued_display_play = None;
        if self.reserved_display_play.as_deref() == Some(active) {
            self.reserved_display_play = None;
        }
        Ok(())
    }
}
