//! Bind two genuine storage scopes alongside one retained original USB Host.
use super::*;
use crate::storage_base::{transaction_contract::*, transaction_factory::*, transaction_owner::*};

impl<P: I2cProvider> PreparedProtocolCalls<P> {
    pub(crate) fn prepare_storage_usb_duplex(
        plan: &Plan,
        admission: crate::protocol_play::StorageUsbAdmission,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        let mut calls = Self::prepare_usb_duplex_with_bindings(
            plan,
            admission.usb,
            zip,
            states,
            &[AUTHORIZE_IMPLEMENTATION, SETTLE_IMPLEMENTATION],
            |_, _, _, _| Ok(()),
        )?;
        let fragment = &plan.fragments[0];
        let lowered =
            lower_plan_fragment(fragment).map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let gear = |implementation: &str| {
            fragment
                .placements
                .iter()
                .find(|gear| gear.implementation_id.as_str() == implementation)
                .ok_or(ProtocolCallRefusal::InvalidPlan)
        };
        let authorize = gear(AUTHORIZE_IMPLEMENTATION)?;
        let settle = gear(SETTLE_IMPLEMENTATION)?;
        let selected =
            |placement_id| crate::machine_membrane::selected_operation::SelectedOperationPlan {
                fragment,
                lowered: &lowered,
                active: &active,
                placement_id,
            };
        let scopes = |possession: crate::protocol_play::StorageScopePossession, placement_id| {
            StorageScopeAdmission {
                table: possession.table,
                handle: possession.handle,
                claim: possession.claim,
                selected: selected(placement_id),
            }
        };
        let owner = BlockReadTransactionOwner::bind(
            &BlockReadTransactionContract::prepare()
                .map_err(|_| ProtocolCallRefusal::InvalidPlan)?,
            scopes(admission.authorize, &authorize.placement_id),
            scopes(admission.settle, &settle.placement_id),
        )
        .map_err(ProtocolCallRefusal::Storage)?;
        let nodes = calls
            .usb_duplex
            .as_ref()
            .ok_or(ProtocolCallRefusal::InvalidPlan)?
            .subordinate_nodes();
        calls.storage = Some(storage::StorageAssociation::prepare(
            owner,
            plan,
            nodes,
            admission.maximum_retained_bytes,
        )?);
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
        Ok(calls)
    }
}
