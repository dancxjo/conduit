//! Prepare Source-owned descriptor exchange against explicit proof-root grants.
//! This is a named cooperative proof appliance, not an installed product offer.
use super::{
    control_contract::ControlContract,
    control_proof_plan::{self, ControlProofSubject},
};
use crate::protocol_source::{
    PreparedProtocolArtifact, PreparedProtocolEntry, ProtocolSourcePackage, ProtocolSourceRefusal,
};
use alloc::collections::BTreeMap;
use conduit_core::BaseImplementationId;
use conduit_planner::PlanningOptions;

#[derive(Debug)]
pub enum DeviceProbeProofRefusal {
    Contract(conduit_core::StructuredInfoRefusal),
    Source(ProtocolSourceRefusal),
    Encoding(serde_json::Error),
    Root(&'static str),
}

pub fn prepare(
    subject: &ControlProofSubject<'_>,
) -> Result<PreparedProtocolArtifact, DeviceProbeProofRefusal> {
    let source = alloc::format!(
        "{}\n{}\n{}",
        include_str!("../../plots/usb/control-types.conduit"),
        include_str!("../../plots/usb/descriptors.conduit"),
        include_str!("../../plots/usb/device-probe.conduit")
    );
    let package =
        ProtocolSourcePackage::compile(source, &[]).map_err(DeviceProbeProofRefusal::Source)?;
    let bytes = serde_json::to_vec(&package).map_err(DeviceProbeProofRefusal::Encoding)?;
    let entry = PreparedProtocolEntry::prepare(&bytes, "usb-device-probe")
        .map_err(DeviceProbeProofRefusal::Source)?;
    let contract = ControlContract::prepare().map_err(DeviceProbeProofRefusal::Contract)?;
    let (mut host, grants) = control_proof_plan::host_and_grants(&contract, subject)
        .map_err(DeviceProbeProofRefusal::Root)?;
    entry
        .publish_pure_backs(&mut host)
        .map_err(DeviceProbeProofRefusal::Source)?;
    let hosts = [host];
    let placements = entry
        .placements(&hosts)
        .map_err(DeviceProbeProofRefusal::Source)?;
    entry
        .plan(
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 4096,
                authority_grants: &grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(DeviceProbeProofRefusal::Source)
}
