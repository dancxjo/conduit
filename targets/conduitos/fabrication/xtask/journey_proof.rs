//! Native-QEMU acceptance for one ordinary in-guest product lifecycle.

use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use serde::Serialize;
use serde_json::Value;

use crate::cli::GlobalOpts;

use super::{hid_qmp, image, profile::Paths, report::git_head, ConduitosArch, ConduitosError};

use super::journey_records::decode as journey_records;

#[derive(Serialize)]
struct JourneyProof {
    schema: &'static str,
    base_commit: String,
    image_sha256: String,
    profile_id: String,
    build_id: String,
    image_id: String,
    host_id: String,
    profile: &'static str,
    boot_id: String,
    source_document_id: String,
    checked_form_id: String,
    expanded_form_id: String,
    body_id: String,
    born_sign_id: String,
    part_id: String,
    wake_id: String,
    plan_id: String,
    active_play_id: String,
    gear_ids: Vec<String>,
    port_ids: Vec<String>,
    cord_ids: Vec<String>,
    presentation_id: String,
    manifestation_id: String,
    presenter_implementation_id: String,
    input_sign_id: String,
    result_sign_id: String,
    result: String,
    open_effects: u8,
    body_retained_after_lull: bool,
    remained_alive: bool,
    stopped_by_harness: bool,
}

pub(super) struct JourneyIdentity {
    pub profile_id: String,
    pub build_id: String,
    pub image_id: String,
    pub host_id: String,
    pub boot_id: String,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-product-journey-proof",
            "product journey proof requires a real normal IMAGE and QEMU lifecycle",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let image = image::execute_architecture_proof(ConduitosArch::X86_64, opts)?;
    let image_path = paths.iso.clone();
    execute_image(opts, paths, &image_path, image.iso_sha256).map(|_| ())
}

pub(super) fn execute_supplied(
    opts: &GlobalOpts,
    image_path: &Path,
    image_sha256: String,
) -> Result<JourneyIdentity, ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-spore-acceptance",
            "Crèche spore acceptance requires a real supplied artifact and QEMU lifecycle",
        ));
    }
    execute_image(
        opts,
        Paths::new(ConduitosArch::X86_64)?,
        image_path,
        image_sha256,
    )
}

fn execute_image(
    opts: &GlobalOpts,
    paths: Paths,
    image_path: &Path,
    image_sha256: String,
) -> Result<JourneyIdentity, ConduitosError> {
    let monitor_socket = paths.target.join("journey-monitor.sock");
    let serial_path = paths.target.join("journey-serial.log");
    let proof_path = paths.target.join("journey-proof.json");
    let _ = fs::remove_file(&monitor_socket);
    let _ = fs::remove_file(&serial_path);
    let monitor = format!(
        "unix:{},server=on,wait=off",
        monitor_socket.to_string_lossy()
    );
    let serial = format!("file:{}", serial_path.to_string_lossy());
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
            &monitor,
            "-serial",
            &serial,
            "-no-reboot",
            "-net",
            "none",
            "-device",
            "qemu-xhci,id=conduitos-xhci,p2=1,p3=0",
            "-device",
            "usb-kbd,bus=conduitos-xhci.0,port=1",
            "-cdrom",
            image_path.to_str().ok_or_else(|| {
                ConduitosError::refusal("product-journey-image-path-invalid", "non-UTF-8 ISO path")
            })?,
            "-boot",
            "d",
        ])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(
            fs::File::create(paths.target.join("journey-qemu-stderr.log"))
                .map_err(|error| ConduitosError::refusal("qemu-stderr-io", error.to_string()))?,
        );
    let mut artifacts = super::qemu_artifacts::Artifacts::new(
        paths.target.join("journey-frames"),
        serial_path.clone(),
        serde_json::json!({"source_commit":git_head(&paths.root)?,"image_sha256":image_sha256.clone(),
            "qemu_argv":command.get_args().map(|value|value.to_string_lossy().into_owned()).collect::<Vec<_>>()}),
    )?;
    let mut child = command
        .spawn()
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;

    let result = (|| {
        let interaction = (|| {
            let (mut qmp, mut reader) = super::qmp::connect_traced(
                &monitor_socket,
                &mut child,
                Some(&paths.target.join("journey-qmp.log")),
            )?;
            hid_qmp::wait_for_stage(
                &serial_path,
                &mut child,
                "CONDUIT_BOOT_STAGE front-door-ready",
                "product-journey-front-door-timeout",
            )?;
            artifacts.capture(&mut qmp, &mut reader, "front-door-ready", false)?;
            for (key, status) in [
                ("ret", "form-opened"),
                ("f3", "born-lulled"),
                ("f4", "awake"),
                ("f5", "planned"),
            ] {
                key_pair(&mut qmp, &mut reader, key, status)?;
                wait_status(&serial_path, &mut child, status)?;
                artifacts.capture(&mut qmp, &mut reader, status, true)?;
            }
            for label in [
                "PROFILE ID",
                "BUILD ID",
                "IMAGE BINDING",
                "HOST ID",
                "BOOT ID",
                "CURRENT OFFERS",
                "FORM SUBJECT",
                "SOURCE DOCUMENT ID",
                "CHECKED FORM ID",
                "EXPANDED FORM ID",
                "BODY ID",
                "WAKE ID",
                "PLAN ID",
            ] {
                key_pair(&mut qmp, &mut reader, "f2", "planned-detail")?;
                hid_qmp::wait_for_stage(
                    &serial_path,
                    &mut child,
                    &format!("\"label\":\"{label}\""),
                    "product-journey-plan-inspection-timeout",
                )?;
            }
            key_pair(&mut qmp, &mut reader, "esc", "leave-details")?;
            key_pair(&mut qmp, &mut reader, "f6", "playing")?;
            wait_status(&serial_path, &mut child, "playing")?;
            artifacts.capture(&mut qmp, &mut reader, "playing", true)?;
            key_pair(&mut qmp, &mut reader, "a", "semantic-input")?;
            wait_status(&serial_path, &mut child, "result-visible")?;
            artifacts.capture(&mut qmp, &mut reader, "result-visible", true)?;
            key_pair(&mut qmp, &mut reader, "f7", "lull")?;
            wait_status(&serial_path, &mut child, "lulled")?;
            artifacts.capture(&mut qmp, &mut reader, "lulled", true)?;
            thread::sleep(Duration::from_millis(250));
            if child
                .try_wait()
                .map_err(|error| {
                    ConduitosError::refusal("product-journey-qemu-wait-failed", error.to_string())
                })?
                .is_some()
            {
                return Err(ConduitosError::refusal(
                    "product-journey-not-long-lived",
                    "normal IMAGE exited after the ordinary product lifecycle",
                ));
            }
            Ok(())
        })();
        interaction?;
        child.kill().map_err(|error| {
            ConduitosError::refusal("product-journey-qemu-stop-failed", error.to_string())
        })?;
        let stopped = child.wait().map_err(|error| {
            ConduitosError::refusal("product-journey-qemu-wait-failed", error.to_string())
        })?;
        artifacts.stopped(&stopped, "harness-kill-after-interaction");
        let serial = fs::read_to_string(&serial_path).map_err(|error| {
            ConduitosError::refusal("product-journey-serial-unavailable", error.to_string())
        })?;
        let records = journey_records(&serial)?;
        let by_status = records
            .iter()
            .filter_map(|record| Some((record.get("status")?.as_str()?.to_owned(), record)))
            .collect::<BTreeMap<_, _>>();
        for status in [
            "form-opened",
            "born-lulled",
            "awake",
            "planned",
            "playing",
            "result-visible",
            "lulled",
        ] {
            if !by_status.contains_key(status) {
                return Err(ConduitosError::refusal(
                    "product-journey-stage-missing",
                    status,
                ));
            }
        }
        let opened = by_status["form-opened"];
        if opened.get("body_id") != Some(&Value::Null)
            || opened.get("wake_id") != Some(&Value::Null)
            || opened.get("plan_id") != Some(&Value::Null)
            || opened.get("active_play_id") != Some(&Value::Null)
        {
            return Err(ConduitosError::refusal(
                "product-journey-open-had-effects",
                "OPEN created lifecycle truth before explicit BIRTH",
            ));
        }
        let born = by_status["born-lulled"];
        let planned = by_status["planned"];
        let playing = by_status["playing"];
        let result = by_status["result-visible"];
        let lulled = by_status["lulled"];
        let plan_id = text(planned, "plan_id")?;
        let inspected_plan = serial.lines().any(|line| {
            line.contains("CONDUIT_FRONT_DOOR_SIGN")
                && line.contains("\"label\":\"PLAN ID\"")
                && line.contains(&format!("\"value\":\"{plan_id}\""))
        });
        if planned.get("active_play_id") != Some(&Value::Null)
            || planned
                .get("gear_ids")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
            || planned
                .get("port_ids")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
            || planned
                .get("cord_ids")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
            || playing.get("active_play_id") == Some(&Value::Null)
            || playing.get("plan_id") == playing.get("active_play_id")
            || result.get("result").and_then(Value::as_str) != Some("A")
            || born.get("body_id") != lulled.get("body_id")
            || !inspected_plan
        {
            return Err(ConduitosError::refusal(
                "product-journey-causality-invalid",
                "exact Plan/Play/result/LULL causality did not match the product contract",
            ));
        }
        for identity in [
            "profile_id",
            "build_id",
            "image_id",
            "host_id",
            "boot_id",
            "source_document_id",
            "checked_form_id",
            "expanded_form_id",
        ] {
            let expected = opened.get(identity);
            if expected.is_none()
                || records
                    .iter()
                    .any(|record| record.get(identity) != expected)
            {
                return Err(ConduitosError::refusal(
                    "product-journey-identity-drift",
                    identity,
                ));
            }
        }
        if serial.contains("CONDUIT_KERNEL_SIGN") || serial.contains("body-patchbay-open") {
            return Err(ConduitosError::refusal(
                "product-journey-used-proof-entrance",
                "normal product lifecycle emitted scripted proof entrance evidence",
            ));
        }
        let proof = JourneyProof {
            schema: "conduit.conduitos/product-journey-proof@1",
            base_commit: git_head(&paths.root)?,
            image_sha256,
            profile_id: text(opened, "profile_id")?,
            build_id: text(opened, "build_id")?,
            image_id: text(opened, "image_id")?,
            host_id: text(opened, "host_id")?,
            profile: super::demo::DEMO_PROFILE,
            boot_id: text(opened, "boot_id")?,
            source_document_id: text(opened, "source_document_id")?,
            checked_form_id: text(opened, "checked_form_id")?,
            expanded_form_id: text(opened, "expanded_form_id")?,
            body_id: text(born, "body_id")?,
            born_sign_id: text(born, "born_sign_id")?,
            part_id: text(born, "part_id")?,
            wake_id: text(by_status["awake"], "wake_id")?,
            plan_id,
            active_play_id: text(playing, "active_play_id")?,
            gear_ids: strings(planned, "gear_ids")?,
            port_ids: strings(planned, "port_ids")?,
            cord_ids: strings(planned, "cord_ids")?,
            presentation_id: text(result, "presentation_id")?,
            manifestation_id: text(result, "manifestation_id")?,
            presenter_implementation_id: text(result, "presenter_implementation_id")?,
            input_sign_id: text(result, "input_sign_id")?,
            result_sign_id: text(result, "result_sign_id")?,
            result: text(result, "result")?,
            open_effects: 0,
            body_retained_after_lull: true,
            remained_alive: true,
            stopped_by_harness: true,
        };
        fs::write(
            &proof_path,
            serde_json::to_vec_pretty(&proof).map_err(|error| {
                ConduitosError::refusal("product-journey-proof-invalid", error.to_string())
            })?,
        )
        .map_err(|error| {
            ConduitosError::refusal("product-journey-proof-unavailable", error.to_string())
        })?;
        if !opts.quiet && !opts.json {
            println!("ConduitOS product journey proof: {}", proof_path.display());
        }
        Ok(JourneyIdentity {
            profile_id: proof.profile_id.clone(),
            build_id: proof.build_id.clone(),
            image_id: proof.image_id.clone(),
            host_id: proof.host_id.clone(),
            boot_id: proof.boot_id.clone(),
        })
    })();
    if result.is_err() {
        if let Some(status) = child.try_wait().ok().flatten() {
            artifacts.stopped(&status, "exited-before-failure-diagnostics");
        } else {
            let diagnostic = hid_qmp::connect(&monitor_socket, &mut child).and_then(|(mut stream, mut reader)| {
                artifacts.registers(super::qmp::request_value(&mut stream, &mut reader,
                    br#"{"execute":"human-monitor-command","arguments":{"command-line":"info registers"}}"#,
                    "failure-registers"));
                artifacts.capture(&mut stream, &mut reader, "failure", false)
            });
            if let Err(error) = diagnostic {
                artifacts.diagnostic_failure(&error);
            }
            let _ = child.kill();
            if let Ok(status) = child.wait() {
                artifacts.stopped(&status, "harness-kill-after-failure");
            }
        }
    }
    // Artifact errors never replace the original runtime/proof refusal.
    if let Err(error) = artifacts.finish(result.as_ref().err()) {
        if result.is_ok() {
            return Err(error);
        }
        eprintln!("failure artifact error: {error}");
    }
    result
}

fn key_pair(
    qmp: &mut std::os::unix::net::UnixStream,
    reader: &mut super::qmp::Reader,
    key: &str,
    label: &'static str,
) -> Result<(), ConduitosError> {
    hid_qmp::send_named_keys(qmp, reader, &[key], true, label)?;
    hid_qmp::send_named_keys(qmp, reader, &[key], false, label)
}

fn wait_status(
    serial: &std::path::Path,
    child: &mut std::process::Child,
    status: &str,
) -> Result<(), ConduitosError> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let text = fs::read_to_string(serial).map_err(|error| {
            ConduitosError::refusal("product-journey-serial-unavailable", error.to_string())
        })?;
        if journey_records(&text)?
            .iter()
            .any(|record| record.get("status").and_then(Value::as_str) == Some(status))
        {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return hid_qmp::stop(
                child,
                "product-journey-stage-timeout",
                format!("no complete guest record for {status}"),
            );
        }
        thread::sleep(Duration::from_millis(1));
    }
}

fn text(record: &Value, field: &str) -> Result<String, ConduitosError> {
    record
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| ConduitosError::refusal("product-journey-identity-missing", field))
}

fn strings(record: &Value, field: &str) -> Result<Vec<String>, ConduitosError> {
    record
        .get(field)
        .and_then(Value::as_array)
        .and_then(|values| {
            values
                .iter()
                .map(|value| value.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .ok_or_else(|| ConduitosError::refusal("product-journey-identity-missing", field))
}
