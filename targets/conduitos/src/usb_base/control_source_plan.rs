//! Ordinary checked control Source admission against retained native metadata.
//! Planning and descriptive grants do not issue possession or perform effects.
use super::{
    control_contract::*,
    installation::{UsbInstallationRefusal, UsbNativeIdentity},
};
use crate::protocol_source::{
    PreparedProtocolArtifact, PreparedProtocolEntry, ProtocolSourcePackage, ProtocolSourceRefusal,
};
use alloc::collections::BTreeMap;
use conduit_core::*;
use conduit_planner::PlanningOptions;
#[derive(Debug)]
pub enum ControlSourcePrepareRefusal {
    WrongGrant,
    Installation(UsbInstallationRefusal),
    Source(ProtocolSourceRefusal),
    Encoding(serde_json::Error),
    Contract(StructuredInfoRefusal),
}
pub fn prepare(
    package: ProtocolSourcePackage,
    entry: &str,
    identity: &UsbNativeIdentity,
    grant: &AuthorityGrant,
) -> Result<PreparedProtocolArtifact, ControlSourcePrepareRefusal> {
    use ControlSourcePrepareRefusal as E;
    if grant.host_id != identity.host_id
        || grant.boot_id != identity.boot_id
        || grant.contract_id.as_str() != super::control_factory::CONTROL_AUTHORITY
        || grant.host_call_contract_id.as_str() != CONTROL_CALL
        || grant.subject_kind.as_str() != CONTROL_KIND
        || grant.capability_id.as_str() != super::control_factory::CONTROL_IMPLEMENTATION
    {
        return Err(E::WrongGrant);
    }
    let bytes = serde_json::to_vec(&package).map_err(E::Encoding)?;
    let entry = PreparedProtocolEntry::prepare(&bytes, entry).map_err(E::Source)?;
    let contract = ControlContract::prepare().map_err(E::Contract)?;
    let mut host = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/native-usb-control@1".into(),
        bases: alloc::vec![],
        resources: alloc::vec![],
        capabilities: alloc::vec![],
        planner_capabilities: alloc::vec![],
    };
    identity
        .append_to_advertisement(&mut host, &contract)
        .map_err(E::Installation)?;
    entry.publish_pure_backs(&mut host).map_err(E::Source)?;
    let hosts = [host];
    let placements = entry.placements(&hosts).map_err(E::Source)?;
    entry
        .plan(
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: CONTROL_MAXIMUM_BYTES,
                authority_grants: core::slice::from_ref(grant),
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(E::Source)
}

#[cfg(test)]
mod tests;
