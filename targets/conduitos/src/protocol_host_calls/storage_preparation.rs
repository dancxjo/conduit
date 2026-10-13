//! Bind bounded, independently issued storage pairs to one original USB Host.
use super::*;
use crate::storage_base::{transaction_contract::*, transaction_factory::*, transaction_owner::*};

/// A second genuine Root admission, never synthesized from a Source permit.
pub(crate) struct AdditionalStoragePairAdmission {
    pub maximum_retained_bytes: u64,
    pub authorize: crate::protocol_play::StorageScopePossession,
    pub settle: crate::protocol_play::StorageScopePossession,
}

pub(super) fn selected_count_allowed(implementation: &str, count: usize) -> bool {
    matches!(
        implementation,
        AUTHORIZE_IMPLEMENTATION | SETTLE_IMPLEMENTATION
    ) && (1..=2).contains(&count)
}

impl<P: I2cProvider> PreparedProtocolCalls<P> {
    pub(crate) fn prepare_storage_usb_duplex(
        plan: &Plan,
        admission: crate::protocol_play::StorageUsbAdmission,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_storage_pairs(plan, admission, None, zip, states)
    }

    pub(crate) fn prepare_storage_usb_duplex_two_pairs(
        plan: &Plan,
        admission: crate::protocol_play::StorageUsbAdmission,
        second: AdditionalStoragePairAdmission,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_storage_pairs(plan, admission, Some(second), zip, states)
    }

    fn prepare_storage_pairs(
        plan: &Plan,
        admission: crate::protocol_play::StorageUsbAdmission,
        second: Option<AdditionalStoragePairAdmission>,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        let [fragment] = plan.fragments.as_slice() else {
            return Err(ProtocolCallRefusal::InvalidPlan);
        };
        let expected = 1 + usize::from(second.is_some());
        for implementation in [AUTHORIZE_IMPLEMENTATION, SETTLE_IMPLEMENTATION] {
            let mut selected = fragment
                .placements
                .iter()
                .filter(|gear| gear.implementation_id.as_str() == implementation);
            let first = selected.next().ok_or(ProtocolCallRefusal::InvalidPlan)?;
            let second = selected.next();
            if selected.next().is_some()
                || 1 + usize::from(second.is_some()) != expected
                || second.is_some_and(|other| other.capability_id == first.capability_id)
            {
                return Err(ProtocolCallRefusal::InvalidPlan);
            }
        }
        let mut calls = Self::prepare_usb_duplex_with_bindings(
            plan,
            admission.usb,
            zip,
            states,
            &[AUTHORIZE_IMPLEMENTATION, SETTLE_IMPLEMENTATION],
            |_, _, _, _| Ok(()),
        )?;
        let lowered =
            lower_plan_fragment(fragment).map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let contract = BlockReadTransactionContract::prepare()
            .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        let nodes = calls
            .usb_duplex
            .as_ref()
            .ok_or(ProtocolCallRefusal::InvalidPlan)?
            .subordinate_nodes();
        let mut bind_pair = |admission: AdditionalStoragePairAdmission| {
            let exact =
                |implementation: &str,
                 possession: &crate::protocol_play::StorageScopePossession| {
                    // Inspection identifies a selected scope, not authority: the
                    // subsequent owner bind validates the retained table/handle and
                    // exact claim against that selected placement before any lease.
                    let mut inspections = possession.table.inspections();
                    let scope = &inspections
                        .next()
                        .ok_or(ProtocolCallRefusal::InvalidPlan)?
                        .scope;
                    if inspections.next().is_some() {
                        return Err(ProtocolCallRefusal::InvalidPlan);
                    }
                    let mut matches = fragment.placements.iter().filter(|gear| {
                        gear.implementation_id.as_str() == implementation
                            && gear.capability_id == scope.capability_id
                    });
                    let gear = matches.next().ok_or(ProtocolCallRefusal::InvalidPlan)?;
                    if matches.next().is_some() {
                        return Err(ProtocolCallRefusal::InvalidPlan);
                    }
                    Ok(gear)
                };
            let authorize = exact(AUTHORIZE_IMPLEMENTATION, &admission.authorize)?;
            let settle = exact(SETTLE_IMPLEMENTATION, &admission.settle)?;
            let scopes = |possession: crate::protocol_play::StorageScopePossession,
                          placement_id| StorageScopeAdmission {
                table: possession.table,
                handle: possession.handle,
                claim: possession.claim,
                selected: crate::machine_membrane::selected_operation::SelectedOperationPlan {
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement_id,
                },
            };
            let owner = BlockReadTransactionOwner::bind(
                &contract,
                scopes(admission.authorize, &authorize.placement_id),
                scopes(admission.settle, &settle.placement_id),
            )
            .map_err(ProtocolCallRefusal::Storage)?;
            let binding_start = crate::allocation::BOOT_ARENA.live_bytes();
            for (gear, owner) in [
                (authorize, Owner::StorageAuthorize),
                (settle, Owner::StorageSettle),
            ] {
                calls.bindings.push(Binding {
                    node: lowered
                        .identity
                        .node_for_placement(&gear.placement_id)
                        .ok_or(ProtocolCallRefusal::InvalidPlan)?,
                    resources: gear.resources.clone(),
                    authorities: gear.authority.clone(),
                    owner,
                });
            }
            let dispatcher_retained_bytes = crate::allocation::BOOT_ARENA
                .live_bytes()
                .checked_sub(binding_start)
                .ok_or(ProtocolCallRefusal::Storage(
                    StorageTransactionRefusal::Retention,
                ))?;
            storage::StorageAssociation::prepare(
                owner,
                plan,
                nodes,
                admission.maximum_retained_bytes,
                dispatcher_retained_bytes,
            )
        };
        let first = bind_pair(AdditionalStoragePairAdmission {
            maximum_retained_bytes: admission.maximum_retained_bytes,
            authorize: admission.authorize,
            settle: admission.settle,
        })?;
        let second = second.map(&mut bind_pair).transpose()?;
        calls.storage = Some(storage::StorageAssociations::prepare(first, second)?);
        Ok(calls)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn additional_storage_count_is_bounded_and_role_specific() {
        for role in [AUTHORIZE_IMPLEMENTATION, SETTLE_IMPLEMENTATION] {
            assert!(!selected_count_allowed(role, 0));
            assert!(selected_count_allowed(role, 1));
            assert!(selected_count_allowed(role, 2));
            assert!(!selected_count_allowed(role, 3));
        }
        assert!(!selected_count_allowed(
            crate::usb_base::control_factory::CONTROL_IMPLEMENTATION,
            2
        ));
        assert!(!selected_count_allowed(
            crate::usb_base::endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION,
            2
        ));
    }
}
