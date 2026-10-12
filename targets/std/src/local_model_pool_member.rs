//! host-owned admission for one selected dynamic local-model pool member.
//!
//! The common pool kernel owns selection. This boundary binds that selection
//! to the std host's current capability/resource ledger and owns the exact
//! finite semantic sessions until terminal release.

use crate::{
    hosted_local_model::LocalModelAdapterTerminal, kernel_preparation::KernelResourceReservation,
    pool_member_sessions::PoolMemberSessions, StdHost,
};
use conduit_core::{GearId, PlacementId, Plan, PlannedGear, PoolSelectionEvidence};

mod completion;
mod request_binding;
mod request_custody;
pub use completion::{
    LocalModelCompletedWork, LocalModelCompletionCalibrationRefusal, LocalModelMeasurementRefusal,
    LocalModelPreparedCalibration,
};
pub use request_custody::{NativePoolCustodyRefusal, NativePoolRequestCustody};
mod request_queue;
pub use request_queue::{
    NativePoolDispatch, NativePoolPreparedCosts, NativePoolQueueError, NativePoolRequestQueue,
};

pub struct AdmittedLocalModelPoolMember {
    sessions: PoolMemberSessions,
    reservation: Option<KernelResourceReservation>,
    placement: PlannedGear,
    operation_started: bool,
    measurement: completion::MeasurementSlot,
    request_binding: Option<request_binding::RequestBinding>,
}

impl AdmittedLocalModelPoolMember {
    /// Prepare typed measurement truth outside execution. This creates neither
    /// a forecast nor producer trust; failed terminals remain distinct.
    pub fn prepare_completed_work(
        &self,
    ) -> Result<LocalModelCompletedWork<'_>, LocalModelMeasurementRefusal> {
        self.measurement
            .prepare(&self.placement, self.sessions.selection())
    }

    pub fn sessions(&self) -> &PoolMemberSessions {
        &self.sessions
    }

    pub fn sessions_mut(&mut self) -> &mut PoolMemberSessions {
        &mut self.sessions
    }

    pub fn operation_started(&self) -> bool {
        self.operation_started
    }
}

impl StdHost {
    pub fn prepare_local_model_pool_member(
        &mut self,
        plan: &Plan,
        selection: PoolSelectionEvidence,
        consumer_placement_id: &PlacementId,
    ) -> Result<AdmittedLocalModelPoolMember, String> {
        self.prepare_local_model_pool_member_inner(plan, selection, consumer_placement_id, None)
    }

    /// Carry the owner's admitted checked demand into this exact operation.
    /// Selection and capacity remain owned by the existing pool kernel/ledger.
    /// A changed request is refused before the adapter and cannot be replayed.
    pub fn prepare_local_model_pool_member_with_demand(
        &mut self,
        plan: &Plan,
        selection: PoolSelectionEvidence,
        consumer_placement_id: &PlacementId,
        demand: conduit_planner::resource_cost::CostDemand<'_>,
    ) -> Result<AdmittedLocalModelPoolMember, String> {
        self.prepare_local_model_pool_member_inner(
            plan,
            selection,
            consumer_placement_id,
            Some(demand),
        )
    }

    fn prepare_local_model_pool_member_inner(
        &mut self,
        plan: &Plan,
        selection: PoolSelectionEvidence,
        consumer_placement_id: &PlacementId,
        demand: Option<conduit_planner::resource_cost::CostDemand<'_>>,
    ) -> Result<AdmittedLocalModelPoolMember, String> {
        selection
            .validate(plan)
            .map_err(|error| format!("validate pool member selection: {error:?}"))?;
        let pool = plan
            .fragments
            .first()
            .and_then(|fragment| {
                fragment
                    .shared_pools
                    .iter()
                    .find(|pool| pool.pool_id == selection.pool_id)
            })
            .ok_or_else(|| "selected shared pool is absent from the plan".to_string())?;
        let realization = pool
            .realization_envelope
            .get(usize::from(selection.selected_realization.ok_or_else(
                || "selected pool realization is absent".to_string(),
            )?))
            .ok_or_else(|| "selected pool realization is outside the plan".to_string())?;
        let advertisement = self.advertisement().clone();
        if realization.host_id != advertisement.host_id
            || realization.boot_id != advertisement.boot_id
            || realization.offer_generation != advertisement.offer_generation
        {
            return Err("selected pool realization is stale for this host".into());
        }
        let capability = advertisement
            .capabilities
            .iter()
            .find(|candidate| candidate.capability_id == realization.capability_id)
            .ok_or_else(|| "selected pool capability is no longer offered".to_string())?;
        if capability.implementation.implementation_id != realization.implementation_id
            || capability.implementation.artifact_id != realization.artifact_id
            || capability.implementation.implementation_id.as_str()
                != conduit_ai::LOCAL_MODEL_IMPLEMENTATION
            || capability.inputs != pool.member_front.inputs()
            || capability.outputs != pool.member_front.outputs()
            || capability.inputs.len() != 1
            || capability.outputs.len() != 1
            || !capability.authority_requirements.is_empty()
        {
            return Err(
                "selected pool realization differs from its exact local-model front".into(),
            );
        }
        let dynamic_identity = format!(
            "pool-member/{}/{}/{}",
            plan.plan_id.as_str(),
            selection.pool_id.as_str(),
            selection.operation_id.as_str()
        );
        let mut placement = conduit_core::planned_gear_from_parts! {
            semantic_contract: Default::default(),
            placement_id: PlacementId::from(dynamic_identity.clone()),
            gear_id: GearId::from(dynamic_identity),
            kind_id: capability.kind_id.clone(),
            kind_contract_revision: capability.kind_contract_revision.clone(),
            execution_profile_id: capability.implementation.execution_profile_id.clone(),
            configuration: Default::default(),
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            offer_generation: advertisement.offer_generation,
            capability_id: capability.capability_id.clone(),
            implementation_id: capability.implementation.implementation_id.clone(),
            artifact_id: capability.implementation.artifact_id.clone(),
            base: None,
            realization_characteristics: Vec::new(),
            limits: capability.limits.clone(),
            inputs: capability.inputs.clone(),
            outputs: capability.outputs.clone(),
            terminal_transductions: Vec::new(),
            host_calls: capability.host_calls.clone(),
            resources: realization.resources.clone(),
            authority: Vec::new(),
            pool_references: Vec::new(),
        };
        let request_binding = demand
            .map(|request| {
                let initialized = self
                    .local_model
                    .as_ref()
                    .ok_or_else(|| "selected local model is no longer initialized".to_string())?;
                request_binding::RequestBinding::admit(
                    request,
                    &pool.member_front,
                    capability,
                    &initialized.offer().limits.work,
                    &mut placement,
                )
            })
            .transpose()?;
        let measurement = completion::MeasurementSlot::admit(&placement);
        let reservation = self.kernel_resources.reserve_pool_member(
            &advertisement,
            plan.plan_id.clone(),
            &realization.capability_id,
            &realization.resources,
        )?;
        let sessions = match PoolMemberSessions::prepare(
            plan,
            selection,
            consumer_placement_id,
            &advertisement.host_id,
        ) {
            Ok(sessions) => sessions,
            Err(error) => {
                self.kernel_resources.release(reservation)?;
                return Err(error);
            }
        };
        Ok(AdmittedLocalModelPoolMember {
            sessions,
            reservation: Some(reservation),
            placement,
            operation_started: false,
            measurement,
            request_binding,
        })
    }

    /// Execute the one operation whose identity and capacity were sealed by
    /// this admitted member. Marking it started before entering the adapter
    /// prevents provider loss or malformed output from becoming a replay.
    pub fn execute_local_model_pool_member(
        &mut self,
        member: &mut AdmittedLocalModelPoolMember,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> LocalModelAdapterTerminal {
        let first = !member.operation_started;
        if first {
            member.measurement.start(input);
        }
        let terminal = if first
            && member
                .request_binding
                .as_ref()
                .is_some_and(|binding| !binding.matches(member.measurement.input_identity()))
        {
            member.operation_started = true;
            output.clear();
            LocalModelAdapterTerminal::Refused
        } else {
            execute_pool_member_once(
                self.local_model.as_deref_mut(),
                &member.placement,
                &mut member.operation_started,
                input,
                output,
            )
        };
        if first {
            member.measurement.finish(terminal, output.len());
        }
        terminal
    }

    pub fn release_local_model_pool_member(
        &mut self,
        mut member: AdmittedLocalModelPoolMember,
    ) -> Result<(), String> {
        let reservation = member
            .reservation
            .take()
            .ok_or_else(|| "pool member reservation was already released".to_string())?;
        self.kernel_resources.release(reservation)
    }
}

pub(crate) fn execute_pool_member_once(
    adapter: Option<&mut (dyn crate::hosted_local_model::HostedLocalModelAdapter + 'static)>,
    placement: &PlannedGear,
    operation_started: &mut bool,
    input: &[u8],
    output: &mut Vec<u8>,
) -> LocalModelAdapterTerminal {
    if *operation_started {
        output.clear();
        return LocalModelAdapterTerminal::Refused;
    }
    *operation_started = true;
    let Some(adapter) = adapter else {
        output.clear();
        return LocalModelAdapterTerminal::Refused;
    };
    adapter.execute(placement, input, output)
}
