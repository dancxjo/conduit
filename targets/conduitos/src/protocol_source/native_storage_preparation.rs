//! Root-initialized storage resources; class wire meaning remains ordinary Source.
use super::*;
use alloc::vec::Vec;
use crate::storage_base::{
    transaction_contract::BlockReadTransactionContract, transaction_factory::*,
};

pub(crate) struct NativeStoragePreparation {
    pub host: HostId,
    pub boot: BootId,
    pub provider: BaseInstanceId,
    pub pool: ResourcePoolId,
    pub artifact: ArtifactId,
    pub maximum_retained_bytes: u64,
    pub authorize: NativeProtocolIssuer,
    pub settle: NativeProtocolIssuer,
    pub additional: Option<Box<NativeStoragePreparation>>,
    /// Exact authorize/settle Gear IDs chosen by the trusted Root, never Source.
    pub selected_gears: Option<(GearId, GearId)>,
}
impl NativeStoragePreparation {
    pub(super) fn select_placements(
        &self,
        gears: &[conduit_plot::CheckedGear],
        host: &HostAdvertisement,
        placements: &mut conduit_planner::PlacementChoices,
    ) -> Result<(), crate::protocol_host_calls::ProtocolCallRefusal> {
        use crate::protocol_host_calls::ProtocolCallRefusal as E;
        let Some(additional) = &self.additional else {
            return if self.selected_gears.is_none() {
                Ok(())
            } else {
                Err(E::InvalidPlan)
            };
        };
        if additional.additional.is_some() {
            return Err(E::InvalidPlan);
        }
        let mut choices = Vec::with_capacity(4);
        for pair in [self, additional.as_ref()] {
            let (authorize, settle) = pair.selected_gears.as_ref().ok_or(E::InvalidPlan)?;
            if host.host_id != pair.host || host.boot_id != pair.boot {
                return Err(E::InvalidPlan);
            }
            for (id, issuer, kind) in [
                (
                    authorize,
                    &pair.authorize,
                    crate::storage_base::transaction_contract::AUTHORIZE_KIND,
                ),
                (
                    settle,
                    &pair.settle,
                    crate::storage_base::transaction_contract::SETTLE_KIND,
                ),
            ] {
                if choices.iter().any(|(previous, _)| previous == id) {
                    return Err(E::InvalidPlan);
                }
                let mut matching = gears.iter().filter(|gear| &gear.gear_id == id);
                let gear = matching.next().ok_or(E::InvalidPlan)?;
                if matching.next().is_some() || gear.kind_id.as_str() != kind {
                    return Err(E::InvalidPlan);
                }
                let grant = issuer.grant();
                let call = if kind == crate::storage_base::transaction_contract::AUTHORIZE_KIND {
                    crate::storage_base::transaction_contract::AUTHORIZE_CALL
                } else {
                    crate::storage_base::transaction_contract::SETTLE_CALL
                };
                if grant.contract_id.as_str() != crate::storage_base::contract::BLOCK_READ_AUTHORITY
                    || grant.host_call_contract_id.as_str() != call
                    || grant.host_id != host.host_id
                    || grant.boot_id != host.boot_id
                    || grant.subject_kind.as_str() != kind
                {
                    return Err(E::InvalidPlan);
                }
                let mut offers = host
                    .capabilities
                    .iter()
                    .filter(|offer| offer.capability_id == grant.capability_id);
                let offer = offers.next().ok_or(E::InvalidPlan)?;
                if offers.next().is_some()
                    || offer.kind_id.as_str() != kind
                    || !gear.accepts_realization(offer)
                    || !placements.by_gear.contains_key(id)
                {
                    return Err(E::InvalidPlan);
                }
                choices.push((
                    id.clone(),
                    conduit_planner::PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: grant.capability_id.clone(),
                    },
                ));
            }
        }
        for (id, choice) in choices {
            placements.by_gear.insert(id, choice);
        }
        Ok(())
    }

    pub(super) fn append(
        &self,
        advertisement: &mut HostAdvertisement,
    ) -> Result<(), crate::protocol_host_calls::ProtocolCallRefusal> {
        use crate::protocol_host_calls::ProtocolCallRefusal as E;
        if advertisement.host_id != self.host
            || advertisement.boot_id != self.boot
            || advertisement
                .bases
                .iter()
                .any(|base| base.provider_instance_id == self.provider)
            || advertisement
                .resources
                .iter()
                .any(|resource| resource.pool_id == self.pool)
            || self
                .additional
                .as_ref()
                .is_some_and(|next| next.additional.is_some())
            || advertisement.capabilities.iter().any(|capability| {
                capability.capability_id == self.authorize.grant().capability_id
                    || capability.capability_id == self.settle.grant().capability_id
            })
        {
            return Err(E::InvalidPlan);
        }
        let contract = BlockReadTransactionContract::prepare().map_err(|_| E::InvalidPlan)?;
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 2,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: u16::try_from(advertisement.capabilities.len() + 2)
                .map_err(|_| E::InvalidPlan)?,
            maximum_advertised_resources: u16::try_from(advertisement.resources.len() + 1)
                .map_err(|_| E::InvalidPlan)?,
        })
        .map_err(|_| E::InvalidPlan)?;
        registry
            .register(BaseProviderEntry {
                base_id: self.provider.as_str().into(),
                provider_instance_id: self.provider.clone(),
                provider_generation: 1,
                implementation_id: TRANSACTION_BASE.into(),
                mechanism_family: TRANSACTION_RESOURCE.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![
                    {
                        let mut offer = offer(
                            StorageTransactionRole::Authorize,
                            &contract,
                            self.artifact.clone(),
                        );
                        offer.capability_id = self.authorize.grant().capability_id.clone();
                        offer
                    },
                    {
                        let mut offer = offer(
                            StorageTransactionRole::Settle,
                            &contract,
                            self.artifact.clone(),
                        );
                        offer.capability_id = self.settle.grant().capability_id.clone();
                        offer
                    },
                ],
                // Two finite roles share the independently backed association envelope.
                resources: vec![resource_offer(self.pool.as_str(), TRANSACTION_RESOURCE, 2)],
            })
            .map_err(|_| E::InvalidPlan)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| E::InvalidPlan)?;
        if let Some(additional) = &self.additional {
            additional.append(advertisement)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "native_storage_preparation_tests.rs"]
mod tests;
