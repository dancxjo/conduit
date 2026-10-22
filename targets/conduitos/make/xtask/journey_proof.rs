//! A user-driven Crèche → Body → Patchbay → Face journey in the real x86 guest.
//! Every image is captured after an observed guest transition, never fabricated
//! from a fixture or a retired Tour surface.

mod creche_choices;
mod creche_controls;
mod creche_pointer;
mod face_correspondence;
mod face_input;
mod failure_stack;
mod keyboard_domain;
mod look_selection;
mod timing_interactions;

use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::{json, Value};

use super::{
    hid_qmp, image, journey_input, journey_records, profile::Paths, report::git_head,
    ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;

pub(super) struct JourneyIdentity {
    pub profile_id: String,
    pub build_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub spore_join: Option<Value>,
}

pub fn execute_viewport(
    opts: &GlobalOpts,
    viewport: Option<super::proof_viewport::Viewport>,
) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(refusal(
            "a live product journey needs a real image and guest",
        ));
    }
    let built = image::execute_architecture_proof(ConduitosArch::X86_64, opts)?;
    let paths = Paths::new(ConduitosArch::X86_64)?;
    execute_with_viewport(
        opts,
        &paths.iso,
        built.iso_sha256,
        false,
        None,
        viewport,
        None,
    )
    .map(|_| ())
}

pub(super) fn execute_supplied(
    opts: &GlobalOpts,
    image_path: &Path,
    image_sha256: String,
) -> Result<JourneyIdentity, ConduitosError> {
    execute_supplied_observed(opts, image_path, image_sha256, false, None)
}

pub(super) fn execute_supplied_viewport(
    opts: &GlobalOpts,
    image_path: &Path,
    image_sha256: String,
    viewport: Option<super::proof_viewport::Viewport>,
) -> Result<JourneyIdentity, ConduitosError> {
    execute_with_viewport(opts, image_path, image_sha256, false, None, viewport, None)
}

pub(super) fn execute_supplied_observed(
    opts: &GlobalOpts,
    image_path: &Path,
    image_sha256: String,
    observe: bool,
    output: Option<&Path>,
) -> Result<JourneyIdentity, ConduitosError> {
    execute_with_viewport(opts, image_path, image_sha256, observe, output, None, None)
}
pub(super) fn execute_controls(
    opts: &GlobalOpts,
    image_path: &Path,
    image_sha256: String,
    viewport: Option<super::proof_viewport::Viewport>,
    metadata: &Value,
) -> Result<JourneyIdentity, ConduitosError> {
    execute_with_viewport(
        opts,
        image_path,
        image_sha256,
        false,
        None,
        viewport,
        Some(metadata),
    )
}
#[allow(clippy::too_many_arguments)]
fn execute_with_viewport(
    opts: &GlobalOpts,
    image_path: &Path,
    image_sha256: String,
    observe: bool,
    output: Option<&Path>,
    viewport: Option<super::proof_viewport::Viewport>,
    controls: Option<&Value>,
) -> Result<JourneyIdentity, ConduitosError> {
    if opts.dry_run {
        return Err(refusal("a live product journey needs a real guest"));
    }
    let mut paths = Paths::new(ConduitosArch::X86_64)?;
    if let Some(output) = output {
        paths.target = output.to_owned();
    }
    fs::create_dir_all(&paths.target).map_err(io_error)?;
    // Unix sockets have a small pathname limit, independent of filesystem limits.
    let monitor_socket = std::env::temp_dir().join(format!("cj-{}.sock", std::process::id()));
    let serial_path = paths.target.join("journey-serial.log");
    let _ = fs::remove_file(&monitor_socket);
    let _ = fs::remove_file(&serial_path);
    for name in [
        "journey-failure-registers.json",
        "journey-failure-stack.json",
    ] {
        match fs::remove_file(paths.target.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(error)),
        }
    }
    let mut command = Command::new("qemu-system-x86_64");
    command.args([
        "-M",
        "q35",
        "-cpu",
        "max",
        "-m",
        conduitos::make::PC_PRODUCT_QEMU_MEMORY,
        "-smp",
        "1",
        "-display",
        "none",
    ]);
    super::proof_viewport::append_vga(&mut command, viewport);
    command
        .args([
            "-monitor",
            "none",
            "-qmp",
            &format!("unix:{},server=on,wait=off", monitor_socket.display()),
            "-serial",
            &format!("file:{}", serial_path.display()),
            "-no-reboot",
            "-net",
            "none",
            "-device",
            "qemu-xhci,id=conduitos-xhci,p2=3,p3=0",
            "-device",
            "usb-kbd,id=conduitos-keyboard,bus=conduitos-xhci.0,port=1",
            "-device",
            "usb-mouse,id=conduitos-pointer,bus=conduitos-xhci.0,port=2",
            "-cdrom",
            image_path
                .to_str()
                .ok_or_else(|| refusal("non-UTF-8 image path"))?,
            "-boot",
            "d",
        ])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(fs::File::create(paths.target.join("journey-qemu-stderr.log")).map_err(io_error)?);
    let source = git_head(&paths.root)?;
    let mut artifacts = super::qemu_artifacts::Artifacts::new(
        paths.target.join("journey-frames"),
        serial_path.clone(),
        json!({"source_commit":source,"image_sha256":image_sha256,
            "qemu_argv":command.get_args().map(|arg|arg.to_string_lossy().into_owned()).collect::<Vec<_>>()}),
    )?
    .with_capture_limit(if observe { 32 } else { 27 })?
    .with_viewport(viewport)?;
    let mut child = command
        .spawn()
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;
    let result = (|| {
        let (mut qmp, mut reader) = super::qmp::connect_traced_at(
            &monitor_socket,
            &mut child,
            Some(&paths.target.join("journey-qmp.log")),
            artifacts.observation_origin(),
        )?;
        hid_qmp::wait_for_product_arrival(
            &serial_path,
            &mut child,
            "CONDUIT_BOOT_STAGE front-door-ready",
            "journey-arrival-timeout",
        )?;
        artifacts.capture(&mut qmp, &mut reader, "front-door-ready", false)?;
        journey_input::enter_creche(&mut qmp, &mut reader, &serial_path, &mut child)?;
        if let Some(metadata) = controls {
            return creche_controls::exercise(
                &serial_path,
                &mut child,
                &mut qmp,
                &mut reader,
                &mut artifacts,
                &paths.target,
                &source,
                &image_sha256,
                metadata,
            );
        }
        artifacts.capture(&mut qmp, &mut reader, "creche-ready", false)?;
        let look_selection = if observe {
            json!({"status":"not-exercised", "reason":"keep the latency workload fixed"})
        } else {
            look_selection::exercise(
                &serial_path,
                &mut child,
                &mut qmp,
                &mut reader,
                &mut artifacts,
            )?
        };

        creche_choices::exercise(
            &serial_path,
            &mut child,
            &mut qmp,
            &mut reader,
            &mut artifacts,
        )?;
        journey_input::birth_from_creche(&mut qmp, &mut reader, &serial_path, &mut child)?;
        // Birth is the only user lifecycle activation. The default policy
        // must produce separate Wake/Plan/Play evidence without F4/F5/F6.
        for status in ["awake", "planned", "quiescent-awaiting-input"] {
            journey_input::wait_status_retaining_guest(&serial_path, &mut child, status)?;
        }
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "CONDUIT_CRECHE_CHECKPOINT body-playing",
            "commons-arrival-timeout",
        )?;
        artifacts.capture(&mut qmp, &mut reader, "body-playing", true)?;
        journey_input::open_hub_plot(
            &mut qmp,
            &mut reader,
            &serial_path,
            &mut child,
            conduitos::native_workset::NativePlot::Tour,
        )?;
        face_input::unavailable_wake(
            &serial_path,
            &mut child,
            &mut qmp,
            &mut reader,
            &mut artifacts,
        )?;

        let keyboard_identity =
            keyboard_domain::exercise(&serial_path, &mut child, &mut qmp, &mut reader)?;
        artifacts.capture(&mut qmp, &mut reader, "protected-keyboard-canvas", true)?;

        let editor_identity = keyboard_domain::exercise_editor(
            &serial_path,
            &mut child,
            &mut qmp,
            &mut reader,
            observe.then_some(&mut artifacts),
        )?;
        // In observation mode the last typed value already has its own frame.
        artifacts.capture(&mut qmp, &mut reader, "protected-memory-lantern", !observe)?;

        keyboard_domain::verify_sibling(&serial_path, &mut child, &mut qmp, &mut reader)?;

        journey_input::key_pair(&mut qmp, &mut reader, "esc", "journey-home")?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "CONDUIT_HOME_CHECKPOINT returned",
            "journey-home-timeout",
        )?;
        artifacts.capture(&mut qmp, &mut reader, "home", true)?;
        journey_input::open_hub_plot(
            &mut qmp,
            &mut reader,
            &serial_path,
            &mut child,
            conduitos::native_workset::NativePlot::Patchbay,
        )?;
        artifacts.capture(&mut qmp, &mut reader, "patchbay-workspace", true)?;

        // Canonical Patchbay enters its checked Face as part of selection.
        face_correspondence::current_patchbay_show(
            &fs::read_to_string(&serial_path).map_err(io_error)?,
            &latest_workspace_view(&serial_path)?,
        )?;
        artifacts.capture(&mut qmp, &mut reader, "patchbay-face", false)?;
        if observe {
            timing_interactions::reading_pages(
                &serial_path,
                &mut child,
                &mut qmp,
                &mut reader,
                &mut artifacts,
            )?;
        }
        let before = stage_count(&serial_path, "CONDUIT_WORKSPACE_VIEW ")?;
        journey_input::key_pair(&mut qmp, &mut reader, "f3", "journey-open-diagram")?;
        hid_qmp::wait_for_stage_count(
            &serial_path,
            &mut child,
            "CONDUIT_WORKSPACE_VIEW ",
            before + 1,
            "journey-diagram-timeout",
        )?;
        artifacts.capture(&mut qmp, &mut reader, "patchbay-diagram", true)?;

        let inspected_view = latest_workspace_view(&serial_path)?;
        for (key, frame) in [
            ("1", "patchbay-gear-details"),
            ("f3", "patchbay-diagram-returned"),
        ] {
            let before = stage_count(&serial_path, "CONDUIT_WORKSPACE_VIEW ")?;
            journey_input::key_pair(&mut qmp, &mut reader, key, "journey-gear-inspection")?;
            hid_qmp::wait_for_stage_count(
                &serial_path,
                &mut child,
                "CONDUIT_WORKSPACE_VIEW ",
                before + 1,
                "journey-gear-inspection-timeout",
            )?;
            artifacts.capture(&mut qmp, &mut reader, frame, true)?;
            let current = latest_workspace_view(&serial_path)?;
            for field in ["face_id", "body_id", "view_sha256", "status"] {
                if inspected_view[field].as_str().is_none()
                    || current[field] != inspected_view[field]
                {
                    return Err(refusal(format!("Gear inspection changed owner {field}")));
                }
            }
        }

        journey_input::key_pair(&mut qmp, &mut reader, "f8", "journey-stop-through-face")?;
        journey_input::wait_status_retaining_guest(&serial_path, &mut child, "stopped")?;
        artifacts.capture(&mut qmp, &mut reader, "body-stopped", true)?;
        face_input::return_and_reenter(
            &serial_path,
            &mut child,
            &mut qmp,
            &mut reader,
            &mut artifacts,
        )?;
        let serial = fs::read_to_string(&serial_path).map_err(io_error)?;
        face_correspondence::retain(
            &serial,
            &paths.target.join("native-shell-paired-linear.json"),
            &source,
            &image_sha256,
        )?;
        let records = journey_records::decode(&serial)?;
        let identity = validate(&records, &serial)?;
        let keyboard_cost = keyboard_domain::validate_cost(&serial, &keyboard_identity, 5, 1)?;
        // Initial empty Current manifests the caret before input: one extra
        // Face Host Call and its two protected domain entries.
        let editor_cost = keyboard_domain::validate_cost(&serial, &editor_identity, 29, 7)?;
        if keyboard_cost["domain_id"] == editor_cost["domain_id"]
            || keyboard_cost["partition_plan_id"] == editor_cost["partition_plan_id"]
        {
            return Err(refusal(
                "keyboard and retained editor shared a protection domain or partition",
            ));
        }
        if child.try_wait().map_err(io_error)?.is_some() {
            return Err(refusal("guest exited before the journey finished"));
        }
        let proof = json!({
            "schema":"conduit.conduitos/face-journey-proof@1",
            "proof_class":"freestanding-emulator",
            "source_commit":source, "image_sha256":image_sha256,
            "profile_id":identity.profile_id, "build_id":identity.build_id,
            "image_id":records.iter().find(|r|r["status"]=="born-lulled").unwrap()["image_id"],
            "host_id":identity.host_id, "boot_id":identity.boot_id,
            "body_id":records.iter().find(|r|r["status"]=="born-lulled").unwrap()["body_id"],
            "input":"real-qmp-keyboard-and-pointer", "screenshots":"journey-frames/manifest.json",
            "protected_keyboard_domain": keyboard_cost,
            "protected_retained_text_domain": editor_cost,
            "committed_edit_receipt": "memory-lantern-native-edit.json",
            "steps":["arrive","creche-three-selected","creche-four-restored","creche-fourth-checked","creche-pointer-three-selected","creche-pointer-four-restored","birth","wake","plan","play","commons-hub","commons-workshop","unavailable-wake-refused","protected-keyboard-canvas","protected-memory-lantern","home","patchbay","face","diagram","gear-details","diagram-return","stop","home-returned","patchbay-reentered"],
            "paired_reconstruction":"native-shell-paired-linear.json",
            "look_selection":look_selection,
            "pointer_actions":creche_pointer::receipt(&serial)?, "browser_comparison":"unsupported",
            "parent_5356_acceptance":"open",
            "physical_evidence":false, "human_enactment":false,
        });
        fs::write(
            paths.target.join("journey-proof.json"),
            serde_json::to_vec_pretty(&proof).map_err(|e| refusal(e.to_string()))?,
        )
        .map_err(io_error)?;
        if !opts.quiet && !opts.json {
            println!(
                "ConduitOS Face journey proof: {}",
                paths.target.join("journey-proof.json").display()
            );
        }
        Ok(identity)
    })();
    if result.is_err() {
        if let Ok((mut qmp, mut reader)) = hid_qmp::connect(&monitor_socket, &mut child) {
            // Freeze only after failure so registers and stack describe one CPU state.
            if let Err(error) = super::qmp::request(
                &mut qmp,
                &mut reader,
                br#"{"execute":"stop"}"#,
                "journey-failure-stop",
            ) {
                artifacts.diagnostic_failure(&error);
            }
            let registers = super::qmp::request_value(
                &mut qmp,
                &mut reader,
                br#"{"execute":"human-monitor-command","arguments":{"command-line":"info registers"}}"#,
                "journey-failure-registers",
            );
            match registers {
                Ok(value) => {
                    if let Err(error) =
                        failure_stack::capture(&mut qmp, &mut reader, &value, &paths.target)
                    {
                        artifacts.diagnostic_failure(&error);
                    }
                    let retained = serde_json::to_vec_pretty(&value)
                        .map_err(|error| refusal(error.to_string()))
                        .and_then(|bytes| {
                            fs::write(paths.target.join("journey-failure-registers.json"), bytes)
                                .map_err(io_error)
                        });
                    if let Err(error) = retained {
                        artifacts.diagnostic_failure(&error);
                    }
                }
                Err(error) => artifacts.diagnostic_failure(&error),
            }
            if let Err(error) = artifacts.capture(&mut qmp, &mut reader, "failure", false) {
                artifacts.diagnostic_failure(&error);
            }
        }
    }
    let _ = child.kill();
    if let Ok(status) = child.wait() {
        artifacts.stopped(
            &status,
            if result.is_ok() {
                "harness-stop-after-success"
            } else {
                "harness-stop-after-failure"
            },
        );
    }
    if let Err(error) = artifacts.finish(result.as_ref().err()) {
        if result.is_ok() {
            return Err(error);
        }
        eprintln!("journey evidence error: {error}");
    }
    result
}

fn stage_count(path: &Path, marker: &str) -> Result<usize, ConduitosError> {
    Ok(fs::read_to_string(path)
        .map_err(io_error)?
        .matches(marker)
        .count())
}

fn validate(records: &[Value], serial: &str) -> Result<JourneyIdentity, ConduitosError> {
    let stages = [
        "world",
        "born-lulled",
        "awake",
        "planned",
        "quiescent-awaiting-input",
        "stopped",
    ];
    let mut previous = None;
    let mut body = None;
    for status in stages {
        let (index, record) = records
            .iter()
            .enumerate()
            .find(|(index, record)| {
                previous.is_none_or(|last| *index > last) && record["status"] == status
            })
            .ok_or_else(|| refusal(format!("missing ordered {status} lifecycle sign")))?;
        if status == "world" {
            if !record["body_id"].is_null() {
                return Err(refusal("arrival already has a Body"));
            }
        } else {
            let current = record["body_id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or_else(|| refusal("post-birth Body identity absent"))?;
            if body.is_some_and(|known| known != current) {
                return Err(refusal("journey changed Body identity"));
            }
            body = Some(current);
        }
        if status == "planned"
            && (record["gear_ids"].as_array().is_none_or(Vec::is_empty)
                || record["port_ids"].as_array().is_none_or(Vec::is_empty)
                || record["cord_ids"].as_array().is_none_or(Vec::is_empty))
        {
            return Err(refusal("the Patchbay diagram has no planned graph"));
        }
        previous = Some(index);
    }
    for marker in [
        "CONDUIT_HOME_CHECKPOINT plot-opened patchbay",
        "CONDUIT_WORKSPACE_FACE shown",
        "CONDUIT_WORKSPACE_VIEW ",
    ] {
        if !serial.contains(marker) {
            return Err(refusal(format!("missing guest Face checkpoint {marker}")));
        }
    }
    let boot = journey_records::boot(serial)?.ok_or_else(|| refusal("boot identity absent"))?;
    let text = |field: &str| -> Result<String, ConduitosError> {
        boot[field]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| refusal(format!("boot {field} absent")))
    };
    let spore_join = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_SPORE_JOIN "))
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()
        .map_err(|error| refusal(error.to_string()))?;
    if spore_join.len() > 1 {
        return Err(refusal("ambiguous spore join"));
    }
    Ok(JourneyIdentity {
        profile_id: text("profile_id")?,
        build_id: text("build_id")?,
        host_id: text("host_id")?,
        boot_id: text("boot_id")?,
        spore_join: spore_join.into_iter().next(),
    })
}

fn refusal(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("face-journey-refused", detail)
}
fn io_error(error: std::io::Error) -> ConduitosError {
    ConduitosError::refusal("face-journey-io", error.to_string())
}

/// Read the latest actual guest record; a missing or malformed record refuses.
fn latest_workspace_view(path: &Path) -> Result<Value, ConduitosError> {
    let serial = fs::read_to_string(path).map_err(io_error)?;
    let encoded = serial
        .lines()
        .rev()
        .find_map(|line| line.strip_prefix("CONDUIT_WORKSPACE_VIEW "))
        .ok_or_else(|| refusal("Gear inspection lacks a workspace-view receipt"))?;
    serde_json::from_str(encoded).map_err(|error| refusal(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proof_rejects_a_body_that_changes_after_birth() {
        let stages = [
            "world",
            "born-lulled",
            "awake",
            "planned",
            "quiescent-awaiting-input",
            "stopped",
        ];
        let records: Vec<_> = stages.iter().enumerate().map(|(i,status)| {
            json!({"status":status,"body_id":if i==0 {Value::Null} else if i==5 {json!("other")} else {json!("body")},
                "gear_ids":["g"],"port_ids":["p"],"cord_ids":["c"]})
        }).collect();
        assert!(validate(&records, "").is_err());
    }
}
