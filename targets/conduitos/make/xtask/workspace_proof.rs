//! Current Workspace acceptance using real guest input and QMP captures.
//! Retired Tour chapter progress is not a substitute for this application.
use super::{
    ConduitosArch, ConduitosError, hid_qmp, journey_input, journey_records, profile::Paths,
    report::git_head,
};
use crate::cli::GlobalOpts;
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

#[derive(serde::Deserialize)]
struct WorkspaceScenario {
    name: String,
    plots: Vec<String>,
    input: String,
    actions: Vec<WorkspaceAction>,
}
#[derive(serde::Deserialize)]
struct WorkspaceAction {
    id: String,
    capture: String,
}

pub(super) fn execute_supplied(
    opts: &GlobalOpts,
    image_path: &Path,
    image_sha256: String,
) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(refusal("real QEMU execution is required"));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let monitor_socket = paths.target.join("workspace-monitor.sock");
    let serial_path = paths.target.join("workspace-serial.log");
    let line_socket = paths.target.join("workspace-usb-line.sock");
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
                ConduitosError::refusal("product-workspace-image-path-invalid", "non-UTF-8 ISO path")
            })?,
            "-boot",
            "d",
        ])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(
            fs::File::create(paths.target.join("workspace-qemu-stderr.log"))
                .map_err(|error| ConduitosError::refusal("qemu-stderr-io", error.to_string()))?,
        );
    let mut artifacts = super::qemu_artifacts::Artifacts::new(
        paths.target.join("workspace-frames"),
        serial_path.clone(),
        serde_json::json!({"source_commit":git_head(&paths.root)?,"image_sha256":image_sha256.clone(),
            "qemu_argv":command.get_args().map(|value|value.to_string_lossy().into_owned()).collect::<Vec<_>>()}),
    )?;
    let mut child = command
        .spawn()
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;

    let result = (|| {
        let (mut qmp, mut reader) = super::qmp::connect_traced(
            &monitor_socket,
            &mut child,
            Some(&paths.target.join("workspace-qmp.log")),
        )?;
        let action_result = (|| {
            let scenario: WorkspaceScenario =
                serde_json::from_str(include_str!("../../../../proof/journeys/workspace.json"))
                    .map_err(|error| refusal(error.to_string()))?;
            let mut completed_actions = Vec::new();
            for action in &scenario.actions {
                match action.id.as_str() {
                    "arrive" => {
                        hid_qmp::wait_for_stage(
                            &serial_path,
                            &mut child,
                            "CONDUIT_BOOT_STAGE front-door-ready",
                            "workspace-arrival-timeout",
                        )?;
                        let before = journey_records::decode(
                            &fs::read_to_string(&serial_path).map_err(io_error)?,
                        )?;
                        if before.iter().any(|record| record["body_id"].is_string()) {
                            return Err(refusal(
                                "arrival already owns a Body before the user births one",
                            ));
                        }
                    }
                    "configure-birth" => {
                        let mut completed_edits = 0;
                        let mut edit = |key: &str| -> Result<(), ConduitosError> {
                            journey_input::key_pair(
                                &mut qmp,
                                &mut reader,
                                key,
                                "workspace-edit-birth",
                            )?;
                            completed_edits += 1;
                            hid_qmp::wait_for_stage_count(
                                &serial_path,
                                &mut child,
                                "CONDUIT_CRECHE_CHECKPOINT edited",
                                completed_edits,
                                "workspace-birth-selection-timeout",
                            )
                        };
                        for character in scenario.name.chars() {
                            edit(&character.to_string())?;
                        }
                        // Ordinary Crèche controls select the same portable plots. Each
                        // completed edit is observed before the next input is submitted.
                        for _ in 0..4 {
                            edit("tab")?;
                        }
                        edit("spc")?; // Omit Keyboard canvas.
                        for _ in 0..3 {
                            edit("tab")?;
                        }
                        edit("spc")?; // Omit Patchbay.
                    }
                    "birth" => {
                        journey_input::key_pair(&mut qmp, &mut reader, "f3", "workspace-birth")?;
                        journey_input::wait_status(&serial_path, &mut child, "born-lulled")?;
                        let born_records = journey_records::decode(
                            &fs::read_to_string(&serial_path).map_err(io_error)?,
                        )?;
                        if born_records.iter().any(|record| {
                            matches!(
                                record["status"].as_str(),
                                Some("awake" | "planned" | "quiescent-awaiting-input")
                            )
                        }) {
                            return Err(refusal(
                                "birth automatically started execution before explicit Wake",
                            ));
                        }
                    }
                    "wake" => {
                        journey_input::key_pair(
                            &mut qmp,
                            &mut reader,
                            "f10",
                            "workspace-tutorial-wake",
                        )?;
                        journey_input::wait_status(
                            &serial_path,
                            &mut child,
                            "quiescent-awaiting-input",
                        )?;
                    }
                    "use-current" => {
                        let before = journey_records::decode(
                            &fs::read_to_string(&serial_path).map_err(io_error)?,
                        )?
                        .len();
                        journey_input::key_pair(
                            &mut qmp,
                            &mut reader,
                            "f10",
                            "workspace-use-current-plot",
                        )?;
                        journey_input::wait_for_record(
                            &serial_path,
                            &mut child,
                            "workspace-selection-timeout",
                            "working Plot selected",
                            |serial| Ok(journey_records::decode(serial)?.len() > before),
                        )?;
                        let mut typed = String::new();
                        for character in scenario.input.chars() {
                            journey_input::key_pair(
                                &mut qmp,
                                &mut reader,
                                &character.to_string(),
                                "workspace-type-text",
                            )?;
                            typed.push(character);
                            journey_input::wait_for_record(
                                &serial_path,
                                &mut child,
                                "workspace-text-timeout",
                                &typed,
                                |serial| {
                                    Ok(journey_records::decode(serial)?.iter().any(|record| {
                                        record["result"].as_str() == Some(typed.as_str())
                                    }))
                                },
                            )?;
                        }
                    }
                    "tutorial" => {
                        let before =
                            views(&fs::read_to_string(&serial_path).map_err(io_error)?)?.len();
                        journey_input::key_pair(
                            &mut qmp,
                            &mut reader,
                            "f9",
                            "workspace-open-tutorial",
                        )?;
                        wait_view_count(&serial_path, &mut child, before + 1)?;
                    }
                    "read-all" => {
                        let current = views(&fs::read_to_string(&serial_path).map_err(io_error)?)?;
                        let pages = current
                            .last()
                            .and_then(|view| view["page_count"].as_u64())
                            .filter(|pages| (1..=32).contains(pages))
                            .ok_or_else(|| refusal("missing finite page count"))?;
                        for page in 1..pages {
                            let count =
                                views(&fs::read_to_string(&serial_path).map_err(io_error)?)?.len();
                            journey_input::key_pair(
                                &mut qmp,
                                &mut reader,
                                "pgdn",
                                "workspace-read-next-page",
                            )?;
                            wait_view_count(&serial_path, &mut child, count + 1)?;
                            let observed =
                                views(&fs::read_to_string(&serial_path).map_err(io_error)?)?;
                            if observed.last().and_then(|view| view["page"].as_u64()) != Some(page)
                            {
                                return Err(refusal("reading did not reach the next current page"));
                            }
                            if page + 1 < pages {
                                artifacts.capture(
                                    &mut qmp,
                                    &mut reader,
                                    &format!("{}-page-{}", action.capture, page + 1),
                                    true,
                                )?;
                            }
                        }
                    }
                    "lull" => {
                        journey_input::key_pair(&mut qmp, &mut reader, "f7", "workspace-lull")?;
                        journey_input::wait_status(&serial_path, &mut child, "lulled")?;
                    }
                    "fulfill" => {
                        journey_input::key_pair(&mut qmp, &mut reader, "end", "workspace-finish")?;
                        journey_input::wait_status(&serial_path, &mut child, "fulfilled")?;
                    }
                    unknown => {
                        return Err(refusal(format!(
                            "unsupported shared Workspace action: {unknown}"
                        )));
                    }
                }
                artifacts.capture(
                    &mut qmp,
                    &mut reader,
                    &action.capture,
                    action.id != "arrive",
                )?;
                completed_actions.push(action.id.clone());
            }
            let serial = fs::read_to_string(&serial_path).map_err(io_error)?;
            let records = journey_records::decode(&serial)?;
            let current_views = views(&serial)?;
            validate(&records, &current_views, &scenario)?;
            if child.try_wait().map_err(io_error)?.is_some() {
                return Err(refusal("guest exited before harness shutdown"));
            }
            let last = records
                .last()
                .ok_or_else(|| refusal("missing final record"))?;
            let proof = json!({
                "schema":"conduit.conduitos/workspace-journey-proof@1",
                "base_commit":git_head(&paths.root)?, "image_sha256":image_sha256,
                "profile_id":last["profile_id"], "build_id":last["build_id"], "image_id":last["image_id"],
                "host_id":last["host_id"], "boot_id":last["boot_id"], "body_id":last["body_id"],
                "proof_class":"freestanding-emulator", "input":"real-qmp-keyboard",
                "same_body_from_birth":true, "birth_requires_explicit_wake":true,
                "result":scenario.input, "application":"tutorial", "actions":completed_actions, "records":records, "views":current_views,
                "pointer_interaction_observed":false, "screen_free_interaction_observed":false,
                "human_observation":false
            });
            fs::write(
                paths.target.join("workspace-proof.json"),
                serde_json::to_vec_pretty(&proof).map_err(|e| refusal(e.to_string()))?,
            )
            .map_err(io_error)
        })();
        if action_result.is_err() {
            if let Err(error) = artifacts.capture(&mut qmp, &mut reader, "failure", false) {
                artifacts.diagnostic_failure(&error);
            }
        }
        action_result
    })();
    if child.try_wait().map_err(io_error)?.is_none() {
        child.kill().map_err(io_error)?;
    }
    let status = child.wait().map_err(io_error)?;
    artifacts.stopped(
        &status,
        if result.is_ok() {
            "harness-stop-after-workspace-actions"
        } else {
            "harness-stop-after-failure"
        },
    );
    artifacts.finish(result.as_ref().err())?;
    result
}

fn views(serial: &str) -> Result<Vec<Value>, ConduitosError> {
    serial
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .filter_map(|line| line.strip_prefix("CONDUIT_WORKSPACE_VIEW "))
        .map(|value| serde_json::from_str(value).map_err(|error| refusal(error.to_string())))
        .collect()
}
fn wait_view_count(
    serial: &Path,
    child: &mut std::process::Child,
    count: usize,
) -> Result<(), ConduitosError> {
    journey_input::wait_for_record(
        serial,
        child,
        "workspace-view-timeout",
        "current Tutorial view",
        |serial| Ok(views(serial)?.len() >= count),
    )
}
fn validate(
    records: &[Value],
    views: &[Value],
    scenario: &WorkspaceScenario,
) -> Result<(), ConduitosError> {
    let born = records
        .iter()
        .position(|record| record["status"] == "born-lulled")
        .ok_or_else(|| refusal("missing real birth"))?;
    let body = records[born]["body_id"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| refusal("missing Body identity"))?;
    let mut after = born;
    for status in ["quiescent-awaiting-input", "lulled", "fulfilled"] {
        after = records
            .iter()
            .enumerate()
            .skip(after + 1)
            .find(|(_, record)| record["status"] == status)
            .map(|(index, _)| index)
            .ok_or_else(|| refusal(format!("missing ordered {status}")))?;
    }
    if records[born..]
        .iter()
        .any(|record| record["body_id"].as_str() != Some(body))
        || !records[born..]
            .iter()
            .any(|record| record["result"].as_str() == Some(scenario.input.as_str()))
    {
        return Err(refusal("work did not occur within the same retained Body"));
    }
    if records[born]["friendly_name"].as_str() != Some(scenario.name.as_str()) {
        return Err(refusal("birth name differs from the shared journey"));
    }
    if views.is_empty()
        || views.iter().any(|view| {
            view["body_id"].as_str() != Some(body)
                || view["application"] != "tutorial"
                || view["view_sha256"].as_str().is_none_or(|id| id.len() != 64)
                || view["presentation_id"].as_str().is_none_or(str::is_empty)
                || view["manifestation_id"].as_str().is_none_or(str::is_empty)
        })
    {
        return Err(refusal(
            "Tutorial view is absent, uncorrelated, or belongs to another Body",
        ));
    }
    let mut expected = scenario.plots.clone();
    expected.sort();
    for view in views {
        let mut observed: Vec<_> = view["plots"]
            .as_array()
            .ok_or_else(|| refusal("missing installed plots"))?
            .iter()
            .map(|plot| plot["title"].as_str().unwrap_or_default().to_owned())
            .collect();
        observed.sort();
        if observed != expected {
            return Err(refusal("installed plots differ from the shared journey"));
        }
    }
    Ok(())
}
fn refusal(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("workspace-journey-refused", detail)
}
fn io_error(error: std::io::Error) -> ConduitosError {
    refusal(error.to_string())
}
