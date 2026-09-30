//! Runtime possession for the discovered surface carried by the native Mask.

use alloc::format;
use conduit_core::{
    ActivePlayId, BaseCapabilityAuthority, BaseCapabilityScope, BaseCapabilityTable,
    BaseOperationClaim, BaseOperationLease, CapabilityEnvelopeId, CapabilityIssueRequest,
    CapabilityLifecycle, HostCallContractId, OfferedResourceCord, Plan, ResourceGenerationId,
    ResourcePortLifecycle,
};
use serde::Serialize;

use crate::product_bases::NativeSurfaceProvider;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NativeSurfacePossessionReceipt {
    pub connection_id: conduit_core::ConnectionId,
    pub resource_pool_id: conduit_core::ResourcePoolId,
    pub provider_instance_id: conduit_core::BaseInstanceId,
    pub provider_generation: u64,
    pub lifecycle: &'static str,
}

pub struct ActiveNativeSurfacePossession {
    table: BaseCapabilityTable,
    cord: conduit_core::AcceptedResourceCord,
    claim: BaseOperationClaim,
    lease: Option<BaseOperationLease>,
}

impl ActiveNativeSurfacePossession {
    pub fn begin(
        provider: &NativeSurfaceProvider,
        plan: &Plan,
        active_play_id: ActivePlayId,
    ) -> Result<Self, ()> {
        let connection = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .find(|connection| connection.resource.is_some())
            .ok_or(())?;
        let resource = connection.resource.as_ref().ok_or(())?;
        let source = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .find(|placement| placement.placement_id == connection.source_placement_id)
            .ok_or(())?;
        let sink = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .find(|placement| placement.placement_id == connection.sink_placement_id)
            .ok_or(())?;
        let base = source.base.as_ref().ok_or(())?;
        let authority = sink.authority.first().ok_or(())?;
        if base.provider_instance_id != provider.entry.provider_instance_id
            || base.provider_generation != provider.entry.provider_generation
            || resource.source_binding.pool_id
                != provider.entry.resources.first().ok_or(())?.pool_id
        {
            return Err(());
        }
        let generation = ResourceGenerationId(format!(
            "{}/generation/{}",
            base.provider_instance_id.as_str(),
            base.provider_generation
        ));
        let envelope = CapabilityEnvelopeId::from("conduitos/native-mask/present@1");
        let scope = BaseCapabilityScope {
            host_id: sink.host_id.clone(),
            boot_id: sink.boot_id.clone(),
            base_instance_id: base.provider_instance_id.clone(),
            base_provider_generation: base.provider_generation,
            plan_id: plan.plan_id.clone(),
            active_play_id: active_play_id.clone(),
            authority_grant_id: authority.grant_id.clone(),
            authority_contract_id: authority.contract_id.clone(),
            capability_id: authority.capability_id.clone(),
            implementation_id: sink.implementation_id.clone(),
            operation_contract_id: authority.host_call_contract_id.clone(),
            subject_kind: authority.subject_kind.clone(),
            resource_pool_id: resource.source_binding.pool_id.clone(),
            resource_generation_id: generation.clone(),
            envelope_id: envelope.clone(),
            maximum_parameter_bytes: 1,
            maximum_result_bytes: 1,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 1,
        };
        let request = CapabilityIssueRequest {
            authority: BaseCapabilityAuthority {
                grant: conduit_core::AuthorityGrant {
                    grant_id: authority.grant_id.clone(),
                    contract_id: authority.contract_id.clone(),
                    host_call_contract_id: authority.host_call_contract_id.clone(),
                    subject_kind: authority.subject_kind.clone(),
                    host_id: authority.host_id.clone(),
                    boot_id: authority.boot_id.clone(),
                    capability_id: authority.capability_id.clone(),
                },
                base_instance_id: base.provider_instance_id.clone(),
                base_provider_generation: base.provider_generation,
                resource_pool_id: resource.source_binding.pool_id.clone(),
                resource_generation_id: generation.clone(),
                operation_contract_id: HostCallContractId::from("conduit.host/present@1"),
                envelope_id: envelope.clone(),
                maximum_parameter_bytes: 1,
                maximum_result_bytes: 1,
                maximum_work_units: 1,
                maximum_in_flight: 1,
                maximum_operations: 1,
            },
            scope: scope.clone(),
        };
        let mut table = BaseCapabilityTable::new(
            sink.host_id.clone(),
            sink.boot_id.clone(),
            base.provider_instance_id.clone(),
            base.provider_generation,
            provider.issuer_key,
            1,
        )
        .map_err(|_| ())?;
        let handle = table.issue(request).map_err(|_| ())?;
        let offered =
            OfferedResourceCord::new(plan.plan_id.clone(), active_play_id, connection, handle)
                .map_err(|_| ())?;
        let cord = offered
            .accept(&connection.sink_placement_id, &connection.sink_port_id)
            .map_err(|_| ())?;
        let claim = BaseOperationClaim {
            host_id: scope.host_id,
            boot_id: scope.boot_id,
            base_instance_id: scope.base_instance_id,
            base_provider_generation: scope.base_provider_generation,
            plan_id: scope.plan_id,
            active_play_id: scope.active_play_id,
            implementation_id: scope.implementation_id,
            operation_contract_id: scope.operation_contract_id,
            subject_kind: scope.subject_kind,
            resource_pool_id: scope.resource_pool_id,
            resource_generation_id: scope.resource_generation_id,
            envelope_id: scope.envelope_id,
            parameter_bytes: 1,
            work_units: 1,
        };
        Ok(Self {
            table,
            cord,
            claim,
            lease: None,
        })
    }

    pub fn authorize(&mut self) -> Result<(), ()> {
        self.lease = Some(
            self.cord
                .authorize(&mut self.table, &self.claim)
                .map_err(|_| ())?,
        );
        Ok(())
    }

    pub fn complete_and_retire(mut self) -> Result<NativeSurfacePossessionReceipt, ()> {
        self.cord
            .complete(&mut self.table, self.lease.take().ok_or(())?, 1)
            .map_err(|_| ())?;
        let inspection = self
            .cord
            .retire(&mut self.table, ResourcePortLifecycle::Play)
            .map_err(|_| ())?;
        let capability = self.table.inspections().next().ok_or(())?;
        if capability.lifecycle != CapabilityLifecycle::Revoked {
            return Err(());
        }
        Ok(NativeSurfacePossessionReceipt {
            connection_id: inspection.connection_id,
            resource_pool_id: inspection.binding.pool_id,
            provider_instance_id: capability.scope.base_instance_id.clone(),
            provider_generation: capability.scope.base_provider_generation,
            lifecycle: "revoked-at-play-end",
        })
    }
}
