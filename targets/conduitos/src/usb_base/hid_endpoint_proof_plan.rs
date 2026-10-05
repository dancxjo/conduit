//! Explicit proof-root selection of shared HID Source and an inbound endpoint.
//! Preparing this artifact performs no device effects and issues no possession.
use super::{
    endpoint_read_contract::EndpointReadContract,
    endpoint_read_proof_plan::{EndpointReadProofSubject, host_and_grants},
};
use crate::protocol_source::{
    PreparedProtocolArtifact, PreparedProtocolEntry, usb_hid_endpoint_package,
};
use alloc::collections::BTreeMap;
use conduit_core::BaseImplementationId;
use conduit_planner::PlanningOptions;

pub fn plan(
    subject: &EndpointReadProofSubject<'_>,
    entry: &str,
) -> Result<PreparedProtocolArtifact, alloc::string::String> {
    if !matches!(
        entry,
        "usb-hid-keyboard-endpoint" | "usb-hid-mouse-endpoint"
    ) {
        return Err("usb-hid-endpoint-proof-entry".into());
    }
    if subject.root_port == 0
        || subject.slot == 0
        || subject.attachment_epoch == 0
        || subject.endpoint_epoch == 0
        || !(3..=31).contains(&subject.endpoint_dci)
        || subject.endpoint_dci & 1 == 0
    {
        return Err("usb-hid-endpoint-proof-subject".into());
    }
    let package = usb_hid_endpoint_package().map_err(|_| "usb-hid-endpoint-proof-package")?;
    let bytes = serde_json::to_vec(&package).map_err(|_| "usb-hid-endpoint-proof-package")?;
    let entry = PreparedProtocolEntry::prepare(&bytes, entry)
        .map_err(|_| "usb-hid-endpoint-proof-source")?;
    let contract =
        EndpointReadContract::prepare().map_err(|_| "usb-hid-endpoint-proof-contract")?;
    let (mut host, grants) = host_and_grants(&contract, subject)?;
    entry
        .publish_pure_backs(&mut host)
        .map_err(|_| "usb-hid-endpoint-proof-backs")?;
    let hosts = [host];
    let placements = entry
        .placements(&hosts)
        .map_err(|_| "usb-hid-endpoint-proof-placement")?;
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
        .map_err(|error| alloc::format!("usb-hid-endpoint-proof-admission: {error:?}"))
}
