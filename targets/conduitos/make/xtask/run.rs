use std::{
    fs,
    process::{Command, Stdio},
};

use crate::cli::GlobalOpts;

use super::{
    hid_qmp, hid_run, image, keyboard_run, keyboard_text_run,
    profile::{Paths, EXPECTED_QEMU_SUCCESS, LIMINE_VERSION, QEMU_PROFILE},
    report::{
        GuestBootSign, GuestKernelSign, GuestPcSpeakerSign, GuestPresentationSign, GuestRun,
        GuestXhciSign,
    },
    usb_run, ConduitosArch, ConduitosError,
};

pub fn execute(arch: ConduitosArch, opts: &GlobalOpts) -> Result<GuestRun, ConduitosError> {
    let paths = Paths::new(arch)?;
    let _image = image::execute_proof(arch, opts)?;
    if opts.dry_run {
        println!("qemu-system-x86_64 {QEMU_PROFILE}");
        return Err(ConduitosError::refusal(
            "dry-run-has-no-boot-sign",
            "run/prove dry-run cannot manufacture execution evidence",
        ));
    }
    boot_once(&paths, opts)
}

mod boot;
mod monitor_socket;
pub(super) use boot::{
    boot_configuration, boot_once, boot_once_with_audio, inspect_wav, QemuWavCapture,
};

fn validate_pc_speaker(
    boot: &GuestBootSign,
    sign: &GuestPcSpeakerSign,
) -> Result<(), ConduitosError> {
    let exact_id =
        |value: &str| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if sign.schema != "conduit.conduitos.pc-speaker-tone/v1"
        || sign.status != "completed"
        || sign.proof_class != "freestanding-emulator"
        || sign.host_id != boot.host_id
        || sign.boot_id != boot.boot_id
        || !exact_id(&sign.base_id)
        || sign.kind != conduit_semantic_catalog::SOUND_TONE_PLAY_KIND
        || sign.implementation != conduitos::pc_speaker_offer::PC_SPEAKER_IMPLEMENTATION
        || sign.execution_profile != conduitos::pc_speaker_offer::PC_SPEAKER_EXECUTION_PROFILE
        || !exact_id(&sign.plan_id)
        || !exact_id(&sign.active_play_id)
        || sign.node_count != 2
        || sign.cord_count != 1
        || sign.requested_millihertz != [440_000, 440_000, 660_000, 660_000]
        || sign.realized_millihertz != [439_963, 0, 659_945, 0]
        || sign.divisors != [2_712, 0, 1_808, 0]
        || sign.gate_transitions != [true, false, true, false]
        || sign.transition_count != 4
        || sign.kernel_decisions == 0
        || sign.kernel_signs == 0
        || sign.final_gate_open
        || !sign.bounded
        || !sign.completed
    {
        return Err(ConduitosError::refusal(
            "invalid-pc-speaker-sign",
            format!("PC-speaker Sign failed exact validation: {sign:?}"),
        ));
    }
    Ok(())
}

fn validate_presentation(
    boot: &GuestBootSign,
    sign: &GuestPresentationSign,
) -> Result<(), ConduitosError> {
    let exact_id =
        |value: &str| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if sign.schema != "conduit.conduitos.framebuffer-presentation/v1"
        || sign.status != "completed"
        || sign.proof_class != "freestanding-emulator"
        || sign.realization != "recursive"
        || sign.back_kind != conduit_semantic_catalog::PATCHBAY_GEAR_FACE_KIND
        || sign.back_contract_revision != conduit_semantic_catalog::PATCHBAY_PRESENTATION_REVISION
        || sign.back_invocation_path != "conduitos-gear-front/front"
        || !exact_id(&sign.back_source_document_id)
        || !exact_id(&sign.back_checked_plot_id)
        || sign.host_id != boot.host_id
        || sign.boot_id != boot.boot_id
        || !exact_id(&sign.display_base_id)
        || sign.display_width < 320
        || sign.display_height < 200
        || sign.display_pitch < sign.display_width.saturating_mul(4)
        || sign.display_bits_per_pixel != 32
        || sign.execution_profile != conduitos::presentation_nucleus::CONDUITOS_PRESENTATION_PROFILE
        || sign.artifact != conduitos::presentation_nucleus::CONDUITOS_PRESENTATION_ARTIFACT
        || !exact_id(&sign.source_document_id)
        || !exact_id(&sign.checked_plot_id)
        || !exact_id(&sign.expanded_plot_id)
        || !exact_id(&sign.plan_id)
        || !exact_id(&sign.fragment_id)
        || sign.node_count != 11
        || sign.cord_count != 8
        || sign.text != "Gear Front"
        || sign.layout_children != 3
        || sign.graphics_commands != 3
        || sign.text_commands != 1
        || sign.text_pixels_written == 0
        || sign.graphics_pixels_written == 0
        || sign.kernel_signs == 0
        || !sign.bounded
        || !sign.completed
    {
        return Err(ConduitosError::refusal(
            "invalid-presentation-sign",
            format!("presentation Sign failed exact validation: {sign:?}"),
        ));
    }
    Ok(())
}

pub(super) fn prove_xhci_absent(paths: &Paths) -> Result<String, ConduitosError> {
    let output = Command::new("qemu-system-x86_64")
        .args([
            "-M",
            "q35",
            "-cpu",
            "max",
            "-m",
            "64M",
            "-smp",
            "1",
            "-display",
            "none",
            "-vga",
            "std",
            "-monitor",
            "none",
            "-serial",
            "stdio",
            "-no-reboot",
            "-net",
            "none",
            "-rtc",
            "base=2026-08-09T00:00:00,clock=vm",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-cdrom",
            paths.iso.to_str().unwrap(),
            "-boot",
            "d",
        ])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;
    let serial = String::from_utf8(output.stdout)
        .map_err(|error| ConduitosError::refusal("malformed-xhci-refusal", error.to_string()))?;
    let expected = "\"status\":\"refused\",\"reason\":\"xhci-controller-absent\"";
    if output.status.code() != Some(35)
        || !serial.contains(expected)
        || serial.contains("CONDUIT_XHCI_SIGN")
    {
        return Err(ConduitosError::refusal(
            "xhci-absence-not-refused",
            format!("status {}; serial {serial}", output.status),
        ));
    }
    Ok("xhci-controller-absent".to_owned())
}

pub(super) fn validate_xhci(
    boot: &GuestBootSign,
    sign: &GuestXhciSign,
) -> Result<(), ConduitosError> {
    if sign.schema != "conduit.conduitos.xhci-base/v1"
        || sign.status != "ready"
        || sign.proof_class != "freestanding-emulator"
        || sign.base_id.len() != 64
        || sign.boot_id != boot.boot_id
        || sign.segment != 0
        || sign.vendor != 0x1b36
        || sign.device_id != 0x000d
        || sign.bar_physical == 0
        || sign.hardware_slots < sign.admitted_slots
        || sign.admitted_slots != 3
        || sign.command_trbs != 16
        || sign.event_trbs != 16
        || sign.dma_bytes != 640
        || sign.dma_alignment != 64
        || sign.maximum_pending_commands != 1
        || sign.poll_steps == 0
        || sign.sign_slots != 8
        || sign.semantic_keyboard_offer
    {
        return Err(ConduitosError::refusal(
            "invalid-xhci-sign",
            format!("xHCI Sign failed exact validation: {sign:?}"),
        ));
    }
    Ok(())
}

mod boot_identity;
use boot_identity::validate_boot;

fn validate_kernel(boot: &GuestBootSign, sign: &GuestKernelSign) -> Result<(), ConduitosError> {
    let valid_base_ids = sign.base_ids.len() == 7
        && sign.base_ids.iter().enumerate().all(|(index, id)| {
            id.len() == 64
                && id.bytes().all(|byte| byte.is_ascii_hexdigit())
                && !sign.base_ids[..index].contains(id)
        });
    let exact_id =
        |value: &str| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if sign.schema != "conduit.conduitos.kernel-sign/v2"
        || sign.status != "accepted"
        || sign.arch != "x86_64"
        || sign.build_id != boot.build_id
        || sign.kernel != "conduit-kernel"
        || sign.scheduler_profile != "conduitos/two-lane-cooperative@1"
        || sign.host_id != boot.host_id
        || sign.boot_id != boot.boot_id
        || sign.pipeline != "check-plan-lower-kernel"
        || !exact_id(&sign.source_document_id)
        || !exact_id(&sign.checked_plot_id)
        || !exact_id(&sign.expanded_plot_id)
        || !exact_id(&sign.plan_id)
        || !exact_id(&sign.fragment_id)
        || !exact_id(&sign.active_play_id)
        || sign.planned_sign_items == 0
        || sign.planned_sign_bytes == 0
        || sign.cord_item_capacity != 3
        || sign.cord_byte_capacity != 192
        || sign.semantic_result != "HELLO, CONDUITOS"
        || sign.allocation_before_play == 0
        || sign.allocation_before_play != sign.allocation_after_play
        || sign.allocation_capacity != boot.runtime_arena_bytes as usize
        || !sign.allocation_stable_during_play
        || sign.base_count != 7
        || !valid_base_ids
        || sign.memory_arena_bytes != boot.runtime_arena_bytes
        || sign.execution_regions != 2
        || sign.execution_lanes != 2
        || sign.region_ids != ["region/text", "region/timer"]
        || sign.lane_resource_ids.len() != 2
        || sign.lane_resource_ids[0] == sign.lane_resource_ids[1]
        || sign.lane_resource_ids.iter().any(|id| id.is_empty())
        || !exact_id(&sign.lane_base_id)
        || sign.timer_slots != 1
        || sign.serial_slots != 2
        || sign.serial_maximum_bytes != 256
        || sign.interrupt_fact_slots != 4
        || sign.sign_item_slots != 64
        || sign.logical_operations != 5
        || sign.kernel_decisions == 0
        || sign.kernel_signs == 0
        || sign.timer_irq_wakes != 1
        || sign.serial_presentations != 2
        || !sign.clock_monotonic
        || sign.pending_host_calls != 0
        || !sign.overlap_witness
        || !sign.timer_pending_during_text_progress
        || sign.physical_parallelism
        || sign.preemption
        || sign.isolation
        || !sign.sse2
    {
        return Err(ConduitosError::refusal(
            "invalid-kernel-sign",
            format!("kernel Sign failed exact validation: {sign:?}"),
        ));
    }
    Ok(())
}

mod observatory;
use observatory::validate_observatory;
