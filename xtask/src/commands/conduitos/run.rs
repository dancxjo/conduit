use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
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

pub(super) fn boot_once(paths: &Paths, opts: &GlobalOpts) -> Result<GuestRun, ConduitosError> {
    let monitor_socket = paths.target.join("hid-monitor.sock");
    let serial_path = paths.target.join("boot-serial.log");
    let _ = fs::remove_file(&monitor_socket);
    let _ = fs::remove_file(&serial_path);
    let monitor = format!(
        "unix:{},server=on,wait=off",
        monitor_socket.to_string_lossy()
    );
    let serial_target = format!("file:{}", serial_path.to_string_lossy());
    let mut child = Command::new("qemu-system-x86_64")
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
            "-qmp",
            &monitor,
            "-serial",
            &serial_target,
            "-no-reboot",
            "-net",
            "none",
            "-rtc",
            "base=2026-08-09T00:00:00,clock=vm",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-device",
            "qemu-xhci,id=conduitos-xhci,p2=1,p3=0",
            "-device",
            "usb-kbd,bus=conduitos-xhci.0,port=1",
            "-audiodev",
            "none,id=conduitos-opl2-audio",
            "-device",
            "adlib,audiodev=conduitos-opl2-audio",
            "-cdrom",
            paths.iso.to_str().unwrap(),
            "-boot",
            "d",
        ])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            ConduitosError::refusal(
                "missing-qemu",
                format!("cannot launch qemu-system-x86_64: {error}"),
            )
        })?;
    hid_qmp::inject(&monitor_socket, &serial_path, &mut child)?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        match child.try_wait().map_err(|error| {
            ConduitosError::refusal("qemu-boot-failed", format!("cannot wait for QEMU: {error}"))
        })? {
            Some(status) => break status,
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                child.kill().map_err(|error| {
                    ConduitosError::refusal(
                        "qemu-timeout",
                        format!("cannot stop timed-out QEMU: {error}"),
                    )
                })?;
                let _ = child.wait();
                return Err(ConduitosError::refusal(
                    "qemu-timeout",
                    "QEMU did not emit a terminal debug-exit within 20 seconds",
                ));
            }
        }
    };
    let _output = child.wait_with_output().map_err(|error| {
        ConduitosError::refusal(
            "qemu-boot-failed",
            format!("cannot collect QEMU output: {error}"),
        )
    })?;
    let _ = fs::remove_file(&monitor_socket);
    if status.code() != Some(EXPECTED_QEMU_SUCCESS) {
        let serial = fs::read_to_string(&serial_path).unwrap_or_default();
        let desired_tail_start = serial.len().saturating_sub(360);
        let tail_start = serial
            .char_indices()
            .find_map(|(index, _)| (index >= desired_tail_start).then_some(index))
            .unwrap_or(0);
        return Err(ConduitosError::refusal(
            "qemu-boot-failed",
            format!(
                "expected isa-debug-exit status {EXPECTED_QEMU_SUCCESS}, got {}; serial tail: {}",
                status,
                &serial[tail_start..]
            ),
        ));
    }
    let serial = fs::read_to_string(&serial_path).map_err(|error| {
        ConduitosError::refusal(
            "malformed-boot-sign",
            format!("cannot read serial: {error}"),
        )
    })?;
    let signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_BOOT_SIGN "))
        .collect();
    let xhci_signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_XHCI_SIGN "))
        .collect();
    if xhci_signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-xhci-sign",
            format!(
                "expected one structured xHCI Sign, found {}",
                xhci_signs.len()
            ),
        ));
    }
    if signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-boot-sign",
            format!("expected one structured boot Sign, found {}", signs.len()),
        ));
    }
    let kernel_signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_KERNEL_SIGN "))
        .collect();
    let presentation_signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_PRESENTATION_SIGN "))
        .collect();
    let pc_speaker_signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_PC_SPEAKER_SIGN "))
        .collect();
    if presentation_signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-presentation-sign",
            format!(
                "expected one structured presentation Sign, found {}",
                presentation_signs.len()
            ),
        ));
    }
    if pc_speaker_signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-pc-speaker-sign",
            format!(
                "expected one structured PC-speaker Sign, found {}",
                pc_speaker_signs.len()
            ),
        ));
    }
    if kernel_signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-kernel-sign",
            format!(
                "expected one structured kernel Sign, found {}",
                kernel_signs.len()
            ),
        ));
    }
    let observatory_snapshots: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_OBSERVATORY_SNAPSHOT "))
        .collect();
    let keyboard_text_observatory_snapshots: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix(conduitos::keyboard_text_observatory::EXPORT_PREFIX))
        .collect();
    let presentations: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_SERIAL_PRESENT "))
        .collect();
    if presentations.len() != 2
        || presentations[0] != "HELLO, CONDUITOS"
        || presentations[1].as_bytes() != [0; conduit_std_catalog::TICK_ENCODED_LEN as usize]
    {
        return Err(ConduitosError::refusal(
            "invalid-serial-presentation",
            format!("expected exact bounded text presentation, found {presentations:?}"),
        ));
    }
    if observatory_snapshots.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-observatory-snapshot",
            format!(
                "expected one ordinary Observatory snapshot, found {}",
                observatory_snapshots.len()
            ),
        ));
    }
    if keyboard_text_observatory_snapshots.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-keyboard-text-observatory",
            format!(
                "expected one keyboard-text Observatory snapshot, found {}",
                keyboard_text_observatory_snapshots.len()
            ),
        ));
    }
    let boot: GuestBootSign = serde_json::from_str(signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-boot-sign", error.to_string()))?;
    let xhci: GuestXhciSign = serde_json::from_str(xhci_signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-xhci-sign", error.to_string()))?;
    let usb = usb_run::extract(&serial)?;
    let hid = hid_run::extract(&serial)?;
    let keyboard = keyboard_run::extract(&serial)?;
    let keyboard_text = keyboard_text_run::extract(&serial)?;
    let kernel: GuestKernelSign = serde_json::from_str(kernel_signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-kernel-sign", error.to_string()))?;
    let presentation: GuestPresentationSign =
        serde_json::from_str(presentation_signs[0]).map_err(|error| {
            ConduitosError::refusal("malformed-presentation-sign", error.to_string())
        })?;
    let pc_speaker: GuestPcSpeakerSign = serde_json::from_str(pc_speaker_signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-pc-speaker-sign", error.to_string()))?;
    let observatory: conduit_observatory::ObservatorySnapshot =
        serde_json::from_str(observatory_snapshots[0]).map_err(|error| {
            ConduitosError::refusal("malformed-observatory-snapshot", error.to_string())
        })?;
    let keyboard_text_observatory: conduit_observatory::ObservatorySnapshot =
        serde_json::from_str(keyboard_text_observatory_snapshots[0]).map_err(|error| {
            ConduitosError::refusal("malformed-keyboard-text-observatory", error.to_string())
        })?;
    validate_boot(&boot)?;
    validate_presentation(&boot, &presentation)?;
    validate_pc_speaker(&boot, &pc_speaker)?;
    validate_xhci(&boot, &xhci)?;
    usb_run::validate(&boot, &xhci, &usb)?;
    hid_run::validate(&boot, &xhci, &usb, &hid)?;
    keyboard_run::validate(&boot, &xhci, &usb, &hid, &keyboard, &observatory)?;
    keyboard_text_run::validate(
        &serial,
        &boot,
        &keyboard,
        &keyboard_text,
        &keyboard_text_observatory,
    )?;
    validate_kernel(&boot, &kernel)?;
    validate_observatory(&boot, &kernel, &presentation, &observatory)?;
    if !opts.quiet && !opts.json {
        println!("{}", signs[0]);
        println!("{}", kernel_signs[0]);
        println!("{}", observatory_snapshots[0]);
    }
    Ok(GuestRun {
        boot,
        presentation,
        pc_speaker,
        xhci,
        usb,
        hid,
        keyboard,
        keyboard_text,
        keyboard_text_observatory,
        kernel,
        observatory,
        serial,
    })
}

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
        || sign.kind != conduit_std_catalog::SOUND_TONE_PLAY_KIND
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
        || sign.back_kind != conduit_std_catalog::PATCHBAY_GEAR_FACE_KIND
        || sign.back_contract_revision != conduit_std_catalog::PATCHBAY_PRESENTATION_REVISION
        || sign.back_invocation_path != "conduitos-gear-face/face"
        || !exact_id(&sign.back_source_document_id)
        || !exact_id(&sign.back_checked_form_id)
        || sign.host_id != boot.host_id
        || sign.boot_id != boot.boot_id
        || !exact_id(&sign.display_base_id)
        || sign.display_width < 320
        || sign.display_height < 200
        || sign.display_pitch < sign.display_width.saturating_mul(4)
        || sign.display_bits_per_pixel != 32
        || sign.execution_profile != conduit_std_catalog::CONDUITOS_PRESENTATION_PROFILE
        || sign.artifact != conduit_std_catalog::CONDUITOS_PRESENTATION_ARTIFACT
        || !exact_id(&sign.source_document_id)
        || !exact_id(&sign.checked_form_id)
        || !exact_id(&sign.expanded_form_id)
        || !exact_id(&sign.plan_id)
        || !exact_id(&sign.fragment_id)
        || sign.node_count != 10
        || sign.cord_count != 7
        || sign.text != "Gear Face"
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

fn validate_xhci(boot: &GuestBootSign, sign: &GuestXhciSign) -> Result<(), ConduitosError> {
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
        || sign.admitted_slots != 1
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

fn validate_boot(sign: &GuestBootSign) -> Result<(), ConduitosError> {
    if sign.schema != "conduit.conduitos.boot-sign/v1"
        || sign.status != "accepted"
        || sign.arch != "x86_64"
        || sign.limine != LIMINE_VERSION
        || sign.qemu_profile != QEMU_PROFILE
        || sign.host_id.len() != 64
        || sign.boot_id.len() != 64
        || sign.memory_regions == 0
        || sign.runtime_arena_bytes != 4_194_304
    {
        return Err(ConduitosError::refusal(
            "invalid-boot-sign",
            format!("boot Sign failed exact validation: {sign:?}"),
        ));
    }
    Ok(())
}

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
        || !exact_id(&sign.checked_form_id)
        || !exact_id(&sign.expanded_form_id)
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
        || sign.pending_host_operations != 0
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

fn validate_observatory(
    boot: &GuestBootSign,
    kernel: &GuestKernelSign,
    presentation: &GuestPresentationSign,
    snapshot: &conduit_observatory::ObservatorySnapshot,
) -> Result<(), ConduitosError> {
    use conduit_observatory::{BootProofClass, OperationalState, PlanLifecycle};

    conduit_observatory::validate_snapshot(snapshot)
        .map_err(|error| ConduitosError::refusal("invalid-observatory-snapshot", error))?;
    let host = snapshot.hosts.first();
    let plan = snapshot.plans.first();
    let play = snapshot.plays.first();
    let provenance = snapshot.sealed_boot_provenance.first();
    let expected_artifact = format!("conduitos-build/{}", boot.build_id);
    let exact_placement = |kind: &str, contract: &str, implementation: &str| {
        plan.and_then(|plan| plan.fragments.first())
            .and_then(|fragment| {
                fragment
                    .placements
                    .iter()
                    .find(|placement| placement.kind_id.as_str() == kind)
            })
            .is_some_and(|placement| {
                let runtime_memory = placement.resources.iter().any(|resource| {
                    resource.class_id.as_str() == "conduit.resource/runtime-memory@1"
                        && resource.units == 4_096
                });
                let exact_effect_realization = match kind {
                    "time/tick" => {
                        placement.resources.len() == 2
                            && placement.resources.iter().any(|resource| {
                                resource.class_id.as_str() == "conduit.resource/timer-slot@1"
                                    && resource.units == 1
                            })
                            && placement.host_operations.len() == 1
                            && placement.host_operations[0].contract_id.as_str()
                                == "conduit.host/wait@1"
                    }
                    "presentation/tick" => {
                        placement.resources.len() == 2
                            && placement.resources.iter().any(|resource| {
                                resource.class_id.as_str() == "conduit.resource/presentation-slot@1"
                                    && resource.units == 1
                            })
                            && placement.host_operations.len() == 1
                            && placement.host_operations[0].contract_id.as_str()
                                == "conduit.host/present@1"
                    }
                    "text/literal" => {
                        placement.resources.len() == 1 && placement.host_operations.is_empty()
                    }
                    "text/upper" => {
                        placement.resources.len() == 1
                            && placement.host_operations.len() == 1
                            && placement.host_operations[0].contract_id.as_str()
                                == "conduit.host/text-upper@1"
                            && placement.host_operations[0].maximum_in_flight == 1
                            && placement.host_operations[0].maximum_input_bytes == 256
                            && placement.host_operations[0].maximum_output_bytes == 256
                    }
                    "presentation/text" => {
                        placement.resources.len() == 2
                            && placement.resources.iter().any(|resource| {
                                resource.class_id.as_str() == "conduit.resource/presentation-slot@1"
                                    && resource.units == 1
                            })
                            && placement.host_operations.len() == 1
                            && placement.host_operations[0].contract_id.as_str()
                                == "conduit.host/present@1"
                            && placement.host_operations[0].maximum_in_flight == 1
                            && placement.host_operations[0].maximum_input_bytes == 256
                    }
                    _ => false,
                };
                placement.kind_contract_revision.as_str() == contract
                    && placement.execution_profile_id.as_str()
                        == "conduitos/single-lane-cooperative@1"
                    && placement.implementation_id.as_str() == implementation
                    && placement.artifact_id.as_str() == expected_artifact
                    && placement.host_id.as_str() == boot.host_id
                    && placement.boot_id.as_str() == boot.boot_id
                    && placement.offer_generation.0 == 1
                    && runtime_memory
                    && exact_effect_realization
            })
    };
    let exact_text_placements = exact_placement(
        "text/literal",
        "conduit.std/text-literal@1",
        "conduitos/kernel-text-literal@1",
    ) && exact_placement(
        "text/upper",
        "conduit.std/text-upper@1",
        "conduitos/kernel-text-upper@1",
    ) && exact_placement(
        "presentation/text",
        "conduit.std/presentation-text@1",
        "conduitos/kernel-serial-text@1",
    );
    let exact_tick_placements = exact_placement(
        "time/tick",
        "conduit.std/time-tick@2",
        "conduitos/kernel-time-tick@1",
    ) && exact_placement(
        "presentation/tick",
        "conduit.std/presentation-tick@1",
        "conduitos/kernel-serial-tick@1",
    );
    let bases_match = snapshot.bases.len() == kernel.base_ids.len() + 1
        && snapshot.bases.iter().all(|base| {
            base.host_id.as_str() == boot.host_id
                && base.boot_id.as_str() == boot.boot_id
                && base.state == OperationalState::Available
                && (kernel.base_ids.iter().any(|id| id == base.base_id.as_str())
                    || base.base_id.as_str() == presentation.display_base_id)
        });
    let exact_base = |kind: &str, capacity: u64| {
        snapshot
            .bases
            .iter()
            .any(|base| base.kind_id.as_str() == kind && base.capacity_units == capacity)
    };
    let base_inventory = exact_base("conduitos.base/memory@1", boot.runtime_arena_bytes)
        && exact_base("conduitos.base/clock@1", 1)
        && exact_base("conduitos.base/timer@1", 1)
        && exact_base("conduitos.base/serial@1", 2)
        && exact_base("conduitos.base/interrupt@1", 4)
        && exact_base("conduitos.base/idle@1", 1)
        && exact_base("conduitos.base/execution-lane@1", 2)
        && exact_base(
            "conduitos.base/framebuffer@1",
            u64::from(presentation.display_pitch) * u64::from(presentation.display_height),
        );
    let current_signs = snapshot
        .observations
        .iter()
        .filter(|sign| {
            matches!(
                sign.kind,
                conduit_core::ObservationKind::PlacementTerminal { .. }
                    | conduit_core::ObservationKind::ConnectionTerminal { .. }
                    | conduit_core::ObservationKind::PlanTerminal { .. }
            )
        })
        .count()
        == 9;
    let overlap_sign = snapshot.observations.iter().any(|sign| {
        matches!(
            &sign.kind,
            conduit_core::ObservationKind::ExecutionRegionOverlap {
                waiting_region_id,
                progressing_region_id,
                physical_parallelism: false,
            } if waiting_region_id.as_str() == "region/timer"
                && progressing_region_id.as_str() == "region/text"
        )
    });
    let historical_signs = [
        conduit_core::ObservationKind::HostStarted,
        conduit_core::ObservationKind::AdvertisementPublished,
        conduit_core::ObservationKind::PlanFragmentReceived,
        conduit_core::ObservationKind::PlanPlayStarted,
    ]
    .iter()
    .all(|kind| {
        snapshot
            .historical_observations
            .iter()
            .any(|sign| core::mem::discriminant(&sign.kind) == core::mem::discriminant(kind))
    }) && snapshot
        .historical_observations
        .iter()
        .filter(|sign| matches!(sign.kind, conduit_core::ObservationKind::PlacementPrepared))
        .count()
        == 5;
    if snapshot.hosts.len() != 1
        || !snapshot.lines.is_empty()
        || snapshot.plans.len() != 1
        || snapshot.plays.len() != 1
        || snapshot.observations.len() != 10
        || snapshot.historical_observations.len() != 9
        || snapshot.sealed_boot_provenance.len() != 1
        || !bases_match
        || !base_inventory
        || !exact_text_placements
        || !exact_tick_placements
        || !current_signs
        || !overlap_sign
        || !historical_signs
        || host.is_none_or(|host| {
            host.advertisement.host_id.as_str() != boot.host_id
                || host.advertisement.boot_id.as_str() != boot.boot_id
                || host.advertisement.profile.as_str() != kernel.scheduler_profile
                || host.advertisement.capabilities.len() != 6
                || !host.advertisement.capabilities.iter().any(|capability| {
                    capability.kind_id.as_str() == "text/upper"
                        && capability.implementation.implementation_id.as_str()
                            == "conduitos/kernel-text-upper@1"
                        && capability.limits.max_queue_items == 4
                        && capability.limits.max_queue_bytes == 256
                })
                || !host.advertisement.capabilities.iter().any(|capability| {
                    capability.kind_id.as_str() == "text/literal"
                        && capability.implementation.implementation_id.as_str()
                            == "conduitos/kernel-text-literal@1"
                        && capability.limits.max_queue_items == 4
                        && capability.limits.max_queue_bytes == 256
                })
                || !host.advertisement.capabilities.iter().any(|capability| {
                    capability.kind_id.as_str() == "presentation/text"
                        && capability.implementation.implementation_id.as_str()
                            == "conduitos/kernel-serial-text@1"
                        && capability.limits.max_queue_items == 4
                        && capability.limits.max_queue_bytes == 256
                })
                || !host.advertisement.planner_capabilities.is_empty()
                || host.advertisement.resources.len() != 12
                || host.state != OperationalState::Available
        })
        || plan.is_none_or(|plan| {
            plan.plan_id.as_str() != kernel.plan_id
                || plan.source_document_id.as_str() != kernel.source_document_id
                || plan.checked_form_id.as_str() != kernel.checked_form_id
                || plan.expanded_form_id.as_str() != kernel.expanded_form_id
                || plan.fragments.len() != 1
                || plan.fragments[0].fragment_id.as_str() != kernel.fragment_id
                || plan.fragments[0].placements.len() != 5
                || plan.fragments[0].connections.len() != 3
                || plan.fragments[0].execution_regions.len() != 2
                || plan.fragments[0].execution_regions[0].region_id.as_str() != "region/text"
                || plan.fragments[0].execution_regions[1].region_id.as_str() != "region/timer"
                || plan.fragments[0].execution_regions[0]
                    .lane_resource
                    .pool_id
                    .as_str()
                    != kernel.lane_resource_ids[0]
                || plan.fragments[0].execution_regions[1]
                    .lane_resource
                    .pool_id
                    .as_str()
                    != kernel.lane_resource_ids[1]
                || plan.fragments[0].execution_regions.iter().any(|region| {
                    region.lane_base_id.as_str() != kernel.lane_base_id
                        || region.lane_count != 1
                        || region.preemption_required
                        || region.isolation_required
                })
                || plan.fragments[0]
                    .connections
                    .iter()
                    .map(|connection| u32::from(connection.item_capacity))
                    .sum::<u32>()
                    != u32::from(kernel.cord_item_capacity)
                || plan.fragments[0]
                    .connections
                    .iter()
                    .map(|connection| connection.byte_capacity)
                    .sum::<u32>()
                    != kernel.cord_byte_capacity
        })
        || play.is_none_or(|play| {
            play.active_play_id.as_str() != kernel.active_play_id
                || play.plan_id.as_str() != kernel.plan_id
                || play.host_id.as_str() != boot.host_id
                || play.boot_id.as_str() != boot.boot_id
                || play.lifecycle != PlanLifecycle::Completed
                || play.placements.len() != 5
                || play.connections.len() != 3
                || play.connections.iter().any(|connection| {
                    connection.pressure.as_ref().is_none_or(|pressure| {
                        pressure.current_in_flight_items != Some(0)
                            || pressure.current_buffered_bytes != Some(0)
                            || pressure.pressure_events != 0
                            || pressure.last_pressure_sequence.is_some()
                    })
                })
        })
        || snapshot.retention.item_capacity != 64
        || snapshot.retention.retained_items != 19
        || snapshot.retention.dropped_items != 0
        || provenance.is_none_or(|provenance| {
            provenance.host_id.as_str() != boot.host_id
                || provenance.boot_id.as_str() != boot.boot_id
                || provenance.firmware_environment != boot.firmware
                || provenance.adapter_name != "Limine"
                || provenance.adapter_version != boot.limine
                || provenance.adapter_revision != "3"
                || provenance.image_id.as_str() != boot.image_id
                || provenance.build_id.as_str() != boot.build_id
                || provenance.memory_map.normalized_region_count != boot.memory_regions
                || provenance.memory_map.runtime_arena_bytes != boot.runtime_arena_bytes
                || !provenance.boot_artifacts.is_empty()
                || provenance.initial_plan_artifact_id.is_some()
                || provenance.recovery_plan_artifact_id.is_some()
                || provenance.framebuffers.len() != 1
                || provenance.framebuffers[0].base_id.as_str() != presentation.display_base_id
                || provenance.framebuffers[0].width != presentation.display_width
                || provenance.framebuffers[0].height != presentation.display_height
                || provenance.framebuffers[0].pitch_bytes != presentation.display_pitch
                || provenance.framebuffers[0].bits_per_pixel != presentation.display_bits_per_pixel
                || provenance.proof_class != BootProofClass::FreestandingEmulator
        })
    {
        return Err(ConduitosError::refusal(
            "invalid-observatory-snapshot",
            "ordinary Observatory identities, bounds, lifecycle, or sealed provenance disagreed with boot/kernel Signs",
        ));
    }
    let mut patchbay = patchbay_model::PatchbayTopology::new(1)
        .map_err(|error| ConduitosError::refusal("patchbay-rejected-report", error.to_string()))?;
    patchbay
        .ingest(snapshot)
        .map_err(|error| ConduitosError::refusal("patchbay-rejected-report", error.to_string()))?;
    let linear = patchbay
        .document(None)
        .map_err(|error| ConduitosError::refusal("patchbay-rejected-report", error.to_string()))?
        .lines()
        .join("\n");
    for required in [
        boot.host_id.as_str(),
        boot.boot_id.as_str(),
        kernel.plan_id.as_str(),
        kernel.active_play_id.as_str(),
        "input/keyboard",
        "conduitos/usb-hid-keyboard@1",
        "BOOT PROVENANCE [SEALED]",
    ] {
        if !linear.contains(required) {
            return Err(ConduitosError::refusal(
                "patchbay-linear-projection-incomplete",
                format!("native Patchbay projection omitted {required}"),
            ));
        }
    }
    Ok(())
}
