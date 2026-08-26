use std::{fs::File, io::Read, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::ConduitosError;

#[derive(Debug, Serialize)]
pub struct BuildRecord {
    pub schema: &'static str,
    pub base_commit: String,
    pub architecture: &'static str,
    pub rust_target: &'static str,
    pub limine_crate: &'static str,
    pub elf_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct ImageRecord {
    pub schema: &'static str,
    pub architecture: &'static str,
    pub limine_version: &'static str,
    pub limine_archive_sha256: &'static str,
    pub iso_sha256: String,
    pub file_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GuestBootSign {
    pub schema: String,
    pub status: String,
    pub arch: String,
    pub firmware: String,
    pub profile_id: String,
    pub build_id: String,
    pub image_binding: String,
    pub offer_generation: u64,
    pub limine: String,
    pub qemu_profile: String,
    pub host_id: String,
    pub boot_id: String,
    pub memory_regions: u16,
    pub artifacts: u16,
    pub framebuffers: u8,
    pub command_line_bytes: u16,
    pub runtime_arena_bytes: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GuestKernelSign {
    pub schema: String,
    pub status: String,
    pub arch: String,
    pub build_id: String,
    pub kernel: String,
    pub scheduler_profile: String,
    pub host_id: String,
    pub boot_id: String,
    pub pipeline: String,
    pub source_document_id: String,
    pub checked_form_id: String,
    pub expanded_form_id: String,
    pub plan_id: String,
    pub fragment_id: String,
    pub active_play_id: String,
    pub planned_sign_items: u16,
    pub planned_sign_bytes: u32,
    pub cord_item_capacity: u16,
    pub cord_byte_capacity: u32,
    pub semantic_result: String,
    pub allocation_before_play: usize,
    pub allocation_after_play: usize,
    pub allocation_capacity: usize,
    pub allocation_stable_during_play: bool,
    pub base_ids: Vec<String>,
    pub base_count: usize,
    pub memory_arena_bytes: u64,
    pub execution_regions: u8,
    pub execution_lanes: u8,
    pub region_ids: Vec<String>,
    pub lane_resource_ids: Vec<String>,
    pub lane_base_id: String,
    pub timer_slots: u16,
    pub serial_slots: u16,
    pub serial_maximum_bytes: u32,
    pub interrupt_fact_slots: u16,
    pub sign_item_slots: u16,
    pub logical_operations: u8,
    pub kernel_decisions: u32,
    pub kernel_signs: u16,
    pub timer_irq_wakes: u32,
    pub idle_entries: u32,
    pub serial_presentations: u32,
    pub clock_monotonic: bool,
    pub pending_host_operations: u8,
    pub overlap_witness: bool,
    pub timer_pending_during_text_progress: bool,
    pub physical_parallelism: bool,
    pub preemption: bool,
    pub isolation: bool,
    pub sse2: bool,
    pub rdrand: bool,
    pub invariant_tsc: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GuestPresentationSign {
    pub schema: String,
    pub status: String,
    pub proof_class: String,
    pub realization: String,
    pub back_kind: String,
    pub back_contract_revision: String,
    pub back_invocation_path: String,
    pub back_source_document_id: String,
    pub back_checked_form_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub display_base_id: String,
    pub display_width: u32,
    pub display_height: u32,
    pub display_pitch: u32,
    pub display_bits_per_pixel: u8,
    pub execution_profile: String,
    pub artifact: String,
    pub source_document_id: String,
    pub checked_form_id: String,
    pub expanded_form_id: String,
    pub plan_id: String,
    pub fragment_id: String,
    pub node_count: u8,
    pub cord_count: u8,
    pub text: String,
    pub layout_children: u8,
    pub graphics_commands: u8,
    pub text_commands: u8,
    pub text_pixels_written: u32,
    pub graphics_pixels_written: u32,
    pub kernel_signs: u16,
    pub bounded: bool,
    pub completed: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GuestPcSpeakerSign {
    pub schema: String,
    pub status: String,
    pub proof_class: String,
    pub host_id: String,
    pub boot_id: String,
    pub base_id: String,
    pub kind: String,
    pub implementation: String,
    pub execution_profile: String,
    pub plan_id: String,
    pub active_play_id: String,
    pub node_count: usize,
    pub cord_count: usize,
    pub requested_millihertz: Vec<u64>,
    pub realized_millihertz: Vec<u64>,
    pub divisors: Vec<u16>,
    pub gate_transitions: Vec<bool>,
    pub transition_count: u32,
    pub kernel_decisions: u32,
    pub kernel_signs: u16,
    pub final_gate_open: bool,
    pub bounded: bool,
    pub completed: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GuestXhciSign {
    pub schema: String,
    pub status: String,
    pub proof_class: String,
    pub base_id: String,
    pub boot_id: String,
    pub segment: u8,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor: u16,
    pub device_id: u16,
    pub bar_physical: u64,
    pub hardware_slots: u8,
    pub admitted_slots: u8,
    pub command_trbs: u8,
    pub event_trbs: u8,
    pub dma_bytes: u16,
    pub dma_alignment: u16,
    pub maximum_pending_commands: u8,
    pub poll_steps: u32,
    pub sign_slots: u8,
    pub semantic_keyboard_offer: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GuestUsbSign {
    pub schema: String,
    pub status: String,
    pub proof_class: String,
    pub controller_base_id: String,
    pub boot_id: String,
    pub device_instance_id: String,
    pub root_port: u8,
    pub slot: u8,
    pub address: u8,
    pub attachment_epoch: u32,
    pub usb_version: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
    pub ep0_maximum_packet_size: u16,
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_version: u16,
    pub configuration_value: u8,
    pub configuration_bytes: u16,
    pub descriptor_records: u8,
    pub interface_count: u8,
    pub endpoint_count: u8,
    pub first_interface_id: String,
    pub first_interface_number: u8,
    pub first_interface_alternate: u8,
    pub first_interface_class: u8,
    pub first_interface_subclass: u8,
    pub first_interface_protocol: u8,
    pub first_endpoint_id: String,
    pub first_endpoint_address: u8,
    pub first_endpoint_direction_in: bool,
    pub first_endpoint_transfer_type: u8,
    pub first_endpoint_maximum_packet_size: u16,
    pub first_endpoint_interval: u8,
    pub configuration_limit_bytes: u16,
    pub interface_limit: u8,
    pub endpoint_limit: u8,
    pub descriptor_record_limit: u8,
    pub outstanding_control_transfer_limit: u8,
    pub enumeration_retries: u8,
    pub control_transfers: u8,
    pub short_packets: u8,
    pub transfer_trbs: u8,
    pub dma_bytes: u16,
    pub dma_alignment: u16,
    pub port_poll_steps: u32,
    pub sign_slots: u8,
    pub semantic_keyboard_offer: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GuestHidSign {
    pub schema: String,
    pub status: String,
    pub proof_class: String,
    pub controller_base_id: String,
    pub boot_id: String,
    pub device_instance_id: String,
    pub interface_id: String,
    pub endpoint_id: String,
    pub interface_number: u8,
    pub endpoint_address: u8,
    pub endpoint_dci: u8,
    pub endpoint_maximum_packet_size: u16,
    pub endpoint_interval: u8,
    pub set_protocol_transfers: u8,
    pub interrupt_transfers: u8,
    pub report_bytes: u8,
    pub report_buffers: u8,
    pub maximum_outstanding_interrupt_transfers: u8,
    pub maximum_transitions_per_report: u8,
    pub transfer_trbs: u8,
    pub dma_bytes: u16,
    pub dma_alignment: u16,
    pub sign_slots: u8,
    pub interrupt_poll_windows: u16,
    pub transition_count: u8,
    pub first_usage_page: String,
    pub first_usage: u8,
    pub first_state: String,
    pub first_modifiers: u8,
    pub second_usage_page: String,
    pub second_usage: u8,
    pub second_state: String,
    pub second_modifiers: u8,
    pub layout_translation: bool,
    pub unicode_translation: bool,
    pub semantic_keyboard_offer: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GuestKeyboardSign {
    pub schema: String,
    pub status: String,
    pub proof_class: String,
    pub host_id: String,
    pub boot_id: String,
    pub offer_generation: u64,
    pub kind: String,
    pub contract_revision: String,
    pub implementation: String,
    pub execution_profile: String,
    pub artifact_build: String,
    pub controller_base_id: String,
    pub device_instance_id: String,
    pub interface_id: String,
    pub endpoint_id: String,
    pub plan_id: String,
    pub active_play_id: String,
    pub resource_bindings: usize,
    pub report_buffers: u16,
    pub transition_slots: u16,
    pub operation_slots: u16,
    pub cord_item_capacity: u16,
    pub cord_byte_capacity: u32,
    pub event_count: u8,
    pub first_value: [u8; 3],
    pub second_value: [u8; 3],
    pub semantic_usb_facts: bool,
    pub layout_translation: bool,
    pub unicode_translation: bool,
    pub completed: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GuestKeyboardTextSign {
    pub schema: String,
    pub status: String,
    pub proof_class: String,
    pub source_document_id: String,
    pub checked_form_id: String,
    pub expanded_form_id: String,
    pub plan_id: String,
    pub active_play_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub form_machine_facts: bool,
    pub keymap_configuration: String,
    pub physical_transition_count: usize,
    pub presentation_fragments: Vec<String>,
    pub visible_ascii: String,
    pub bounded: bool,
    pub completed: bool,
}

#[derive(Debug, Clone)]
pub struct GuestRun {
    pub boot: GuestBootSign,
    pub presentation: GuestPresentationSign,
    pub pc_speaker: GuestPcSpeakerSign,
    pub xhci: GuestXhciSign,
    pub usb: GuestUsbSign,
    pub hid: GuestHidSign,
    pub keyboard: GuestKeyboardSign,
    pub keyboard_text: GuestKeyboardTextSign,
    pub keyboard_text_observatory: conduit_observatory::ObservatorySnapshot,
    pub kernel: GuestKernelSign,
    pub observatory: conduit_observatory::ObservatorySnapshot,
    pub serial: String,
}

#[derive(Debug, Serialize)]
pub struct ProofRecord {
    pub schema: &'static str,
    pub base_commit: String,
    pub architecture: &'static str,
    pub proof_class: &'static str,
    pub limine_version: &'static str,
    pub limine_archive_sha256: &'static str,
    pub qemu_profile: &'static str,
    pub qemu_version: String,
    pub iso_sha256: String,
    pub reproducible_image: bool,
    pub first_boot: GuestBootSign,
    pub first_presentation: GuestPresentationSign,
    pub first_kernel: GuestKernelSign,
    pub first_observatory: conduit_observatory::ObservatorySnapshot,
    pub second_boot: GuestBootSign,
    pub second_presentation: GuestPresentationSign,
    pub second_kernel: GuestKernelSign,
    pub second_observatory: conduit_observatory::ObservatorySnapshot,
    pub fresh_host_id: bool,
    pub fresh_boot_id: bool,
    pub stable_semantic_identities: bool,
    pub fresh_realization_identities: bool,
    pub native_patchbay_consumed: bool,
    pub native_patchbay_linear_lines: usize,
}

pub fn sha256_file(path: &Path) -> Result<String, ConduitosError> {
    let mut file = File::open(path).map_err(|error| {
        ConduitosError::refusal(
            "artifact-unavailable",
            format!("cannot open {}: {error}", path.display()),
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            ConduitosError::refusal(
                "artifact-unavailable",
                format!("cannot read {}: {error}", path.display()),
            )
        })?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn git_head(root: &Path) -> Result<String, ConduitosError> {
    let output = super::profile::command(
        "git",
        &["rev-parse", "HEAD"],
        root,
        "base-commit-unavailable",
    )?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
