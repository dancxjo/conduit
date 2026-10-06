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
        "usb-hid-keyboard-endpoint" | "usb-hid-mouse-endpoint" | "usb-hid-keyboard-capture-window"
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
    if entry == "usb-hid-keyboard-capture-window" {
        return capture_plan(subject);
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

// This is an explicit proof Root recipe. The native caller must additionally
// establish readiness from the real Configure Endpoint receipt and retained
// eight-buffer DMA, then issue distinct possession for every selected call.
fn capture_plan(
    subject: &EndpointReadProofSubject<'_>,
) -> Result<PreparedProtocolArtifact, alloc::string::String> {
    let contract = EndpointReadContract::prepare().map_err(|_| "usb-hid-capture-proof-contract")?;
    let (mut host, mut grants) = host_and_grants(&contract, subject)?;
    host.capabilities[0] = super::endpoint_read_offer::capture_offer(
        &contract,
        "conduitos/usb-endpoint-read-native@1".into(),
        alloc::format!(
            "usb-endpoint-capture/{}/{}",
            subject.controller_base_id,
            subject.endpoint_epoch
        )
        .into(),
        8,
    )?;
    host.bases[0].capability_ids = alloc::vec![host.capabilities[0].capability_id.clone()];
    host.resources[0].capacity_units = 8;
    grants[0].capability_id = host.capabilities[0].capability_id.clone();
    super::hid_source_plan::prepare(
        super::hid_source_plan::HidSourceRole::KeyboardCapture,
        &host,
        &grants,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_proof_recipe_prepares_eight_selected_calls_and_fences_attachment_epoch() {
        let mut subject = EndpointReadProofSubject {
            host_id: "fixture/host",
            boot_id: "fixture/boot",
            controller_base_id: "fixture/controller",
            device_instance_id: "fixture/device",
            root_port: 1,
            slot: 1,
            attachment_epoch: 1,
            endpoint_dci: 3,
            endpoint_epoch: 1,
        };
        let artifact = plan(&subject, "usb-hid-keyboard-capture-window").unwrap();
        let definition = artifact.artifact().definition();
        let before = definition.internal_plan.plan_id.clone();
        let calls: alloc::vec::Vec<_> = definition.internal_plan.fragments[0]
            .placements
            .iter()
            .filter(|gear| {
                gear.implementation_id.as_str()
                    == super::super::endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION
            })
            .collect();
        assert_eq!(calls.len(), 8);
        assert_eq!(definition.external_capability.inputs.len(), 9);
        assert_eq!(definition.external_capability.outputs.len(), 3);
        for call in calls {
            assert_eq!(call.host_calls.len(), 1);
            assert_eq!(call.resources.len(), 1);
            assert_eq!(call.authority.len(), 1);
        }
        crate::usb_base::hid_source_kernel::PreparedHidSourceKernel::prepare(
            artifact,
            super::super::endpoint_read_proof_plan::ENDPOINT_READ_PROOF_SIGN_STORAGE,
        )
        .unwrap();
        subject.endpoint_epoch += 1;
        let next = plan(&subject, "usb-hid-keyboard-capture-window").unwrap();
        assert_ne!(next.artifact().definition().internal_plan.plan_id, before);
        subject.attachment_epoch = 0;
        assert!(plan(&subject, "usb-hid-keyboard-capture-window").is_err());
    }
}
