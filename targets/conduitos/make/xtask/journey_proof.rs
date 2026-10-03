//! A user-driven Crèche → Body → Patchbay → Face journey in the real x86 guest.
//! Every image is captured after an observed guest transition, never fabricated
//! from a fixture or a retired Tour surface.

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

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(refusal(
            "a live product journey needs a real image and guest",
        ));
    }
    let built = image::execute_architecture_proof(ConduitosArch::X86_64, opts)?;
    let paths = Paths::new(ConduitosArch::X86_64)?;
    execute_supplied(opts, &paths.iso, built.iso_sha256).map(|_| ())
}

pub(super) fn execute_supplied(
    opts: &GlobalOpts,
    image_path: &Path,
    image_sha256: String,
) -> Result<JourneyIdentity, ConduitosError> {
    if opts.dry_run {
        return Err(refusal("a live product journey needs a real guest"));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    fs::create_dir_all(&paths.target).map_err(io_error)?;
    let monitor_socket = paths.target.join("journey-monitor.sock");
    let serial_path = paths.target.join("journey-serial.log");
    let _ = fs::remove_file(&monitor_socket);
    let _ = fs::remove_file(&serial_path);
    let mut command = Command::new("qemu-system-x86_64");
    command
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
    )?;
    let mut child = command
        .spawn()
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;
    let result = (|| {
        let (mut qmp, mut reader) = super::qmp::connect_traced(
            &monitor_socket,
            &mut child,
            Some(&paths.target.join("journey-qmp.log")),
        )?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "CONDUIT_BOOT_STAGE front-door-ready",
            "journey-arrival-timeout",
        )?;
        artifacts.capture(&mut qmp, &mut reader, "front-door-ready", false)?;

        journey_input::birth_from_creche(&mut qmp, &mut reader)?;
        journey_input::wait_status(&serial_path, &mut child, "born-lulled")?;
        artifacts.capture(&mut qmp, &mut reader, "body-born", true)?;
        for (key, status, frame) in [
            ("f4", "awake", "body-woken"),
            ("f5", "planned", "body-planned"),
            ("f6", "quiescent-awaiting-input", "body-playing"),
        ] {
            journey_input::key_pair(&mut qmp, &mut reader, key, "journey-lifecycle")?;
            journey_input::wait_status(&serial_path, &mut child, status)?;
            artifacts.capture(&mut qmp, &mut reader, frame, true)?;
        }

        journey_input::key_pair(&mut qmp, &mut reader, "esc", "journey-home")?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "CONDUIT_HOME_CHECKPOINT returned",
            "journey-home-timeout",
        )?;
        artifacts.capture(&mut qmp, &mut reader, "home", true)?;
        journey_input::key_pair(&mut qmp, &mut reader, "tab", "journey-select-patchbay")?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "CONDUIT_HOME_STATE launcher 1",
            "journey-patchbay-selection-timeout",
        )?;
        journey_input::key_pair(&mut qmp, &mut reader, "ret", "journey-open-patchbay")?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "CONDUIT_HOME_CHECKPOINT patchbay-opened",
            "journey-patchbay-timeout",
        )?;
        artifacts.capture(&mut qmp, &mut reader, "patchbay-workspace", true)?;

        journey_input::key_pair(&mut qmp, &mut reader, "f2", "journey-open-face")?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "CONDUIT_WORKSPACE_FACE shown",
            "journey-face-timeout",
        )?;
        artifacts.capture(&mut qmp, &mut reader, "patchbay-face", true)?;
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

        journey_input::key_pair(&mut qmp, &mut reader, "f8", "journey-stop-through-face")?;
        journey_input::wait_status(&serial_path, &mut child, "stopped")?;
        artifacts.capture(&mut qmp, &mut reader, "body-stopped", true)?;
        let serial = fs::read_to_string(&serial_path).map_err(io_error)?;
        let records = journey_records::decode(&serial)?;
        let identity = validate(&records, &serial)?;
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
            "input":"real-qmp-keyboard", "screenshots":"journey-frames/manifest.json",
            "steps":["arrive","birth","wake","plan","play","home","patchbay","face","diagram","stop"],
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
        "CONDUIT_HOME_CHECKPOINT patchbay-opened",
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
