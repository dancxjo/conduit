//! Ordinary Observatory export for the completed keyboard-text Form.

use alloc::{format, string::String, vec, vec::Vec};

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
    boot::BootRecord, identity::BootIdentities, keyboard_text_plan::PreparedKeyboardTextPlay,
    observatory::ExportError, offer::HostOffer,
};

pub const EXPORT_PREFIX: &str = "CONDUIT_KEYBOARD_TEXT_OBSERVATORY ";
const RETAINED_SIGN_CAPACITY: u32 = 32;

pub fn completed_snapshot(
    record: &BootRecord,
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    prepared: &PreparedKeyboardTextPlay,
    build_id: &str,
    image_id: &str,
    framebuffer: Option<&FramebufferBasis>,
) -> Result<String, ExportError> {
    if record.artifact_count != 0 {
        return Err(ExportError::UnsupportedBootArtifacts);
    }
    if usize::from(record.framebuffer_count) != framebuffer.iter().count() {
        return Err(ExportError::UnsupportedFramebuffer);
    }
    let host_id = prepared.advertisement.host_id.clone();
    let boot_id = prepared.advertisement.boot_id.clone();
    if host_id.as_str() != crate::identity::hex(&identities.host)
        || boot_id.as_str() != crate::identity::hex(&identities.boot)
    {
        return Err(ExportError::InvalidSnapshot);
    }
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(ExportError::InvalidSnapshot)?;
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
            base_id: HostBaseId::from(crate::identity::hex(&base.id)),
            kind_id: HostBaseKindId::from(format!("conduitos.base/{}@1", base.kind.as_str())),
            state: OperationalState::Available,
            capacity_units: u64::from(base.capacity),
        })
        .collect::<Vec<_>>();
    let keyboard = offer
        .keyboard
        .ok_or(ExportError::InvalidSnapshot)?
        .realization;
    for (id, kind, capacity) in [
        (keyboard.controller_id, "xhci", 1_u64),
        (keyboard.device_id, "usb-device", 1),
        (keyboard.interface_id, "usb-interface", 1),
        (
            keyboard.endpoint_id,
            "usb-interrupt-endpoint",
            u64::from(keyboard.report_buffers),
        ),
    ] {
        bases.push(BaseReport {
            host_id: host_id.clone(),
            boot_id: boot_id.clone(),
            base_id: HostBaseId::from(crate::identity::hex(&id)),
            kind_id: HostBaseKindId::from(format!("conduitos.base/{kind}@1")),
            state: OperationalState::Available,
            capacity_units: capacity,
        });
    }
    let play = PlayReport {
        active_play_id: prepared.active_play.active_play_id.clone(),
        plan_id: prepared.plan.plan_id.clone(),
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
                    last_accepted_sequence: Some(7),
                    last_manifested_sequence: Some(7),
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
    let historical_observations = vec![
        observation(prepared, 0, None, ObservationKind::HostStarted),
        observation(prepared, 1, None, ObservationKind::AdvertisementPublished),
        observation(prepared, 2, None, ObservationKind::PlanFragmentReceived),
        observation(prepared, 3, None, ObservationKind::PlanPlayStarted),
    ];
    let observations = vec![observation(
        prepared,
        4,
        None,
        ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed,
        },
    )];
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
            framebuffers: framebuffer.into_iter().cloned().collect(),
            proof_class: BootProofClass::FreestandingEmulator,
        }],
        retention: RetentionReport {
            item_capacity: RETAINED_SIGN_CAPACITY,
            retained_items: 5,
            dropped_items: 0,
        },
    };
    validate_snapshot(&snapshot).map_err(|_| ExportError::InvalidSnapshot)?;
    let encoded = serde_json::to_string(&snapshot).map_err(|_| ExportError::EncodingFailed)?;
    if encoded.len() > crate::observatory::MAX_EXPORT_BYTES {
        return Err(ExportError::ExportTooLarge);
    }
    Ok(encoded)
}

fn observation(
    prepared: &PreparedKeyboardTextPlay,
    sequence: u64,
    placement_id: Option<conduit_core::PlacementId>,
    kind: ObservationKind,
) -> Observation {
    let active_play_id = match kind {
        ObservationKind::HostStarted
        | ObservationKind::AdvertisementPublished
        | ObservationKind::PlanFragmentReceived => None,
        _ => Some(prepared.active_play.active_play_id.clone()),
    };
    let plan_id = match kind {
        ObservationKind::HostStarted | ObservationKind::AdvertisementPublished => None,
        _ => Some(prepared.plan.plan_id.clone()),
    };
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
        connection_id: None,
        kind,
    }
}
