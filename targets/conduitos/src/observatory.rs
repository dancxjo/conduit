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
    FramebufferBasis, HostReport, MemoryMapSummary, ObservatorySnapshot, OfferFreshness,
    OperationalState, PlanLifecycle, PlayConnectionReport, PlayPlacementReport, PlayReport,
    PressureReport, RetentionReport, SNAPSHOT_SCHEMA, SealedBootProvenanceReport,
    validate_snapshot,
};

use crate::{
    boot::BootRecord, dual_region_plan::PreparedDualRegionPlay, identity::BootIdentities,
    offer::HostOffer,
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
    prepared: &PreparedDualRegionPlay,
    build_id: &str,
    image_id: &str,
    framebuffer: Option<&FramebufferBasis>,
) -> Result<PreparedObservatoryExport, ExportError> {
    if record.artifact_count != 0 {
        return Err(ExportError::UnsupportedBootArtifacts);
    }
    if usize::from(record.framebuffer_count) != framebuffer.iter().count() {
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
    let mut bases = offer
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
    append_framebuffer_base(&mut bases, &host_id, &boot_id, framebuffer)?;
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
            devices: Vec::new(),
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
            image_build_trace: None,
            memory_map: MemoryMapSummary {
                normalized_region_count: record.memory_region_count,
                runtime_arena_bytes: record.runtime_arena.length,
            },
            boot_artifacts: Vec::new(),
            initial_plan_artifact_id: None,
            recovery_plan_artifact_id: None,
            framebuffers: framebuffer.into_iter().cloned().collect(),
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

#[cfg(target_arch = "x86_64")]
pub struct ImageBoundProvenance<'a> {
    pub profile_id: &'a str,
    pub build_id: &'a str,
    pub image_binding: &'a str,
}

#[cfg(target_arch = "x86_64")]
pub fn prepare_image_bound_export(
    record: &BootRecord,
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    prepared: &PreparedDualRegionPlay,
    provenance: ImageBoundProvenance<'_>,
    framebuffer: Option<&FramebufferBasis>,
) -> Result<PreparedObservatoryExport, ExportError> {
    if provenance.build_id != offer.capabilities[0].artifact_build {
        return Err(ExportError::InvalidSnapshot);
    }
    let export = prepare_export(
        record,
        identities,
        offer,
        prepared,
        provenance.build_id,
        provenance.image_binding,
        framebuffer,
    )?;
    let mut snapshot: ObservatorySnapshot =
        serde_json::from_str(&export.encoded).map_err(|_| ExportError::EncodingFailed)?;
    snapshot.sealed_boot_provenance[0].image_build_trace =
        Some(conduit_observatory::ImageBuildTraceReport {
            profile_id: provenance.profile_id.into(),
            inclusions: Vec::new(),
        });
    let encoded = serde_json::to_string(&snapshot).map_err(|_| ExportError::EncodingFailed)?;
    if encoded.len() > MAX_EXPORT_BYTES {
        return Err(ExportError::ExportTooLarge);
    }
    Ok(PreparedObservatoryExport { encoded })
}

pub(crate) fn append_framebuffer_base(
    bases: &mut Vec<BaseReport>,
    host_id: &conduit_core::HostId,
    boot_id: &conduit_core::BootId,
    framebuffer: Option<&FramebufferBasis>,
) -> Result<(), ExportError> {
    let Some(framebuffer) = framebuffer else {
        return Ok(());
    };
    let capacity_units = u64::from(framebuffer.pitch_bytes)
        .checked_mul(u64::from(framebuffer.height))
        .ok_or(ExportError::InvalidSnapshot)?;
    bases.push(BaseReport {
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        base_id: framebuffer.base_id.clone(),
        kind_id: HostBaseKindId::from("conduitos.base/framebuffer@1"),
        state: OperationalState::Available,
        capacity_units,
    });
    Ok(())
}

fn historical_signs(
    prepared: &PreparedDualRegionPlay,
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
        3 + fragment.placements.len() as u64,
        Some(prepared.active_play.active_play_id.clone()),
        Some(prepared.plan_id.clone()),
        None,
        None,
        ObservationKind::PlanPlayStarted,
    ));
    observations
}

fn terminal_signs(
    prepared: &PreparedDualRegionPlay,
    fragment: &conduit_core::PlanFragment,
) -> Vec<Observation> {
    let active_play_id = Some(prepared.active_play.active_play_id.clone());
    let plan_id = Some(prepared.plan_id.clone());
    let terminal_start = 4 + fragment.placements.len() as u64;
    let mut observations = vec![observation(
        prepared,
        terminal_start,
        active_play_id.clone(),
        plan_id.clone(),
        None,
        None,
        ObservationKind::ExecutionRegionOverlap {
            waiting_region_id: conduit_core::ExecutionRegionId::from("region/timer"),
            progressing_region_id: conduit_core::ExecutionRegionId::from("region/text"),
            physical_parallelism: false,
        },
    )];
    observations.extend(
        fragment
            .placements
            .iter()
            .enumerate()
            .map(|(index, placement)| {
                observation(
                    prepared,
                    terminal_start + 1 + index as u64,
                    active_play_id.clone(),
                    plan_id.clone(),
                    Some(placement.placement_id.clone()),
                    None,
                    ObservationKind::PlacementTerminal {
                        disposition: TerminalDisposition::Completed,
                    },
                )
            }),
    );
    observations.extend(
        fragment
            .connections
            .iter()
            .enumerate()
            .map(|(index, connection)| {
                observation(
                    prepared,
                    terminal_start + 1 + fragment.placements.len() as u64 + index as u64,
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
        terminal_start + 1 + fragment.placements.len() as u64 + fragment.connections.len() as u64,
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
    prepared: &PreparedDualRegionPlay,
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
mod tests;
