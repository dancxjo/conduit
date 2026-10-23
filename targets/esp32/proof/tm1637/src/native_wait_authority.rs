//! Root timer policy. Expected cadence belongs to independently checked Source.
use super::*;
use conduit_monotonic_wait::{
    WaitNativeIdentity, PARAMETER_BYTES, WAIT_AUTHORITY, WAIT_CAPABILITY,
};
impl RootIssuer {
    /// Project permission only for this actual Host/Boot's retained timer.
    /// A grant is not locally issued possession and starts no operation.
    pub fn grant_wait(&self, native: &WaitNativeIdentity) -> Result<c::AuthorityGrant, ()> {
        if native.host_id != self.host || native.boot_id != self.boot {
            return Err(());
        }
        Ok(c::AuthorityGrant {
            grant_id: "grant/esp32/monotonic-wait/1".into(),
            contract_id: WAIT_AUTHORITY.into(),
            host_call_contract_id: c::WAIT_HOST_CALL_CONTRACT.into(),
            subject_kind: conduit_time::TICK_KIND.into(),
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            capability_id: WAIT_CAPABILITY.into(),
        })
    }
    /// Root must supply the finite cadence from its independently checked Source,
    /// not the received Plan. Complete memory/work and Body start remain separate.
    /// Zero-count Source needs no timer possession and is refused here.
    pub fn issue_wait(
        &mut self,
        native: &WaitNativeIdentity,
        fragment: &c::PlanFragment,
        active: &c::ActivePlayIdentity,
        cadence: &conduit_time::TickConfiguration,
    ) -> Result<
        (
            c::BaseCapabilityTable,
            c::BaseCapabilityHandle,
            c::BaseOperationClaim,
        ),
        (),
    > {
        if cadence.count == 0 || cadence.count > conduit_time::MAX_TICK_COUNT {
            return Err(());
        }
        let mut ticks = fragment
            .placements
            .iter()
            .filter(|gear| gear.kind_id.as_str() == conduit_time::TICK_KIND);
        let tick = ticks.next().ok_or(())?;
        if ticks.next().is_some()
            || tick.configuration.len() != 2
            || conduit_time::parse_tick_configuration(&tick.configuration).map_err(|_| ())?
                != *cadence
            || tick.implementation_id != native.implementation_id
            || tick.base.as_ref().is_none_or(|base| {
                base.provider_instance_id != native.provider_instance_id
                    || base.provider_generation != native.provider_generation
            })
        {
            return Err(());
        }
        if fragment.host_id != self.host
            || fragment.boot_id != self.boot
            || !c::verify_plan_fragment(fragment)
        {
            return Err(());
        }
        let possession =
            self.prepare_wait_possession(native, &fragment.plan_id, active, cadence)?;
        self.wait_issued = true;
        Ok(possession)
    }
    /// Bounded assignment issuance checks the actual retained timer before
    /// returning possession. Failure drops preparation-only authority and does
    /// not consume Root's one issuance budget or transfer the provider.
    pub fn issue_assigned_wait<P: conduit_monotonic_wait::MonotonicWaitProvider>(
        &mut self,
        owner: &conduit_monotonic_wait::ReadyMonotonicWaitBase<P>,
        assignment: &conduit_environment_runtime::appliance::assigned::ValidatedAssignedAppliance,
        selected: conduit_plan_lowering::selected_assigned_operation::SelectedAssignedOperationPlan<
            '_,
        >,
        cadence: &conduit_time::TickConfiguration,
    ) -> Result<
        (
            c::BaseCapabilityTable,
            c::BaseCapabilityHandle,
            c::BaseOperationClaim,
        ),
        (),
    > {
        let identity = assignment.identity();
        let id = c::AssignedIdentity::from_text;
        if assignment.cadence() != *cadence
            || identity.host != id(self.host.as_str())
            || identity.boot != id(self.boot.as_str())
            || identity.plan != id(selected.active.plan_id.as_str())
            || identity.fragment != id(selected.fragment_id.as_str())
        {
            return Err(());
        }
        let possession = self.prepare_wait_possession(
            owner.native_identity(),
            &selected.active.plan_id,
            selected.active,
            cadence,
        )?;
        let configuration = [
            c::ConfigurationEntry {
                key: "count".into(),
                value: c::ConfigurationValue::U64(cadence.count),
            },
            c::ConfigurationEntry {
                key: "period-ms".into(),
                value: c::ConfigurationValue::U64(cadence.period_ms),
            },
        ];
        owner
            .check_assigned(&possession.0, &possession.2, &configuration, selected)
            .map_err(|_| ())?;
        self.wait_issued = true;
        Ok(possession)
    }
    pub(super) fn prepare_wait_possession(
        &self,
        native: &WaitNativeIdentity,
        plan: &c::PlanId,
        active: &c::ActivePlayIdentity,
        cadence: &conduit_time::TickConfiguration,
    ) -> Result<
        (
            c::BaseCapabilityTable,
            c::BaseCapabilityHandle,
            c::BaseOperationClaim,
        ),
        (),
    > {
        if cadence.count == 0 || cadence.count > conduit_time::MAX_TICK_COUNT {
            return Err(());
        }
        let operations = u32::try_from(cadence.count).map_err(|_| ())?;
        let grant = self.grant_wait(native)?;
        let call = c::WAIT_HOST_CALL_CONTRACT;
        let subject = conduit_time::TICK_KIND;
        let cap = WAIT_CAPABILITY;
        let input = PARAMETER_BYTES;
        let output = 0;
        if self.wait_issued
            || self.reserved_play.as_ref() != Some(active)
            || *active != c::bind_active_play(plan, &self.host, &self.boot, active.play_sequence)
        {
            return Err(());
        }
        let mut hash = Sha256::new();
        hash.update(self.key);
        hash.update(native.provider_instance_id.as_str().as_bytes());
        let key: [u8; 32] = hash.finalize().into();
        let mut table = c::BaseCapabilityTable::new(
            self.host.clone(),
            self.boot.clone(),
            native.provider_instance_id.clone(),
            native.provider_generation,
            key,
            1,
        )
        .map_err(|_| ())?;
        let scope = c::BaseCapabilityScope {
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            base_instance_id: native.provider_instance_id.clone(),
            base_provider_generation: native.provider_generation,
            plan_id: plan.clone(),
            active_play_id: active.active_play_id.clone(),
            authority_grant_id: grant.grant_id.clone(),
            authority_contract_id: grant.contract_id.clone(),
            capability_id: cap.into(),
            implementation_id: native.implementation_id.clone(),
            operation_contract_id: call.into(),
            subject_kind: subject.into(),
            resource_pool_id: native.resource_pool_id.clone(),
            resource_generation_id: native.resource_generation_id.clone(),
            envelope_id: native.envelope_id.clone(),
            maximum_parameter_bytes: input,
            maximum_result_bytes: output,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: operations,
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
            parameter_bytes: input,
            work_units: 1,
        };
        let authority = c::BaseCapabilityAuthority {
            grant,
            base_instance_id: native.provider_instance_id.clone(),
            base_provider_generation: native.provider_generation,
            resource_pool_id: native.resource_pool_id.clone(),
            resource_generation_id: native.resource_generation_id.clone(),
            operation_contract_id: call.into(),
            envelope_id: native.envelope_id.clone(),
            maximum_parameter_bytes: input,
            maximum_result_bytes: output,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: operations,
        };
        let handle = table
            .issue(c::CapabilityIssueRequest { scope, authority })
            .map_err(|_| ())?;
        Ok((table, handle, claim))
    }
}
