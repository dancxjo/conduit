//! Body State preparation uses the issued partition Play, never a prior or
//! prospective identity. Residence effects begin only after accepted start.
use super::*;
use crate::body_execution::BodyDurableStateSelection;
use crate::state_value::{
    DurableStateBinding, InstalledDurableStateBack, InstalledDurableStateHost,
};
use std::path::PathBuf;

pub(super) fn is_operation(contract: &str) -> bool {
    matches!(
        contract,
        conduit_std_offers::STATE_VALUE_DURABLE_RECOVER_HOST_CALL
            | conduit_std_offers::STATE_VALUE_DURABLE_COMMIT_HOST_CALL
    )
}
pub(super) struct PreparedBodyState {
    pub(super) node: conduit_kernel::NodeId,
    root: PathBuf,
    binding: DurableStateBinding,
    host: Option<InstalledDurableStateHost>,
}
impl PreparedBodyState {
    pub(super) fn prepare(
        fragment: &PlanFragment,
        _partition: &conduit_plan_lowering::lowering::LoweredPlanFragment,
        state: &conduit_plan_lowering::lowering::LoweredState,
        selection: &BodyDurableStateSelection<'_>,
        play: &conduit_body::BodyPlayIdentity,
        values: &mut HostedValueStore,
    ) -> Result<(InstalledDurableStateBack, Self), String> {
        if selection.body_id != &play.body_id || selection.plan_id != &play.plan_id {
            return Err("Body State belongs to another issued Body Plan".into());
        }
        let placement = fragment
            .placements
            .iter()
            .find(|placement| placement.gear_id == state.contract.gear_id)
            .ok_or("Body State has no exact placement")?;
        let [resource] = placement.resources.as_slice() else {
            return Err("Body State needs one exact resource".into());
        };
        if resource.pool_id != selection.resource.pool_id
            || resource.class_id != selection.resource.class_id
            || resource.units == 0
            || resource.units > selection.resource.capacity_units
            || resource.content != selection.resource.content
            || !same_authorities(&placement.authority, selection.authorities)
        {
            return Err(
                "Body State resource or authority differs from independently selected custody"
                    .into(),
            );
        }
        let active = conduit_core::bind_active_play(
            &fragment.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            play.play_sequence,
        );
        let mut local_state = state.clone();
        local_state.slot = u16::try_from(
            fragment
                .states
                .iter()
                .position(|candidate| candidate.state_id == state.contract.state_id)
                .ok_or("State boundary absent")?,
        )
        .map_err(|_| "State slot overflow")?;
        local_state.node = conduit_kernel::NodeId(
            u16::try_from(
                fragment
                    .placements
                    .iter()
                    .position(|candidate| candidate.placement_id == placement.placement_id)
                    .ok_or("State placement absent")?,
            )
            .map_err(|_| "State local node overflow")?,
        );
        let driver = crate::state_value::prepare_durable_state(
            fragment,
            &local_state,
            &active,
            play.body_id.as_str(),
            values,
        )?;
        let binding = DurableStateBinding {
            body: play.body_id.as_str().into(),
            state: state.contract.state_id.clone(),
            value_kind: state.contract.value_kind.clone(),
            maximum_value_bytes: state.contract.maximum_value_bytes,
        };
        Ok((
            driver,
            Self {
                node: state.node,
                root: selection.root.to_path_buf(),
                binding,
                host: None,
            },
        ))
    }
    pub(super) fn perform(
        &mut self,
        scheduler: &mut InstalledScheduler,
        request: HostCallRequest,
        operation: &LoweredHostCall,
    ) -> Result<(), String> {
        let input = scheduler
            .host_value(request.input.value)
            .map_err(|e| format!("Body State input: {e:?}"))?;
        let result = (|| {
            if self.host.is_none() {
                self.host = Some(InstalledDurableStateHost::open(
                    &self.root,
                    self.binding.clone(),
                )?);
            }
            let host = self
                .host
                .as_mut()
                .ok_or(crate::state_value::DurableStateRefusal::Lost)?;
            if operation.contract_id.as_str()
                == conduit_std_offers::STATE_VALUE_DURABLE_RECOVER_HOST_CALL
            {
                match input {
                    [1] => host.recover_metadata(),
                    metadata => host.recover_exact(metadata),
                }
            } else {
                host.commit(input).map(|receipt| receipt.to_vec())
            }
        })();
        let outcome = match result {
            Ok(bytes) => {
                let value = scheduler
                    .store_host_value(&bytes)
                    .map_err(|e| format!("Body State receipt: {e:?}"))?;
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, operation.binding.maximum_output_bytes)
                            .map_err(|e| format!("Body State receipt bound: {e:?}"))?,
                    ),
                    failure: None,
                }
            }
            Err(refusal) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(crate::state_value::durable_refusal_failure(refusal)),
            },
        };
        scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(|e| format!("complete Body State: {e:?}"))
    }
}
impl<'a> BodyKernel<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_durable_on_host(
        partitions: &[BodyPlotPlan],
        has_keyboard: bool,
        play: &conduit_body::BodyPlayIdentity,
        fore_inputs: &'a [ExternalForeInput],
        sequential_fore: bool,
        has_fore_output: bool,
        host: &conduit_core::HostAdvertisement,
        #[cfg(target_os = "linux")] selected_file: Option<
            &'a crate::text_file::SelectedTextFileResidence,
        >,
        selection: &BodyDurableStateSelection<'_>,
    ) -> Result<Self, String> {
        let sources = parsec_sources::prepare(partitions, host)?;
        let kernel = Self::prepare_with_storage(
            partitions,
            has_keyboard,
            &play.active_play_id,
            fore_inputs,
            sequential_fore,
            has_fore_output,
            None,
            sources.as_ref(),
            #[cfg(target_os = "linux")]
            selected_file,
            &[],
            Some((selection, play)),
        )?;
        if kernel.durable_states.len() != 1 {
            return Err("one Body State residence admits exactly one State".into());
        }
        Ok(kernel)
    }
}

fn same_authorities(
    bindings: &[conduit_core::AuthorityBinding],
    grants: &[conduit_core::AuthorityGrant],
) -> bool {
    bindings.len() == grants.len()
        && bindings.iter().zip(grants).all(|(binding, grant)| {
            binding.grant_id == grant.grant_id
                && binding.contract_id == grant.contract_id
                && binding.host_call_contract_id == grant.host_call_contract_id
                && binding.subject_kind == grant.subject_kind
                && binding.host_id == grant.host_id
                && binding.boot_id == grant.boot_id
                && binding.capability_id == grant.capability_id
        })
}
