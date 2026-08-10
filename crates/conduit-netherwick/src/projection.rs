use crate::{
    brainstem_advertisement, motherbrain_advertisement, pinned_profile, BRAINSTEM_BOOT,
    BRAINSTEM_HOST, MOTHERBRAIN_BOOT, MOTHERBRAIN_HOST,
};
use conduit_core::{
    AuthorityGrantId, ConnectionBase, ConnectionBaseInstanceId, HostBaseId, HostBaseKindId,
    LineAvailability, LineAvailabilitySign, LineContinuation, LineContract, LineDuplex, LineId,
    LineOffer, LineOrdering, LineReliability, LineScope, LineSecurity, LineTrafficShape,
    LinkAuthorityReference, LinkBinding, LinkBindingId, LinkCredentialReference, LinkEndpoint,
    LinkEndpointId, LinkLimits, SignId,
};
use conduit_observatory::{
    BaseReport, CapabilityAvailability, CapabilityStatusReport, CapabilitySupport, HostReport,
    LineReport, ObservatorySnapshot, OfferFreshness, OperationalState, RetentionReport,
    SNAPSHOT_SCHEMA,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActuatorDescription {
    pub semantic_kind: String,
    pub physical_device: String,
    pub capability_exists_in_netherwick: bool,
    pub conduit_control_state: String,
    pub physical_safe_state: String,
    pub local_inhibit_owner: String,
    pub actionable_offer_present: bool,
    pub authority_grant_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DescribeSign {
    ConfigurationProjected,
    SensorObserved { sensor: String, state: String },
    ActuatorControlAbsent,
    LostHost,
    ReplacedBoot,
    StaleDeviceOrBase,
    LostLine,
    MalformedReport,
    UnknownSafetyState,
    ActuatorCommandRefused,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DescribeProjection {
    pub proof_class: String,
    pub snapshot: ObservatorySnapshot,
    pub actuator: ActuatorDescription,
    pub signs: Vec<DescribeSign>,
}

pub fn describe_projection() -> DescribeProjection {
    let brainstem = brainstem_advertisement();
    let motherbrain = motherbrain_advertisement();
    let profile = pinned_profile();
    DescribeProjection {
        proof_class: "configuration-projection-not-live-hardware".into(),
        snapshot: ObservatorySnapshot {
            schema: SNAPSHOT_SCHEMA.into(),
            hosts: vec![host_report(motherbrain), host_report(brainstem.clone())],
            bases: vec![
                BaseReport {
                    host_id: brainstem.host_id.clone(),
                    boot_id: brainstem.boot_id.clone(),
                    base_id: HostBaseId::from("netherwick/pete-brainstem/create-uart"),
                    kind_id: HostBaseKindId::from("netherwick.base/create-oi-uart@57600-8n1"),
                    state: OperationalState::Unknown,
                    capacity_units: 1,
                },
                BaseReport {
                    host_id: brainstem.host_id.clone(),
                    boot_id: brainstem.boot_id.clone(),
                    base_id: HostBaseId::from("netherwick/pete-brainstem/usb-cdc"),
                    kind_id: HostBaseKindId::from("netherwick.base/usb-cdc@1"),
                    state: OperationalState::Unknown,
                    capacity_units: 1,
                },
            ],
            lines: vec![LineReport {
                offer: usb_line(),
                state: OperationalState::Unknown,
            }],
            plans: vec![],
            plays: vec![],
            observations: vec![],
            historical_observations: vec![],
            sealed_boot_provenance: vec![],
            retention: RetentionReport {
                item_capacity: 16,
                retained_items: 0,
                dropped_items: 0,
            },
        },
        actuator: ActuatorDescription {
            semantic_kind: crate::DRIVE_KIND.into(),
            physical_device: format!("{} differential Create OI", profile.body_kind),
            capability_exists_in_netherwick: profile
                .compiled_providers
                .iter()
                .any(|p| p.capability == "netherwick/capability/create-motion"),
            conduit_control_state: "uncommandable-no-offer-no-authority".into(),
            physical_safe_state: "unknown-not-live-observed".into(),
            local_inhibit_owner: "pinned pete-brainstem safety/runtime lane".into(),
            actionable_offer_present: false,
            authority_grant_present: false,
        },
        signs: vec![
            DescribeSign::ConfigurationProjected,
            DescribeSign::SensorObserved {
                sensor: "bump".into(),
                state: "configured-current-state-unknown".into(),
            },
            DescribeSign::SensorObserved {
                sensor: "imu".into(),
                state: "configured-current-state-unknown".into(),
            },
            DescribeSign::ActuatorControlAbsent,
            DescribeSign::UnknownSafetyState,
            DescribeSign::Terminal,
        ],
    }
}

pub fn validate_projection_json(bytes: &[u8]) -> Result<DescribeProjection, DescribeSign> {
    let projection: DescribeProjection =
        serde_json::from_slice(bytes).map_err(|_| DescribeSign::MalformedReport)?;
    conduit_observatory::validate_snapshot(&projection.snapshot)
        .map_err(|_| DescribeSign::MalformedReport)?;
    Ok(projection)
}

fn host_report(advertisement: conduit_core::HostAdvertisement) -> HostReport {
    let capabilities = advertisement
        .capabilities
        .iter()
        .map(|offer| CapabilityStatusReport {
            capability_id: offer.capability_id.clone(),
            freshness: OfferFreshness::Unknown,
            support: CapabilitySupport::Supported,
            availability: CapabilityAvailability::Unknown,
        })
        .collect();
    HostReport {
        advertisement,
        state: OperationalState::Unknown,
        capabilities,
    }
}

fn usb_line() -> LineOffer {
    let binding = LinkBinding {
        binding_id: LinkBindingId::from("netherwick/motherbrain-brainstem-usb-binding"),
        source: LinkEndpoint {
            host_id: MOTHERBRAIN_HOST.into(),
            boot_id: MOTHERBRAIN_BOOT.into(),
            endpoint_id: LinkEndpointId::from("netherwick/motherbrain/usb"),
        },
        sink: LinkEndpoint {
            host_id: BRAINSTEM_HOST.into(),
            boot_id: BRAINSTEM_BOOT.into(),
            endpoint_id: LinkEndpointId::from("netherwick/brainstem/usb"),
        },
        base: ConnectionBase::UsbCdc,
        base_instance_id: ConnectionBaseInstanceId::from("netherwick/describe-fixture/usb-cdc-0"),
        credential: LinkCredentialReference::None,
        authority: LinkAuthorityReference::Grant(AuthorityGrantId::from(
            "netherwick/describe-observation-only-usb",
        )),
        limits: LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 512,
            maximum_buffered_bytes: 512,
            maximum_frame_bytes: 512,
        },
    };
    LineOffer {
        line_id: LineId::from("netherwick/motherbrain-brainstem-observation-line"),
        availability: LineAvailabilitySign {
            line_id: LineId::from("netherwick/motherbrain-brainstem-observation-line"),
            binding_id: binding.binding_id.clone(),
            availability: LineAvailability::Unavailable,
            sign_id: SignId::from("netherwick/sign/usb-line-not-live-observed"),
        },
        binding,
        contract: LineContract {
            scope: LineScope::PointToPoint,
            traffic_shape: LineTrafficShape::ByteStream,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::PhysicalPossession,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_observatory_projection_shows_topology_and_absent_authority() {
        let projection = describe_projection();
        conduit_observatory::validate_snapshot(&projection.snapshot).unwrap();
        let report = conduit_observatory::build_report(&projection.snapshot).unwrap();
        let rendered = conduit_observatory::render_text_report(&report);
        for expected in [
            BRAINSTEM_HOST,
            MOTHERBRAIN_HOST,
            "usb-cdc",
            "observe-bump",
            "observe-imu",
        ] {
            assert!(
                rendered
                    .to_ascii_lowercase()
                    .contains(&expected.to_ascii_lowercase()),
                "missing {expected}"
            );
        }
        assert!(!projection.actuator.actionable_offer_present);
        assert!(!projection.actuator.authority_grant_present);
        assert!(projection.snapshot.plans.is_empty());
        assert!(projection.snapshot.plays.is_empty());

        let mut patchbay = patchbay_model::PatchbayTopology::new(1).unwrap();
        patchbay.ingest(&projection.snapshot).unwrap();
        let document = patchbay.document(None).unwrap();
        let linear = document.lines().join("\n").to_ascii_lowercase();
        for expected in [
            "pete-brainstem",
            "pete-motherbrain",
            "observe-bump",
            "observe-imu",
            "observe-safety-boundary",
            "describe-actuator",
        ] {
            assert!(linear.contains(expected), "Patchbay omitted {expected}");
        }
        assert!(!linear.contains("drive-differential"));
    }

    #[test]
    fn failure_signs_are_distinct_bounded_and_content_safe() {
        let failures = [
            DescribeSign::LostHost,
            DescribeSign::ReplacedBoot,
            DescribeSign::StaleDeviceOrBase,
            DescribeSign::LostLine,
            DescribeSign::MalformedReport,
            DescribeSign::UnknownSafetyState,
            DescribeSign::ActuatorCommandRefused,
        ];
        assert_eq!(failures.len(), 7);
        let encoded = serde_json::to_vec(&failures).unwrap();
        assert!(encoded.len() < 512);
        assert_eq!(
            failures
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            failures.len()
        );
        assert_eq!(
            validate_projection_json(b"{}"),
            Err(DescribeSign::MalformedReport)
        );
    }
}
