//! Bounded ordinary Observatory export prepared before Play and emitted only
//! after the expected terminal kernel result has been verified.

use alloc::{format, string::String, vec, vec::Vec};
use core::fmt::Write;

use conduit_core::{
    ArtifactId, ConnectionTerminalDisposition, HostBaseId, HostBaseKindId, Observation,
    ObservationKind, SignId, TerminalDisposition, bind_sign,
};
use conduit_observatory::{
    BaseReport, BootProofClass, CapabilityAvailability, CapabilityStatusReport, CapabilitySupport,
    HostReport, MemoryMapSummary, ObservatorySnapshot, OfferFreshness, OperationalState,
    PlanLifecycle, PlayConnectionReport, PlayPlacementReport, PlayReport, PressureReport,
    RetentionReport, SNAPSHOT_SCHEMA, SealedBootProvenanceReport, validate_snapshot,
};

use crate::{
    boot::BootRecord, identity::BootIdentities, offer::HostOffer,
    ordinary_plan::PreparedOrdinaryPlay,
};

pub const EXPORT_PREFIX: &str = "CONDUIT_OBSERVATORY_SNAPSHOT ";
pub const MAX_EXPORT_BYTES: usize = 64 * 1024;
const RETAINED_SIGN_CAPACITY: u32 = 64;

pub struct PreparedObservatoryExport {
    encoded: String,
}

impl PreparedObservatoryExport {
    pub fn as_bytes(&self) -> &[u8] {
        self.encoded.as_bytes()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportError {
    UnsupportedBootArtifacts,
    UnsupportedFramebuffer,
    InvalidSnapshot,
    EncodingFailed,
    ExportTooLarge,
}

impl ExportError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedBootArtifacts => "observatory-boot-artifacts-unrepresented",
            Self::UnsupportedFramebuffer => "observatory-framebuffer-unrepresented",
            Self::InvalidSnapshot => "observatory-snapshot-invalid",
            Self::EncodingFailed => "observatory-snapshot-encoding-failed",
            Self::ExportTooLarge => "observatory-snapshot-too-large",
        }
    }
}

pub fn prepare_export(
    record: &BootRecord,
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    prepared: &PreparedOrdinaryPlay,
    build_id: &str,
    image_id: &str,
) -> Result<PreparedObservatoryExport, ExportError> {
    if record.artifact_count != 0 {
        return Err(ExportError::UnsupportedBootArtifacts);
    }
    if record.framebuffer_count != 0 {
        return Err(ExportError::UnsupportedFramebuffer);
    }
    let host_id = prepared.advertisement.host_id.clone();
    let boot_id = prepared.advertisement.boot_id.clone();
    if host_id.as_str() != hex_identity(&identities.host)
        || boot_id.as_str() != hex_identity(&identities.boot)
    {
        return Err(ExportError::InvalidSnapshot);
    }
    let capabilities = prepared
        .advertisement
        .capabilities
        .iter()
        .map(|capability| CapabilityStatusReport {
            capability_id: capability.capability_id.clone(),
            freshness: OfferFreshness::Fresh,
            support: CapabilitySupport::Supported,
            availability: CapabilityAvailability::Available,
        })
        .collect();
    let bases = offer
        .bases
        .iter()
        .map(|base| BaseReport {
            host_id: host_id.clone(),
            boot_id: boot_id.clone(),
            base_id: HostBaseId::from(hex_identity(&base.id)),
            kind_id: HostBaseKindId::from(format!("conduitos.base/{}@1", base.kind.as_str())),
            state: OperationalState::Available,
            capacity_units: u64::from(base.capacity),
        })
        .collect::<Vec<_>>();
    let fragment = prepared
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.fragment_id == prepared.fragment_id)
        .ok_or(ExportError::InvalidSnapshot)?;
    let play = PlayReport {
        active_play_id: prepared.active_play.active_play_id.clone(),
        plan_id: prepared.plan_id.clone(),
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        lifecycle: PlanLifecycle::Completed,
        terminal_disposition: Some(TerminalDisposition::Completed),
        failure_message: None,
        placements: fragment
            .placements
            .iter()
            .map(|placement| PlayPlacementReport {
                placement_id: placement.placement_id.clone(),
                lifecycle: PlanLifecycle::Completed,
                terminal_disposition: Some(TerminalDisposition::Completed),
                failure_message: None,
            })
            .collect(),
        connections: fragment
            .connections
            .iter()
            .map(|connection| PlayConnectionReport {
                connection_id: connection.connection_id.clone(),
                lifecycle: PlanLifecycle::Completed,
                terminal_disposition: Some(ConnectionTerminalDisposition {
                    disposition: TerminalDisposition::Completed,
                    last_accepted_sequence: Some(0),
                    last_manifested_sequence: Some(0),
                    undeliverable_items: 0,
                }),
                pressure: Some(PressureReport {
                    current_in_flight_items: Some(0),
                    current_buffered_bytes: Some(0),
                    pressure_events: 0,
                    last_pressure_sequence: None,
                }),
                failure_message: None,
            })
            .collect(),
    };
    let historical_observations = historical_signs(prepared, fragment);
    let observations = terminal_signs(prepared, fragment);
    let retained_items = u32::try_from(
        observations
            .len()
            .checked_add(historical_observations.len())
            .ok_or(ExportError::InvalidSnapshot)?,
    )
    .map_err(|_| ExportError::InvalidSnapshot)?;
    let snapshot = ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts: vec![HostReport {
            advertisement: prepared.advertisement.clone(),
            state: OperationalState::Available,
            capabilities,
        }],
        bases,
        lines: Vec::new(),
        plans: vec![prepared.plan.clone()],
        plays: vec![play],
        observations,
        historical_observations,
        sealed_boot_provenance: vec![SealedBootProvenanceReport {
            host_id,
            boot_id,
            firmware_environment: record.firmware.as_str().into(),
            adapter_name: "Limine".into(),
            adapter_version: "12.5.2".into(),
            adapter_revision: "3".into(),
            image_id: ArtifactId::from(image_id),
            build_id: ArtifactId::from(build_id),
            memory_map: MemoryMapSummary {
                normalized_region_count: record.memory_region_count,
                runtime_arena_bytes: record.runtime_arena.length,
            },
            boot_artifacts: Vec::new(),
            initial_plan_artifact_id: None,
            recovery_plan_artifact_id: None,
            framebuffers: Vec::new(),
            proof_class: BootProofClass::FreestandingEmulator,
        }],
        retention: RetentionReport {
            item_capacity: RETAINED_SIGN_CAPACITY,
            retained_items,
            dropped_items: 0,
        },
    };
    validate_snapshot(&snapshot).map_err(|_| ExportError::InvalidSnapshot)?;
    let encoded = serde_json::to_string(&snapshot).map_err(|_| ExportError::EncodingFailed)?;
    if encoded.len() > MAX_EXPORT_BYTES {
        return Err(ExportError::ExportTooLarge);
    }
    Ok(PreparedObservatoryExport { encoded })
}

fn historical_signs(
    prepared: &PreparedOrdinaryPlay,
    fragment: &conduit_core::PlanFragment,
) -> Vec<Observation> {
    let mut observations = vec![
        observation(
            prepared,
            0,
            None,
            None,
            None,
            None,
            ObservationKind::HostStarted,
        ),
        observation(
            prepared,
            1,
            None,
            None,
            None,
            None,
            ObservationKind::AdvertisementPublished,
        ),
        observation(
            prepared,
            2,
            None,
            Some(prepared.plan_id.clone()),
            None,
            None,
            ObservationKind::PlanFragmentReceived,
        ),
    ];
    for (index, placement) in fragment.placements.iter().enumerate() {
        observations.push(observation(
            prepared,
            3 + index as u64,
            None,
            Some(prepared.plan_id.clone()),
            Some(placement.placement_id.clone()),
            None,
            ObservationKind::PlacementPrepared,
        ));
    }
    observations.push(observation(
        prepared,
        5,
        Some(prepared.active_play.active_play_id.clone()),
        Some(prepared.plan_id.clone()),
        None,
        None,
        ObservationKind::PlanPlayStarted,
    ));
    observations
}

fn terminal_signs(
    prepared: &PreparedOrdinaryPlay,
    fragment: &conduit_core::PlanFragment,
) -> Vec<Observation> {
    let active_play_id = Some(prepared.active_play.active_play_id.clone());
    let plan_id = Some(prepared.plan_id.clone());
    let mut observations = fragment
        .placements
        .iter()
        .enumerate()
        .map(|(index, placement)| {
            observation(
                prepared,
                6 + index as u64,
                active_play_id.clone(),
                plan_id.clone(),
                Some(placement.placement_id.clone()),
                None,
                ObservationKind::PlacementTerminal {
                    disposition: TerminalDisposition::Completed,
                },
            )
        })
        .collect::<Vec<_>>();
    observations.extend(
        fragment
            .connections
            .iter()
            .enumerate()
            .map(|(index, connection)| {
                observation(
                    prepared,
                    6 + fragment.placements.len() as u64 + index as u64,
                    active_play_id.clone(),
                    plan_id.clone(),
                    None,
                    Some(connection.connection_id.clone()),
                    ObservationKind::ConnectionTerminal {
                        disposition: ConnectionTerminalDisposition {
                            disposition: TerminalDisposition::Completed,
                            last_accepted_sequence: Some(0),
                            last_manifested_sequence: Some(0),
                            undeliverable_items: 0,
                        },
                    },
                )
            }),
    );
    observations.push(observation(
        prepared,
        6 + fragment.placements.len() as u64 + fragment.connections.len() as u64,
        active_play_id,
        plan_id,
        None,
        None,
        ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed,
        },
    ));
    observations
}

fn observation(
    prepared: &PreparedOrdinaryPlay,
    sequence: u64,
    active_play_id: Option<conduit_core::ActivePlayId>,
    plan_id: Option<conduit_core::PlanId>,
    placement_id: Option<conduit_core::PlacementId>,
    connection_id: Option<conduit_core::ConnectionId>,
    kind: ObservationKind,
) -> Observation {
    let identity = bind_sign(
        &prepared.advertisement.host_id,
        &prepared.advertisement.boot_id,
        active_play_id.as_ref(),
        sequence,
    );
    Observation {
        sign_id: SignId::from(identity.sign_id.as_str()),
        active_play_id,
        presentation_id: None,
        host_id: prepared.advertisement.host_id.clone(),
        boot_id: prepared.advertisement.boot_id.clone(),
        plan_id,
        placement_id,
        connection_id,
        kind,
    }
}

fn hex_identity(bytes: &[u8; 32]) -> String {
    let mut result = String::with_capacity(64);
    for byte in bytes {
        write!(&mut result, "{byte:02x}").expect("writing to String cannot fail");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::BootIdentities,
        offer::{CpuFeatures, HostOffer},
        ordinary_plan,
    };

    fn fixture() -> (BootRecord, BootIdentities, HostOffer<'static>) {
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let offer = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            256 * 1024,
        );
        let record = BootRecord {
            firmware: crate::boot::Firmware::X86Bios,
            timestamp: 1,
            hhdm_offset: 2,
            image_physical_start: 3,
            image_length: 4,
            memory_region_count: 5,
            artifact_count: 0,
            framebuffer_count: 0,
            command_line_bytes: 0,
            runtime_arena: crate::boot::RuntimeArena {
                physical_start: 6,
                length: 256 * 1024,
            },
        };
        (record, identities, offer)
    }

    #[test]
    fn export_is_an_exact_bounded_v2_snapshot() {
        let (record, identities, offer) = fixture();
        let prepared = ordinary_plan::prepare(&identities, &offer, "build").unwrap();
        let export =
            prepare_export(&record, &identities, &offer, &prepared, "build", "image").unwrap();
        let snapshot: ObservatorySnapshot = serde_json::from_slice(export.as_bytes()).unwrap();

        validate_snapshot(&snapshot).unwrap();
        assert_eq!(snapshot.bases.len(), 7);
        assert_eq!(
            snapshot.hosts[0].advertisement.planner_capabilities.len(),
            0
        );
        assert_eq!(snapshot.plays[0].lifecycle, PlanLifecycle::Completed);
        assert_eq!(snapshot.observations.len(), 4);
        assert_eq!(snapshot.historical_observations.len(), 6);
        assert_eq!(snapshot.sealed_boot_provenance.len(), 1);
        assert!(snapshot.bases.iter().all(|base| {
            base.kind_id.as_str() != "Limine" && base.kind_id.as_str() != "x86-bios"
        }));
        let report = conduit_observatory::build_report(&snapshot).unwrap();
        let linear = conduit_observatory::render_text_report(&report);
        assert_eq!(report.execution_regions.len(), 1);
        assert!(linear.contains("execution_regions 1"));
        assert!(linear.contains("scheduling=CooperativeBoundedStep lane_count=1"));
        assert!(linear.contains("preemption_required=false isolation_required=false"));
        let mut rendered_copy = report.clone();
        rendered_copy.execution_regions[0].lane_count = 99;
        assert_eq!(
            snapshot.plans[0].fragments[0].execution_regions[0].lane_count,
            1
        );
        assert!(linear.contains("boot provenance [sealed] 1"));
        assert!(linear.contains("history=current"));
        assert!(linear.contains("history=historical"));
        assert!(export.as_bytes().len() <= MAX_EXPORT_BYTES);
    }

    #[test]
    fn stale_provenance_duplicate_bases_and_signs_fail_closed_while_gaps_remain_visible() {
        let (record, identities, offer) = fixture();
        let prepared = ordinary_plan::prepare(&identities, &offer, "build").unwrap();
        let export =
            prepare_export(&record, &identities, &offer, &prepared, "build", "image").unwrap();
        let snapshot: ObservatorySnapshot = serde_json::from_slice(export.as_bytes()).unwrap();

        let mut duplicate_base = snapshot.clone();
        duplicate_base.bases.push(duplicate_base.bases[0].clone());
        assert!(validate_snapshot(&duplicate_base).is_err());

        let mut stale_provenance = snapshot.clone();
        stale_provenance.sealed_boot_provenance[0].boot_id =
            conduit_core::BootId::from("stale-boot");
        assert!(validate_snapshot(&stale_provenance).is_err());

        let mut duplicate_sign = snapshot.clone();
        duplicate_sign.historical_observations[0].sign_id =
            duplicate_sign.observations[0].sign_id.clone();
        assert!(validate_snapshot(&duplicate_sign).is_err());

        let mut gaps = snapshot;
        gaps.historical_observations[0].kind = ObservationKind::SignGap { dropped: 3 };
        gaps.retention.dropped_items = 2;
        let report = conduit_observatory::build_report(&gaps).unwrap();
        assert_eq!(report.retention.visible_gap_count, 5);
    }

    #[test]
    fn unrepresentable_boot_inputs_fail_before_play() {
        let (mut record, identities, offer) = fixture();
        let prepared = ordinary_plan::prepare(&identities, &offer, "build").unwrap();
        record.artifact_count = 1;
        assert_eq!(
            prepare_export(&record, &identities, &offer, &prepared, "build", "image").err(),
            Some(ExportError::UnsupportedBootArtifacts)
        );
        record.artifact_count = 0;
        record.framebuffer_count = 1;
        assert_eq!(
            prepare_export(&record, &identities, &offer, &prepared, "build", "image").err(),
            Some(ExportError::UnsupportedFramebuffer)
        );
    }
}
