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

use super::{
    hid_qmp, image, journey_input, profile::Paths, report::git_head, ConduitosArch, ConduitosError,
};

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
    tour_specimen_id: String,
    tour_source_document_id: String,
    tour_checked_form_id: String,
    tour_expanded_form_id: String,
    tour_plan_id: String,
    tour_active_play_id: String,
    tour_result: String,
    tour_workspace_presentation_id: String,
    tour_workspace_manifestation_id: String,
    tour_status_presentation_id: String,
    tour_status_manifestation_id: String,
    pointer_hover_subject: String,
    pointer_selected_subject: String,
    pointer_press_sequence: u64,
    pointer_release_sequence: u64,
    inspector_presentation_id: String,
    inspector_manifestation_id: String,
    inspector_focused_manifestation_id: String,
    transient_kinds: Vec<String>,
    transient_refusal_cause: String,
    chooser_manifestation_id: String,
    transient_stale_input_refused: bool,
    usb_line_id: String,
    usb_line_binding_id: String,
    usb_line_plan_id: String,
    usb_line_source_active_play_id: String,
    usb_line_sink_active_play_id: String,
    usb_line_value: String,
    usb_line_membership: String,
    usb_line_body_unchanged: bool,
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
    let line_socket = paths.target.join("journey-usb-line.sock");
    let proof_path = paths.target.join("journey-proof.json");
    let _ = fs::remove_file(&monitor_socket);
    let _ = fs::remove_file(&serial_path);
    let _ = fs::remove_file(&line_socket);
    let monitor = format!(
        "unix:{},server=on,wait=off",
        monitor_socket.to_string_lossy()
    );
    let serial = format!("file:{}", serial_path.to_string_lossy());
    let line_chardev = format!(
        "socket,id=conduitos-usb-line-chardev,path={},server=on,wait=off",
        line_socket.to_string_lossy()
    );
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
            "qemu-xhci,id=conduitos-xhci,p2=3,p3=0",
            "-device",
            "usb-kbd,id=conduitos-keyboard,bus=conduitos-xhci.0,port=1",
            "-device",
            "usb-mouse,id=conduitos-pointer,bus=conduitos-xhci.0,port=2",
            "-chardev",
            &line_chardev,
            "-device",
            "usb-serial,id=conduitos-usb-line,bus=conduitos-xhci.0,port=3,chardev=conduitos-usb-line-chardev",
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
            let pending_line_peer = super::journey_usb_line::PendingPeer::connect(&line_socket)?;
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
                journey_input::key_pair(&mut qmp, &mut reader, key, status)?;
                journey_input::wait_status(&serial_path, &mut child, status)?;
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
                journey_input::key_pair(&mut qmp, &mut reader, "f2", "planned-detail")?;
                hid_qmp::wait_for_stage(
                    &serial_path,
                    &mut child,
                    &format!("\"label\":\"{label}\""),
                    "product-journey-plan-inspection-timeout",
                )?;
            }
            journey_input::key_pair(&mut qmp, &mut reader, "esc", "leave-details")?;
            journey_input::key_pair(&mut qmp, &mut reader, "f6", "playing")?;
            journey_input::wait_status(&serial_path, &mut child, "playing")?;
            artifacts.capture(&mut qmp, &mut reader, "playing", true)?;
            journey_input::key_pair(&mut qmp, &mut reader, "a", "semantic-input")?;
            journey_input::wait_status(&serial_path, &mut child, "result-visible")?;
            artifacts.capture(&mut qmp, &mut reader, "result-visible", true)?;
            journey_input::key_pair(&mut qmp, &mut reader, "f7", "lull")?;
            journey_input::wait_status(&serial_path, &mut child, "lulled")?;
            artifacts.capture(&mut qmp, &mut reader, "lulled", true)?;
            journey_input::key_pair(&mut qmp, &mut reader, "f12", "usb-line")?;
            hid_qmp::wait_for_stage(
                &serial_path,
                &mut child,
                "CONDUIT_BOOT_STAGE usb-line-current",
                "product-journey-usb-line-current-timeout",
            )?;
            artifacts.capture(&mut qmp, &mut reader, "usb-line-current", true)?;
            let mut line_peer = pending_line_peer.activate()?;
            hid_qmp::wait_for_stage(
                &serial_path,
                &mut child,
                "CONDUIT_BOOT_STAGE peer-attached",
                "product-journey-usb-line-peer-timeout",
            )?;
            artifacts.capture(&mut qmp, &mut reader, "peer-attached", true)?;
            line_peer.receive_value_and_acknowledge()?;
            hid_qmp::wait_for_stage(
                &serial_path,
                &mut child,
                "CONDUIT_BOOT_STAGE line-value-visible",
                "product-journey-usb-line-value-timeout",
            )?;
            artifacts.capture(&mut qmp, &mut reader, "line-value-visible", true)?;
            super::qmp::request_value(
                &mut qmp,
                &mut reader,
                br#"{"execute":"device_del","arguments":{"id":"conduitos-usb-line"}}"#,
                "usb-line-remove",
            )?;
            hid_qmp::wait_for_stage(
                &serial_path,
                &mut child,
                "CONDUIT_BOOT_STAGE line-lost",
                "product-journey-usb-line-loss-timeout",
            )?;
            artifacts.capture(&mut qmp, &mut reader, "line-lost", true)?;
            drop(line_peer);
            journey_input::key_pair(&mut qmp, &mut reader, "f9", "tour-open")?;
            journey_input::wait_tour_status(&serial_path, &mut child, "tour-opened")?;
            artifacts.capture(&mut qmp, &mut reader, "tour-opened", true)?;
            journey_input::key_pair(&mut qmp, &mut reader, "f10", "tour-run")?;
            journey_input::wait_tour_status(&serial_path, &mut child, "result-visible")?;
            artifacts.capture(&mut qmp, &mut reader, "tour-result-visible", true)?;
            journey_input::wait_transient_status(&serial_path, &mut child, "shown")?;
            artifacts.capture(&mut qmp, &mut reader, "confirmation-transient", true)?;
            journey_input::key_pair(&mut qmp, &mut reader, "esc", "dismiss-confirmation")?;
            journey_input::wait_transient_status(&serial_path, &mut child, "dismissed")?;
            artifacts.capture(&mut qmp, &mut reader, "confirmation-dismissed", true)?;
            journey_input::key_pair(&mut qmp, &mut reader, "f10", "refused-repeat-run")?;
            hid_qmp::wait_for_stage(
                &serial_path,
                &mut child,
                "CONDUIT_TOUR_CHECKPOINT refusal-transient-shown",
                "product-journey-refusal-transient-timeout",
            )?;
            artifacts.capture(&mut qmp, &mut reader, "refusal-transient", true)?;
            journey_input::key_pair(&mut qmp, &mut reader, "esc", "dismiss-refusal")?;
            hid_qmp::wait_for_stage(
                &serial_path,
                &mut child,
                "CONDUIT_TOUR_CHECKPOINT transient-dismissed",
                "product-journey-refusal-dismissal-timeout",
            )?;
            artifacts.capture(&mut qmp, &mut reader, "refusal-dismissed", true)?;
            journey_input::key_pair(&mut qmp, &mut reader, "f11", "tour-patchbay")?;
            journey_input::wait_tour_status(&serial_path, &mut child, "patchbay-open")?;
            hid_qmp::wait_for_stage(
                &serial_path,
                &mut child,
                "CONDUIT_TOUR_CHECKPOINT chooser-transient-shown",
                "product-journey-chooser-transient-timeout",
            )?;
            artifacts.capture(&mut qmp, &mut reader, "tour-patchbay-open", true)?;
            hid_qmp::wait_for_stage(
                &serial_path,
                &mut child,
                "CONDUIT_BOOT_STAGE pointer-awaiting-report",
                "product-journey-pointer-ready-timeout",
            )?;
            journey_input::primary_button(&mut qmp, &mut reader, true, "pointer-chooser")?;
            journey_input::wait_pointer_status(&serial_path, &mut child, "transient-focused")?;
            artifacts.capture(&mut qmp, &mut reader, "chooser-pointer-focused", true)?;
            journey_input::primary_button(&mut qmp, &mut reader, false, "pointer-chooser-release")?;
            journey_input::relative_motion(&mut qmp, &mut reader, 0, -100, "pointer-hover")?;
            journey_input::wait_pointer_status(&serial_path, &mut child, "hovered")?;
            artifacts.capture(&mut qmp, &mut reader, "pointer-hover-or-focus", true)?;
            journey_input::primary_button(&mut qmp, &mut reader, true, "pointer-select")?;
            journey_input::wait_pointer_status(&serial_path, &mut child, "selected")?;
            artifacts.capture(&mut qmp, &mut reader, "pointer-selected", true)?;
            journey_input::primary_button(&mut qmp, &mut reader, false, "pointer-release")?;
            journey_input::wait_pointer_status_count(&serial_path, &mut child, "hovered", 2)?;
            journey_input::relative_motion(&mut qmp, &mut reader, 120, 0, "pointer-inspector")?;
            journey_input::primary_button(&mut qmp, &mut reader, true, "pointer-focus-inspector")?;
            journey_input::wait_pointer_status(&serial_path, &mut child, "auxiliary-focused")?;
            artifacts.capture(&mut qmp, &mut reader, "inspector-focused", true)?;
            journey_input::primary_button(
                &mut qmp,
                &mut reader,
                false,
                "pointer-release-inspector",
            )?;
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
        let tour_records = super::journey_records::tour(&serial)?;
        let pointer_records = super::journey_records::pointer(&serial)?;
        let transient_records = super::journey_records::transient(&serial)?;
        let usb_line_records = super::journey_records::usb_line(&serial)?;
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
        let tour_by_status = tour_records
            .iter()
            .filter_map(|record| Some((record.get("status")?.as_str()?.to_owned(), record)))
            .collect::<BTreeMap<_, _>>();
        super::journey_tour::validate(&tour_records, opened)?;
        let tour_opened = tour_by_status["tour-opened"];
        let tour_result = tour_by_status["result-visible"];
        if usb_line_records.len() != 4 {
            return Err(ConduitosError::refusal(
                "product-journey-usb-line-record-count",
                "current, peer, value, and loss must produce exactly four Line records",
            ));
        }
        for (record, status) in usb_line_records.iter().zip([
            "usb-line-current",
            "peer-attached",
            "line-value-visible",
            "line-lost",
        ]) {
            if record.get("status").and_then(Value::as_str) != Some(status)
                || record.get("proof_class").and_then(Value::as_str)
                    != Some("freestanding-emulator")
                || record.get("membership").and_then(Value::as_str) != Some("not-requested")
            {
                return Err(ConduitosError::refusal(
                    "product-journey-usb-line-stage-invalid",
                    status,
                ));
            }
        }
        for identity in [
            "line_id",
            "binding_id",
            "base_instance_id",
            "plan_id",
            "source_active_play_id",
            "sink_active_play_id",
            "source_host_id",
            "source_boot_id",
            "sink_host_id",
            "sink_boot_id",
            "body_id",
        ] {
            if usb_line_records
                .iter()
                .any(|record| record.get(identity) != usb_line_records[0].get(identity))
            {
                return Err(ConduitosError::refusal(
                    "product-journey-usb-line-identity-drift",
                    identity,
                ));
            }
        }
        if usb_line_records[2].get("value").and_then(Value::as_str) != Some("HELLO USB LINE")
            || usb_line_records[0].get("body_id") != by_status["lulled"].get("body_id")
        {
            return Err(ConduitosError::refusal(
                "product-journey-usb-line-causality-invalid",
                "value or unchanged Body correlation did not match",
            ));
        }
        let ordinary_pointer_records = pointer_records
            .iter()
            .filter(|record| {
                record.get("status").and_then(Value::as_str) != Some("transient-focused")
            })
            .cloned()
            .collect::<Vec<_>>();
        let pointer = super::journey_pointer::validate(&ordinary_pointer_records, opened)?;
        let transient =
            super::journey_transient::validate(&transient_records, &pointer_records, opened)?;
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
            profile: "q35-single-cpu-64m-headless-xhci-usb-kbd-usb-mouse-usb-ftdi-adlib",
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
            tour_specimen_id: text(tour_opened, "specimen_id")?,
            tour_source_document_id: text(tour_result, "source_document_id")?,
            tour_checked_form_id: text(tour_result, "checked_form_id")?,
            tour_expanded_form_id: text(tour_result, "expanded_form_id")?,
            tour_plan_id: text(tour_result, "plan_id")?,
            tour_active_play_id: text(tour_result, "active_play_id")?,
            tour_result: text(tour_result, "result")?,
            tour_workspace_presentation_id: text(tour_result, "workspace_presentation_id")?,
            tour_workspace_manifestation_id: text(tour_result, "workspace_manifestation_id")?,
            tour_status_presentation_id: text(tour_result, "status_presentation_id")?,
            tour_status_manifestation_id: text(tour_result, "status_manifestation_id")?,
            pointer_hover_subject: pointer.hover_subject,
            pointer_selected_subject: pointer.selected_subject,
            pointer_press_sequence: pointer.press_sequence,
            pointer_release_sequence: pointer.release_sequence,
            inspector_presentation_id: pointer.inspector_presentation_id,
            inspector_manifestation_id: pointer.inspector_manifestation_id,
            inspector_focused_manifestation_id: pointer.focused_manifestation_id,
            transient_kinds: transient.kinds,
            transient_refusal_cause: transient.refusal_cause,
            chooser_manifestation_id: transient.chooser_manifestation_id,
            transient_stale_input_refused: transient.stale_input_refused,
            usb_line_id: text(&usb_line_records[0], "line_id")?,
            usb_line_binding_id: text(&usb_line_records[0], "binding_id")?,
            usb_line_plan_id: text(&usb_line_records[0], "plan_id")?,
            usb_line_source_active_play_id: text(&usb_line_records[0], "source_active_play_id")?,
            usb_line_sink_active_play_id: text(&usb_line_records[0], "sink_active_play_id")?,
            usb_line_value: text(&usb_line_records[2], "value")?,
            usb_line_membership: text(&usb_line_records[0], "membership")?,
            usb_line_body_unchanged: true,
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
