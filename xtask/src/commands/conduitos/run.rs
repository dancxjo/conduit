use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::cli::GlobalOpts;

use super::{
    image,
    profile::{Paths, EXPECTED_QEMU_SUCCESS, LIMINE_VERSION, QEMU_PROFILE},
    report::{GuestBootSign, GuestKernelSign, GuestRun},
    ConduitosArch, ConduitosError,
};

pub fn execute(arch: ConduitosArch, opts: &GlobalOpts) -> Result<GuestRun, ConduitosError> {
    let paths = Paths::new(arch)?;
    let _image = image::execute(arch, opts)?;
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
            "none",
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
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            ConduitosError::refusal(
                "missing-qemu",
                format!("cannot launch qemu-system-x86_64: {error}"),
            )
        })?;
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
    let output = child.wait_with_output().map_err(|error| {
        ConduitosError::refusal(
            "qemu-boot-failed",
            format!("cannot collect QEMU output: {error}"),
        )
    })?;
    if status.code() != Some(EXPECTED_QEMU_SUCCESS) {
        return Err(ConduitosError::refusal(
            "qemu-boot-failed",
            format!(
                "expected isa-debug-exit status {EXPECTED_QEMU_SUCCESS}, got {}; serial: {}",
                status,
                String::from_utf8_lossy(&output.stdout)
            ),
        ));
    }
    let serial = String::from_utf8(output.stdout).map_err(|error| {
        ConduitosError::refusal("malformed-boot-sign", format!("non-UTF-8 serial: {error}"))
    })?;
    let signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_BOOT_SIGN "))
        .collect();
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
    let presentations: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_SERIAL_PRESENT "))
        .collect();
    if presentations != ["Hello from ConduitOS"] {
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
    let boot: GuestBootSign = serde_json::from_str(signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-boot-sign", error.to_string()))?;
    let kernel: GuestKernelSign = serde_json::from_str(kernel_signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-kernel-sign", error.to_string()))?;
    let observatory: conduit_observatory::ObservatorySnapshot =
        serde_json::from_str(observatory_snapshots[0]).map_err(|error| {
            ConduitosError::refusal("malformed-observatory-snapshot", error.to_string())
        })?;
    validate_boot(&boot)?;
    validate_kernel(&boot, &kernel)?;
    validate_observatory(&boot, &kernel, &observatory)?;
    if !opts.quiet && !opts.json {
        println!("{}", signs[0]);
        println!("{}", kernel_signs[0]);
        println!("{}", observatory_snapshots[0]);
    }
    Ok(GuestRun {
        boot,
        kernel,
        observatory,
    })
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
        || sign.runtime_arena_bytes != 262_144
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
        || sign.scheduler_profile != "conduitos/single-lane-cooperative@1"
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
        || sign.cord_item_capacity != 1
        || sign.cord_byte_capacity != 20
        || sign.semantic_result != "Hello from ConduitOS"
        || sign.allocation_before_play == 0
        || sign.allocation_before_play != sign.allocation_after_play
        || sign.allocation_capacity != boot.runtime_arena_bytes as usize
        || !sign.allocation_stable_during_play
        || sign.base_count != 7
        || !valid_base_ids
        || sign.memory_arena_bytes != boot.runtime_arena_bytes
        || sign.execution_lanes != 1
        || sign.timer_slots != 0
        || sign.serial_slots != 1
        || sign.serial_maximum_bytes != 256
        || sign.interrupt_fact_slots != 4
        || sign.sign_item_slots != 64
        || sign.logical_operations != 2
        || sign.kernel_decisions == 0
        || sign.kernel_signs == 0
        || sign.timer_irq_wakes != 0
        || sign.serial_presentations != 1
        || !sign.clock_monotonic
        || sign.pending_host_operations != 0
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
                let exact_effect_realization = if kind == "text/literal" {
                    placement.resources.len() == 1 && placement.host_operations.is_empty()
                } else {
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
        "presentation/text",
        "conduit.std/presentation-text@1",
        "conduitos/kernel-serial-text@1",
    );
    let bases_match = snapshot.bases.len() == kernel.base_ids.len()
        && snapshot.bases.iter().all(|base| {
            base.host_id.as_str() == boot.host_id
                && base.boot_id.as_str() == boot.boot_id
                && base.state == OperationalState::Available
                && kernel.base_ids.iter().any(|id| id == base.base_id.as_str())
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
        && exact_base("conduitos.base/serial@1", 1)
        && exact_base("conduitos.base/interrupt@1", 4)
        && exact_base("conduitos.base/idle@1", 1)
        && exact_base("conduitos.base/execution-lane@1", 1);
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
        == 4;
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
        == 2;
    if snapshot.hosts.len() != 1
        || !snapshot.lines.is_empty()
        || snapshot.plans.len() != 1
        || snapshot.plays.len() != 1
        || snapshot.observations.len() != 4
        || snapshot.historical_observations.len() != 6
        || snapshot.sealed_boot_provenance.len() != 1
        || !bases_match
        || !base_inventory
        || !exact_text_placements
        || !current_signs
        || !historical_signs
        || host.is_none_or(|host| {
            host.advertisement.host_id.as_str() != boot.host_id
                || host.advertisement.boot_id.as_str() != boot.boot_id
                || host.advertisement.profile.as_str() != kernel.scheduler_profile
                || host.advertisement.capabilities.len() != 2
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
                || host.advertisement.resources.len() != 4
                || host.state != OperationalState::Available
        })
        || plan.is_none_or(|plan| {
            plan.plan_id.as_str() != kernel.plan_id
                || plan.source_document_id.as_str() != kernel.source_document_id
                || plan.checked_form_id.as_str() != kernel.checked_form_id
                || plan.expanded_form_id.as_str() != kernel.expanded_form_id
                || plan.fragments.len() != 1
                || plan.fragments[0].fragment_id.as_str() != kernel.fragment_id
                || plan.fragments[0].placements.len() != 2
                || plan.fragments[0].connections.len() != 1
                || plan.fragments[0].connections[0].item_capacity != kernel.cord_item_capacity
                || plan.fragments[0].connections[0].byte_capacity != kernel.cord_byte_capacity
        })
        || play.is_none_or(|play| {
            play.active_play_id.as_str() != kernel.active_play_id
                || play.plan_id.as_str() != kernel.plan_id
                || play.host_id.as_str() != boot.host_id
                || play.boot_id.as_str() != boot.boot_id
                || play.lifecycle != PlanLifecycle::Completed
                || play.placements.len() != 2
                || play.connections.len() != 1
                || play.connections[0]
                    .pressure
                    .as_ref()
                    .is_none_or(|pressure| {
                        pressure.current_in_flight_items != Some(0)
                            || pressure.current_buffered_bytes != Some(0)
                            || pressure.pressure_events != 0
                            || pressure.last_pressure_sequence.is_some()
                    })
        })
        || snapshot.retention.item_capacity != 64
        || snapshot.retention.retained_items != 10
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
                || !provenance.framebuffers.is_empty()
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
