use conduit_host_fabrication::BuildManifest;

use super::{refusal, ConduitosError};

const IMPLEMENTATION: &str = "conduitos/kernel-http-client-http1-literal@1";
const HOST_OPERATION: &str = "conduit.host/http-client-exchange@1";
const CLIENT_RESOURCE: &str = "conduit.resource/network/http-client@1";
const PACKET_RESOURCE: &str = "network/packet-buffer@1";
const SOCKET_RESOURCE: &str = "network/tcp-socket@1";
const TIMER_RESOURCE: &str = "network/timer@1";
const FACILITY: &str = "network/http1-literal-client@1";
const BASE: &str = "network/ipv4-tcp";
const DRIVER: &str = "conduitos/deterministic-ipv4-tcp@1";

pub(super) struct HttpInputs {
    pub selected: bool,
    pub implementation: u16,
    pub facility: u16,
    pub resource: u16,
    pub base: u16,
    pub driver: u16,
}

pub(super) fn lower(manifest: &BuildManifest) -> Result<HttpInputs, ConduitosError> {
    let selected = manifest
        .implementations
        .iter()
        .any(|item| item == IMPLEMENTATION);
    let present = [
        (
            HOST_OPERATION,
            manifest
                .host_operations
                .iter()
                .any(|item| item == HOST_OPERATION),
        ),
        (
            CLIENT_RESOURCE,
            resource(manifest, CLIENT_RESOURCE, 1, 107_000),
        ),
        (
            PACKET_RESOURCE,
            resource(manifest, PACKET_RESOURCE, 4, 6_144),
        ),
        (
            SOCKET_RESOURCE,
            resource(manifest, SOCKET_RESOURCE, 1, 32_768),
        ),
        (TIMER_RESOURCE, resource(manifest, TIMER_RESOURCE, 2, 64)),
        (
            FACILITY,
            manifest.facilities.iter().any(|item| item == FACILITY),
        ),
        (
            BASE,
            manifest
                .base_selections
                .iter()
                .any(|item| item.kind == BASE && item.driver == DRIVER),
        ),
        (
            DRIVER,
            manifest
                .driver_selections
                .iter()
                .any(|item| item.kind == DRIVER),
        ),
    ];
    if selected {
        if manifest.target != "conduitos/x86_64/pc" {
            return Err(refusal(
                "http-profile-target-unsupported",
                manifest.target.clone(),
            ));
        }
        if let Some((missing, _)) = present.iter().find(|(_, found)| !found) {
            return Err(refusal(
                "http-profile-prerequisite-missing",
                format!("implementation:{IMPLEMENTATION} > {missing}"),
            ));
        }
    } else if let Some((leaked, _)) = present.iter().find(|(_, found)| *found) {
        return Err(refusal(
            "http-machinery-leaked",
            format!("PROFILE without {IMPLEMENTATION} selected {leaked}"),
        ));
    }
    Ok(HttpInputs {
        selected,
        implementation: if selected {
            conduitos::fabrication::IMPL_HTTP_CLIENT
        } else {
            0
        },
        facility: if selected {
            conduitos::fabrication::FACILITY_HTTP_CLIENT
        } else {
            0
        },
        resource: if selected {
            conduitos::fabrication::RESOURCE_HTTP_CLIENT
        } else {
            0
        },
        base: if selected {
            conduitos::fabrication::BASE_HTTP_NETWORK
        } else {
            0
        },
        driver: if selected {
            conduitos::fabrication::DRIVER_HTTP_NETWORK
        } else {
            0
        },
    })
}

fn resource(manifest: &BuildManifest, class: &str, slots: u32, bytes: u64) -> bool {
    manifest
        .resource_budgets
        .iter()
        .any(|item| item.class == class && item.slots == slots && item.bytes == bytes)
}
