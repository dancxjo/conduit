//! Bounded structured boot Sign formatting.

use core::fmt::{self, Write};

#[cfg(any(test, target_arch = "x86_64"))]
use crate::{boot::BootRecord, fabrication::FabricationRecord};
use crate::{
    composition::MachineProof,
    dual_region_plan::PreparedDualRegionPlay,
    identity::BootIdentities,
    offer::{HostOffer, SERIAL_MAXIMUM_BYTES},
};

pub const BOOT_SIGN_SCHEMA: &str = "conduit.conduitos.boot-sign/v1";
pub const MAX_BOOT_SIGN_BYTES: usize = 1024;
pub const MACHINE_SIGN_SCHEMA: &str = "conduit.conduitos.kernel-sign/v2";
pub const MAX_STRUCTURED_SIGN_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocationProof {
    pub before_play: usize,
    pub after_play: usize,
    pub capacity: usize,
}

pub struct FixedText {
    bytes: [u8; MAX_STRUCTURED_SIGN_BYTES],
    len: usize,
}

impl FixedText {
    pub const fn new() -> Self {
        Self {
            bytes: [0; MAX_STRUCTURED_SIGN_BYTES],
            len: 0,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

impl Default for FixedText {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for FixedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.len.checked_add(value.len()).ok_or(fmt::Error)?;
        let target = self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?;
        target.copy_from_slice(value.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[cfg(any(test, target_arch = "x86_64"))]
pub fn accepted(
    record: &BootRecord,
    identities: &BootIdentities,
    fabrication: &FabricationRecord,
    offer_generation: u64,
) -> Result<FixedText, fmt::Error> {
    let mut output = FixedText::new();
    write!(
        output,
        "CONDUIT_BOOT_SIGN {{\"schema\":\"{BOOT_SIGN_SCHEMA}\",\"status\":\"accepted\",\"arch\":\"{}\",\"firmware\":\"{}\",\"profile_id\":\"{}\",\"build_id\":\"{}\",\"image_binding\":\"{}\",\"offer_generation\":{},\"limine\":\"12.5.2\",\"qemu_profile\":\"q35-single-cpu-64m-headless-xhci-usb-kbd-adlib\",\"host_id\":\"",
        crate::arch::ARCHITECTURE,
        record.firmware.as_str(),
        fabrication.profile_id,
        fabrication.build_id,
        fabrication.image_binding,
        offer_generation,
    )?;
    write_hex(&mut output, &identities.host)?;
    output.write_str("\",\"boot_id\":\"")?;
    write_hex(&mut output, &identities.boot)?;
    writeln!(
        output,
        "\",\"memory_regions\":{},\"artifacts\":{},\"framebuffers\":{},\"command_line_bytes\":{},\"runtime_arena_bytes\":{}}}",
        record.memory_region_count,
        record.artifact_count,
        record.framebuffer_count,
        record.command_line_bytes,
        record.runtime_arena.length,
    )?;
    Ok(output)
}

pub fn refused(reason: &str) -> Result<FixedText, fmt::Error> {
    let mut output = FixedText::new();
    writeln!(
        output,
        "CONDUIT_BOOT_SIGN {{\"schema\":\"{BOOT_SIGN_SCHEMA}\",\"status\":\"refused\",\"reason\":\"{reason}\"}}"
    )?;
    Ok(output)
}

pub fn machine_accepted(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    report: &MachineProof,
    prepared: &PreparedDualRegionPlay,
    allocation: AllocationProof,
    build_id: &str,
) -> Result<FixedText, fmt::Error> {
    let [text_region, timer_region] = prepared.plan.fragments[0].execution_regions.as_slice()
    else {
        return Err(fmt::Error);
    };
    let mut output = FixedText::new();
    write!(
        output,
        "CONDUIT_KERNEL_SIGN {{\"schema\":\"{MACHINE_SIGN_SCHEMA}\",\"status\":\"accepted\",\"arch\":\"{}\",\"build_id\":\"{}\",\"kernel\":\"conduit-kernel\",\"scheduler_profile\":\"{}\",\"host_id\":\"",
        crate::arch::ARCHITECTURE,
        build_id,
        offer.profile,
    )?;
    write_hex(&mut output, &identities.host)?;
    output.write_str("\",\"boot_id\":\"")?;
    write_hex(&mut output, &identities.boot)?;
    write!(
        output,
        "\",\"pipeline\":\"check-plan-lower-kernel\",\"source_document_id\":\"{}\",\"checked_form_id\":\"{}\",\"expanded_form_id\":\"{}\",\"plan_id\":\"{}\",\"fragment_id\":\"{}\",\"active_play_id\":\"{}\",\"planned_sign_items\":{},\"planned_sign_bytes\":{},\"cord_item_capacity\":3,\"cord_byte_capacity\":{},\"semantic_result\":\"{}\",\"allocation_before_play\":{},\"allocation_after_play\":{},\"allocation_capacity\":{},\"allocation_stable_during_play\":{},\"base_ids\":[",
        prepared.source_document_id.as_str(),
        prepared.checked_form_id.as_str(),
        prepared.expanded_form_id.as_str(),
        prepared.plan_id.as_str(),
        prepared.fragment_id.as_str(),
        prepared.active_play.active_play_id.as_str(),
        prepared.planned_sign_items,
        prepared.planned_sign_bytes,
        64 * 3,
        crate::dual_region_plan::TEXT_RESULT,
        allocation.before_play,
        allocation.after_play,
        allocation.capacity,
        allocation.before_play == allocation.after_play,
    )?;
    for (index, base) in offer.bases.iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        output.write_char('"')?;
        write_hex(&mut output, &base.id)?;
        output.write_char('"')?;
    }
    writeln!(
        output,
        "],\"base_count\":{},\"memory_arena_bytes\":{},\"execution_regions\":2,\"execution_lanes\":2,\"region_ids\":[\"region/text\",\"region/timer\"],\"lane_resource_ids\":[\"{}\",\"{}\"],\"lane_base_id\":\"{}\",\"timer_slots\":1,\"serial_slots\":2,\"serial_maximum_bytes\":{},\"interrupt_fact_slots\":{},\"sign_item_slots\":{},\"logical_operations\":{},\"kernel_decisions\":{},\"kernel_signs\":{},\"timer_irq_wakes\":{},\"idle_entries\":{},\"serial_presentations\":{},\"clock_monotonic\":{},\"pending_host_operations\":{},\"overlap_witness\":{},\"timer_pending_during_text_progress\":{},\"physical_parallelism\":{},\"preemption\":false,\"isolation\":false,\"sse2\":{},\"rdrand\":{},\"invariant_tsc\":{}}}",
        offer.bases.len(),
        offer.runtime_arena_bytes,
        text_region.lane_resource.pool_id.as_str(),
        timer_region.lane_resource.pool_id.as_str(),
        text_region.lane_base_id.as_str(),
        SERIAL_MAXIMUM_BYTES,
        offer.interrupt_fact_capacity,
        offer.sign_item_capacity,
        report.logical_operations,
        report.decisions,
        report.kernel_signs,
        report.timer_irq_wakes,
        report.idle_entries,
        report.serial_presentations,
        report.clock_monotonic,
        report.pending_host_operations,
        report.overlap_witness,
        report.timer_pending_during_text_progress,
        report.physical_parallelism,
        offer.cpu_features.sse2,
        offer.cpu_features.rdrand,
        offer.cpu_features.invariant_tsc,
    )?;
    Ok(output)
}

pub fn machine_refused(reason: &str) -> Result<FixedText, fmt::Error> {
    let mut output = FixedText::new();
    writeln!(
        output,
        "CONDUIT_KERNEL_SIGN {{\"schema\":\"{MACHINE_SIGN_SCHEMA}\",\"status\":\"refused\",\"reason\":\"{reason}\"}}"
    )?;
    Ok(output)
}

fn write_hex(output: &mut FixedText, bytes: &[u8]) -> fmt::Result {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.write_char(HEX[(byte >> 4) as usize] as char)?;
        output.write_char(HEX[(byte & 0x0f) as usize] as char)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot::{Firmware, RuntimeArena};

    #[test]
    fn accepted_sign_is_bounded_and_machine_readable() {
        let record = BootRecord {
            firmware: Firmware::Uefi64,
            timestamp: 1,
            hhdm_offset: 2,
            image_physical_start: 3,
            image_length: 4,
            memory_region_count: 5,
            artifact_count: 0,
            framebuffer_count: 0,
            command_line_bytes: 0,
            runtime_arena: RuntimeArena {
                physical_start: 6,
                length: 262_144,
            },
        };
        let fabrication = FabricationRecord {
            schema: crate::fabrication::FABRICATION_SCHEMA,
            profile_id: "sha256:profile",
            build_id: "build:sha256:build",
            image_binding: "image:sha256:binding",
            target: "conduitos/x86_64/pc",
            implementations: crate::fabrication::ALL_KNOWN_IMPLEMENTATIONS,
            facilities: crate::fabrication::FACILITY_NATIVE_COMPOSITOR,
            resources: crate::fabrication::RESOURCE_PRESENTATION_SURFACE,
            bases: crate::fabrication::BASE_DISPLAY_SCANOUT,
            drivers: crate::fabrication::DRIVER_LINEAR_FRAMEBUFFER,
            presenters: crate::fabrication::PRESENTER_NATIVE_GRAPHICAL,
            proof_instrumentation: 0,
            presentation_surface_slots: 2,
            presentation_surface_bytes: 4 * 1024 * 1024,
            runtime_arena_ceiling: 262_144,
            operation_slot_ceiling: 64,
            timer_slot_ceiling: 32,
            evidence_item_ceiling: 64,
        };
        let output = accepted(
            &record,
            &BootIdentities {
                host: [0xaa; 32],
                boot: [0xbb; 32],
            },
            &fabrication,
            1,
        )
        .unwrap();
        let text = core::str::from_utf8(output.as_bytes()).unwrap();
        assert!(text.contains("\"status\":\"accepted\""));
        assert!(text.contains(&"aa".repeat(32)));
        assert!(text.len() <= MAX_BOOT_SIGN_BYTES);
    }

    #[test]
    fn machine_sign_binds_finite_bases_and_kernel_ownership() {
        let identities = BootIdentities {
            host: [0xaa; 32],
            boot: [0xbb; 32],
        };
        let offer = HostOffer::new(
            &identities,
            "build",
            crate::offer::CpuFeatures {
                sse2: true,
                rdrand: false,
                invariant_tsc: true,
            },
            262_144,
        );
        let output = machine_accepted(
            &identities,
            &offer,
            &MachineProof {
                logical_operations: 3,
                decisions: 6,
                kernel_signs: 12,
                timer_irq_wakes: 1,
                idle_entries: 1,
                serial_presentations: 1,
                clock_monotonic: true,
                pending_host_operations: 0,
                overlap_witness: true,
                timer_pending_during_text_progress: true,
                physical_parallelism: false,
            },
            &crate::dual_region_plan::prepare(&identities, &offer, "build").unwrap(),
            AllocationProof {
                before_play: 1024,
                after_play: 1024,
                capacity: 262_144,
            },
            "build",
        )
        .unwrap();
        let text = core::str::from_utf8(output.as_bytes()).unwrap();
        assert!(text.starts_with("CONDUIT_KERNEL_SIGN "));
        assert!(text.contains("\"kernel\":\"conduit-kernel\""));
        assert!(text.contains("\"base_count\":7"));
        assert!(text.contains("\"memory_arena_bytes\":262144"));
        assert!(text.contains("\"pipeline\":\"check-plan-lower-kernel\""));
        assert!(text.len() <= MAX_STRUCTURED_SIGN_BYTES);
    }
}
