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
    if !crate::boot::has_only_optional_spore_artifact(record) {
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
            provider_instance_id: conduit_core::BaseInstanceId::from(crate::identity::hex(
                &base.provider_instance_id,
            )),
            provider_generation: base.provider_generation,
            kind_id: HostBaseKindId::from(format!("conduitos.base/{}@1", base.kind.as_str())),
            implementation_id: None,
            enforcement_class: None,
            lifecycle: None,
            state: OperationalState::Available,
            capacity_units: u64::from(base.capacity),
        })
        .collect::<Vec<_>>();
    crate::observatory::append_advertised_bases(&mut bases, &prepared.advertisement);
    crate::observatory::append_framebuffer_base(&mut bases, &host_id, &boot_id, framebuffer)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::BootIdentities,
        keyboard_offer::{KeyboardMechanism, KeyboardRealization},
        offer::{CpuFeatures, HostOffer},
    };

    #[test]
    fn snapshot_reports_the_exact_keyboard_base_that_drove_planning() {
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
            1_048_576,
        )
        .with_keyboard(
            KeyboardRealization {
                mechanism: KeyboardMechanism::UsbHid,
                controller_id: [3; 32],
                device_id: [4; 32],
                interface_id: [5; 32],
                endpoint_id: [6; 32],
                report_buffers: 2,
                transition_slots: 8,
                operation_slots: 2,
            },
            "build",
        )
        .unwrap();
        let prepared = crate::keyboard_text_plan::prepare(&identities, &offer, "build").unwrap();
        let record = BootRecord {
            firmware: crate::boot::Firmware::X86Bios,
            timestamp: 1,
            hhdm_offset: 2,
            rsdp_address: None,
            image_physical_start: 3,
            image_length: 4,
            memory_region_count: 5,
            artifact_count: 0,
            framebuffer_count: 0,
            command_line_bytes: 0,
            runtime_arena: crate::boot::RuntimeArena {
                physical_start: 6,
                length: 1_048_576,
            },
        };
        let encoded = completed_snapshot(
            &record,
            &identities,
            &offer,
            &prepared,
            "build",
            "image",
            None,
        )
        .unwrap();
        let snapshot: ObservatorySnapshot = serde_json::from_str(&encoded).unwrap();
        validate_snapshot(&snapshot).unwrap();
        let advertised = &prepared.advertisement.bases[0];
        let reported = snapshot
            .bases
            .iter()
            .find(|base| base.base_id == advertised.base_id)
            .unwrap();
        assert_eq!(
            reported.provider_instance_id,
            advertised.provider_instance_id
        );
        assert_eq!(reported.provider_generation, advertised.provider_generation);
        assert_eq!(
            reported.implementation_id.as_ref(),
            Some(&advertised.implementation_id)
        );
        assert_eq!(
            reported.enforcement_class,
            Some(advertised.enforcement_class)
        );
        assert_eq!(reported.lifecycle, Some(advertised.lifecycle));
    }
}
