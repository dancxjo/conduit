//! Explicit proof-root selection of shared HID Source and an inbound endpoint.
//! Preparing this artifact performs no device effects and issues no possession.
use super::{
    endpoint_read_contract::EndpointReadContract,
    endpoint_read_proof_plan::{EndpointReadProofSubject, host_and_grants},
};
use crate::protocol_source::{
    PreparedProtocolArtifact, PreparedProtocolSource, usb_hid_endpoint_package,
};
use alloc::collections::BTreeMap;
use conduit_core::{ArtifactId, BaseImplementationId};
use conduit_planner::{PlanningOptions, default_expanded_placements};

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
    let source =
        PreparedProtocolSource::prepare(package).map_err(|_| "usb-hid-endpoint-proof-source")?;
    let expanded = source
        .expand(entry)
        .map_err(|_| "usb-hid-endpoint-proof-source")?;
    let contract =
        EndpointReadContract::prepare().map_err(|_| "usb-hid-endpoint-proof-contract")?;
    let (mut host, grants) = host_and_grants(&contract, subject)?;
    source
        .publish_pure_backs(&expanded, &mut host)
        .map_err(|_| "usb-hid-endpoint-proof-backs")?;
    let hosts = [host];
    let placements = default_expanded_placements(&expanded.expanded, &hosts)
        .map_err(|_| "usb-hid-endpoint-proof-placement")?;
    source
        .plan_artifact(
            &expanded,
            ArtifactId::from(alloc::format!("conduitos.proof/{entry}@1")),
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
