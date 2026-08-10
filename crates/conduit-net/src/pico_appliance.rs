//! Truthful finite Host offers for the optional Pico W Hello appliance.
//!
//! These are Host implementation facts, not authored Form vocabulary and not
//! Conduit-session Line semantics. A composition advertises this family only
//! after every exact artifact and resource has initialized.

use alloc::vec;
use alloc::vec::Vec;

use conduit_core::{
    kind_id, resource_offer, resource_requirement, ArtifactId, BootId, CapabilityId,
    CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostAdvertisement, HostId,
    HostOperationContractId, HostOperationRequirement, HostProfileId, ImplementationId,
    ImplementationOffer, KindContractRevision, OfferGeneration, ResourceOffer, PROTOCOL_VERSION,
};

pub const PICO_APPLIANCE_PROFILE: &str = "pico/appliance-hello@1";
pub const PICO_MINIMAL_PROFILE: &str = "pico/minimal@1";
pub const PICO_APPLIANCE_ARTIFACT: &str = "pico/appliance-hello-firmware@1";

pub const AP_RESOURCE_CLASS: &str = "conduit.resource/network/wifi-ap@1";
pub const DHCP_RESOURCE_CLASS: &str = "conduit.resource/network/dhcp-lease-pool@1";
pub const DNS_RESOURCE_CLASS: &str = "conduit.resource/network/dns-udp-base@1";
pub const HTTP_RESOURCE_CLASS: &str = "conduit.resource/network/http-tcp-base@1";

pub const AP_CAPABILITY: &str = "pico/appliance/ap-ready";
pub const DHCP_CAPABILITY: &str = "pico/appliance/dhcp";
pub const DNS_CAPABILITY: &str = "pico/appliance/dns";
pub const HTTP_CAPABILITY: &str = "pico/appliance/http-hello";

pub const MAXIMUM_AP_ASSOCIATIONS: u16 = 4;
pub const MAXIMUM_DHCP_LEASES: u16 = 4;
pub const MAXIMUM_DNS_PACKET_BYTES: u32 = 256;
pub const MAXIMUM_HTTP_REQUEST_BYTES: u32 = 512;
pub const MAXIMUM_HTTP_RESPONSE_BYTES: u32 = 128;
pub const MAXIMUM_APPLIANCE_SIGNS: u16 = 32;
pub const APPLIANCE_SSID: &str = "Conduit-Hello";
pub const APPLIANCE_LOCAL_NAME: &str = "hello.conduit";
pub const APPLIANCE_HELLO_BODY: &str = "Hello from Conduit\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PicoApplianceComposition {
    Minimal,
    Hello,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PicoApplianceInitialization {
    pub radio_artifact: bool,
    pub ap_base: bool,
    pub dhcp_base: bool,
    pub dns_base: bool,
    pub http_base: bool,
}

impl PicoApplianceInitialization {
    pub const fn hello_ready() -> Self {
        Self {
            radio_artifact: true,
            ap_base: true,
            dhcp_base: true,
            dns_base: true,
            http_base: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PicoApplianceAdvertisementError {
    MissingRadioArtifact,
    AccessPointBaseUnavailable,
    DhcpBaseUnavailable,
    DnsBaseUnavailable,
    HttpBaseUnavailable,
}

pub fn pico_appliance_advertisement(
    host_id: &str,
    boot_id: &str,
    composition: PicoApplianceComposition,
    initialized: PicoApplianceInitialization,
) -> Result<HostAdvertisement, PicoApplianceAdvertisementError> {
    if composition == PicoApplianceComposition::Minimal {
        return Ok(HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from(host_id),
            boot_id: BootId::from(boot_id),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from(PICO_MINIMAL_PROFILE),
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        });
    }

    validate_hello_initialization(initialized)?;
    Ok(HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host_id),
        boot_id: BootId::from(boot_id),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from(PICO_APPLIANCE_PROFILE),
        resources: appliance_resources(),
        capabilities: vec![
            appliance_offer(
                AP_CAPABILITY,
                "network/ap-ready",
                "conduit.network/ap-ready@1",
                "conduit.host/pico-ap-start@1",
                AP_RESOURCE_CLASS,
                1,
                1,
            ),
            appliance_offer(
                DHCP_CAPABILITY,
                "network/dhcp-lease-service",
                "conduit.network/dhcp-lease-service@1",
                "conduit.host/pico-dhcp-serve@1",
                DHCP_RESOURCE_CLASS,
                MAXIMUM_DHCP_LEASES as u32,
                576,
            ),
            appliance_offer(
                DNS_CAPABILITY,
                "network/dns-response-service",
                "conduit.network/dns-response-service@1",
                "conduit.host/pico-dns-serve@1",
                DNS_RESOURCE_CLASS,
                1,
                MAXIMUM_DNS_PACKET_BYTES,
            ),
            appliance_offer(
                HTTP_CAPABILITY,
                "network/http-hello-service",
                "conduit.network/http-hello-service@1",
                "conduit.host/pico-http-serve@1",
                HTTP_RESOURCE_CLASS,
                1,
                MAXIMUM_HTTP_REQUEST_BYTES,
            ),
        ],
        planner_capabilities: vec![],
    })
}

fn validate_hello_initialization(
    initialized: PicoApplianceInitialization,
) -> Result<(), PicoApplianceAdvertisementError> {
    if !initialized.radio_artifact {
        Err(PicoApplianceAdvertisementError::MissingRadioArtifact)
    } else if !initialized.ap_base {
        Err(PicoApplianceAdvertisementError::AccessPointBaseUnavailable)
    } else if !initialized.dhcp_base {
        Err(PicoApplianceAdvertisementError::DhcpBaseUnavailable)
    } else if !initialized.dns_base {
        Err(PicoApplianceAdvertisementError::DnsBaseUnavailable)
    } else if !initialized.http_base {
        Err(PicoApplianceAdvertisementError::HttpBaseUnavailable)
    } else {
        Ok(())
    }
}

fn appliance_resources() -> Vec<ResourceOffer> {
    vec![
        resource_offer("pico/appliance/radio-0", AP_RESOURCE_CLASS, 1),
        resource_offer(
            "pico/appliance/dhcp-pool-0",
            DHCP_RESOURCE_CLASS,
            MAXIMUM_DHCP_LEASES as u32,
        ),
        resource_offer("pico/appliance/dns-base-0", DNS_RESOURCE_CLASS, 1),
        resource_offer("pico/appliance/http-base-0", HTTP_RESOURCE_CLASS, 1),
    ]
}

fn appliance_offer(
    capability: &str,
    kind: &str,
    revision: &str,
    host_operation: &str,
    resource_class: &str,
    resource_units: u32,
    maximum_bytes: u32,
) -> CapabilityOffer {
    CapabilityOffer {
        capability_id: CapabilityId::from(capability),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: ImplementationOffer {
            implementation_id: ImplementationId::from(capability),
            artifact_id: ArtifactId::from(PICO_APPLIANCE_ARTIFACT),
            execution_profile_id: ExecutionProfileId::from(PICO_APPLIANCE_PROFILE),
        },
        startup_parameters: vec![],
        inputs: vec![],
        outputs: vec![],
        shorthand: None,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(host_operation),
            target_kind: None,
            maximum_in_flight: 1,
            maximum_input_bytes: maximum_bytes,
            maximum_output_bytes: maximum_bytes.max(MAXIMUM_HTTP_RESPONSE_BYTES),
        }],
        resource_requirements: vec![resource_requirement(resource_class, resource_units)],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: resource_units.min(u16::MAX.into()) as u16,
            max_queue_bytes: maximum_bytes,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_composition_advertises_no_appliance_family() {
        let advertisement = pico_appliance_advertisement(
            "pico/minimal",
            "boot/1",
            PicoApplianceComposition::Minimal,
            PicoApplianceInitialization::default(),
        )
        .unwrap();
        assert!(advertisement.capabilities.is_empty());
        assert!(advertisement.resources.is_empty());
        assert_eq!(advertisement.profile.as_str(), PICO_MINIMAL_PROFILE);
    }

    #[test]
    fn hello_composition_advertises_exact_initialized_family() {
        let advertisement = pico_appliance_advertisement(
            "pico/appliance",
            "boot/1",
            PicoApplianceComposition::Hello,
            PicoApplianceInitialization::hello_ready(),
        )
        .unwrap();
        let ids = advertisement
            .capabilities
            .iter()
            .map(|offer| offer.capability_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            [
                AP_CAPABILITY,
                DHCP_CAPABILITY,
                DNS_CAPABILITY,
                HTTP_CAPABILITY
            ]
        );
        assert_eq!(advertisement.resources.len(), 4);
        assert!(advertisement.capabilities.iter().all(|offer| {
            offer.implementation.artifact_id.as_str() == PICO_APPLIANCE_ARTIFACT
                && offer.implementation.execution_profile_id.as_str() == PICO_APPLIANCE_PROFILE
                && offer.limits.max_active_instances == 1
                && offer.host_operations.len() == 1
                && offer.resource_requirements.len() == 1
        }));
    }

    #[test]
    fn incomplete_hello_initialization_fails_closed_by_exact_cause() {
        let cases = [
            (
                PicoApplianceInitialization::default(),
                PicoApplianceAdvertisementError::MissingRadioArtifact,
            ),
            (
                PicoApplianceInitialization {
                    radio_artifact: true,
                    ..PicoApplianceInitialization::default()
                },
                PicoApplianceAdvertisementError::AccessPointBaseUnavailable,
            ),
            (
                PicoApplianceInitialization {
                    radio_artifact: true,
                    ap_base: true,
                    ..PicoApplianceInitialization::default()
                },
                PicoApplianceAdvertisementError::DhcpBaseUnavailable,
            ),
            (
                PicoApplianceInitialization {
                    radio_artifact: true,
                    ap_base: true,
                    dhcp_base: true,
                    ..PicoApplianceInitialization::default()
                },
                PicoApplianceAdvertisementError::DnsBaseUnavailable,
            ),
            (
                PicoApplianceInitialization {
                    radio_artifact: true,
                    ap_base: true,
                    dhcp_base: true,
                    dns_base: true,
                    http_base: false,
                },
                PicoApplianceAdvertisementError::HttpBaseUnavailable,
            ),
        ];
        for (initialization, expected) in cases {
            assert_eq!(
                pico_appliance_advertisement(
                    "pico/appliance",
                    "boot/1",
                    PicoApplianceComposition::Hello,
                    initialization,
                ),
                Err(expected)
            );
        }
    }
}
