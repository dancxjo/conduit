//! Real QMP input against one exact private owner-provisioned product image.
//! The installed owner and its bounded invitation are prepared independently.

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::{Child, Command, Stdio},
    time::Duration,
};

use serde_json::{json, Value};

use crate::cli::GlobalOpts;

use super::{
    demo, journey_input, owner_boot, profile::Paths, qmp, qmp_display, report::git_head,
    ConduitosArch, ConduitosError, LiveOwnerActionProofArgs, LiveOwnerTodoActionProofArgs,
    LiveOwnerTodoFaceProofArgs,
};

#[path = "owner_action_coordination.rs"]
mod coordination;
use coordination::{wait_for_resume, write_checkpoint};
#[path = "owner_action_validation.rs"]
mod validation;
use validation::{refreshed_show_after_action, validate_success};
#[path = "owner_todo_action_proof.rs"]
mod todo_action;
#[path = "owner_todo_face_validation.rs"]
mod todo_validation;

#[path = "owner_action_observation.rs"]
mod observation;
use observation::*;
#[path = "owner_todo_fallback_proof.rs"]
mod todo_fallback;
#[path = "owner_todo_look_proof.rs"]
mod todo_look;

const MAX_SERIAL_BYTES: u64 = 2 * 1024 * 1024;
const OWNER_FACE: &str = "CONDUIT_NATIVE_OWNER_FACE ";
const GUEST_PART: &str = "CONDUIT_NATIVE_GUEST_PART ";
const OWNER_ACTION: &str = "CONDUIT_NATIVE_OWNER_ACTION ";

pub(super) fn execute(
    args: &LiveOwnerActionProofArgs,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    execute_mode(args, opts, None, None)
}

pub(super) fn execute_todo_face(
    args: &LiveOwnerTodoFaceProofArgs,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    if !args.route.coordinate || args.expected_body_id.is_empty() || args.expected_status.is_empty()
    {
        return Err(refusal("native-todo-face-requires-coordinated-body"));
    }
    execute_mode(&args.route, opts, Some(args), None)
}

pub(super) fn execute_todo_action(
    args: &LiveOwnerTodoActionProofArgs,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    if args.face.look_selection
        || !args.face.route.coordinate
        || args.face.expected_body_id.is_empty()
        || args.face.expected_status.is_empty()
        || args.expected_after_status.is_empty()
        || !(1..=64).contains(&args.tab_count)
        || !args.expected_action_id.starts_with("todo.")
    {
        return Err(refusal(
            "native-todo-action-requires-bounded-coordinated-body",
        ));
    }
    if args.required_item_fallback
        && (args.expected_action_id != "todo.complete.task-1"
            || args
                .fallback_action_id
                .as_deref()
                .is_none_or(|id| !id.starts_with("todo.") || id == args.expected_action_id)
            || args
                .fallback_tab_count
                .is_none_or(|tabs| !(1..=64).contains(&tabs))
            || args.fallback_status.as_deref().is_none_or(str::is_empty))
    {
        return Err(refusal(
            "native-todo-fallback-requires-two-explicit-actions",
        ));
    }
    execute_mode(&args.face.route, opts, Some(&args.face), Some(args))
}

fn execute_mode(
    args: &LiveOwnerActionProofArgs,
    opts: &GlobalOpts,
    expected_todo: Option<&LiveOwnerTodoFaceProofArgs>,
    expected_todo_action: Option<&LiveOwnerTodoActionProofArgs>,
) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(refusal("native-owner-proof-requires-live-input"));
    }
    let directory = fs::canonicalize(&args.output_dir).map_err(|error| {
        ConduitosError::refusal("native-owner-proof-directory", error.to_string())
    })?;
    let metadata = fs::metadata(&directory).map_err(|error| {
        ConduitosError::refusal("native-owner-proof-directory", error.to_string())
    })?;
    if !metadata.is_dir()
        || metadata.permissions().mode() & 0o077 != 0
        || fs::read_dir(&directory)
            .map_err(|error| {
                ConduitosError::refusal("native-owner-proof-directory", error.to_string())
            })?
            .next()
            .is_some()
    {
        return Err(refusal(
            "native-owner-proof-directory-must-be-empty-private",
        ));
    }
    let route = owner_boot::prepare(&args.spore, &args.candidate_id, args.owner_forward)?;
    let root = Paths::new(ConduitosArch::X86_64)?.root;
    let source = git_head(&root)?;
    if source != route.source_identity || !clean_source(&root)? {
        return Err(refusal("native-owner-proof-source-not-exact"));
    }
    if let Some(args) = expected_todo.filter(|args| args.look_selection) {
        todo_look::validate_installation(args, &source)?;
    }
    let isolated = expected_todo_action
        .map(|action| todo_action::validate_isolated_installation(action, &source))
        .transpose()?;
    let spore = fs::canonicalize(&args.spore)
        .map_err(|error| ConduitosError::refusal("native-owner-proof-spore", error.to_string()))?;
    let spore_str = spore
        .to_str()
        .ok_or_else(|| refusal("native-owner-proof-spore-utf8"))?;
    let serial_path = directory.join("guest.serial.log");
    let qmp_path = directory.join("qmp.sock");
    let qmp_arg = owner_boot::qmp_arg(&qmp_path)?;
    let serial_arg = format!("file:{}", serial_path.display());
    let mut qemu_args: Vec<String> =
        demo::qemu_args(spore_str, Some(&route.netdev), Some(&qmp_arg))
            .into_iter()
            .map(str::to_owned)
            .collect();
    replace_qemu_option(&mut qemu_args, "-display", "none")?;
    replace_qemu_option(&mut qemu_args, "-serial", &serial_arg)?;
    let stderr = fs::File::create(directory.join("qemu.stderr")).map_err(|error| {
        ConduitosError::refusal("native-owner-proof-artifact", error.to_string())
    })?;
    let mut child = Command::new("qemu-system-x86_64")
        .args(&qemu_args)
        .current_dir(&directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|error| ConduitosError::refusal("native-owner-proof-qemu", error.to_string()))?;
    let result = prove(
        &directory,
        &serial_path,
        &qmp_path,
        &mut child,
        &route,
        &qemu_args,
        args.coordinate,
        expected_todo,
        expected_todo_action,
        isolated.as_ref(),
    );
    let _ = child.kill();
    let _ = child.wait();
    let receipt = result?;
    let path = directory.join(if expected_todo_action.is_some() {
        "owner-todo-action-proof.json"
    } else if expected_todo.is_some() {
        "owner-todo-face-proof.json"
    } else {
        "owner-action-proof.json"
    });
    fs::write(
        &path,
        serde_json::to_vec_pretty(&receipt).map_err(|error| {
            ConduitosError::refusal("native-owner-proof-encode", error.to_string())
        })?,
    )
    .map_err(|error| ConduitosError::refusal("native-owner-proof-artifact", error.to_string()))?;
    if opts.json {
        println!("{receipt}");
    } else if !opts.quiet {
        println!("Native owner action proof: {}", path.display());
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "one local proof invocation carries exact QMP and owner evidence"
)]
fn prove(
    directory: &std::path::Path,
    serial_path: &std::path::Path,
    qmp_path: &std::path::Path,
    child: &mut Child,
    route: &owner_boot::PreparedOwnerBoot,
    qemu_args: &[String],
    coordinate: bool,
    expected_todo: Option<&LiveOwnerTodoFaceProofArgs>,
    expected_todo_action: Option<&LiveOwnerTodoActionProofArgs>,
    isolated: Option<&todo_action::Isolation>,
) -> Result<Value, ConduitosError> {
    let (mut qmp, mut reader) =
        qmp::connect_traced(qmp_path, child, Some(&directory.join("qmp.jsonl")))?;
    let (standby_part, standby_face, return_route_available) = wait_for_standby(
        serial_path,
        child,
        Duration::from_secs(120),
        expected_todo.is_some(),
    )?;
    if let Some(args) = expected_todo {
        if standby_part["body_id"] != args.expected_body_id {
            return Err(refusal("native-todo-face-body-mismatch"));
        }
    }
    let todo_face = expected_todo
        .map(|args| todo_validation::read_and_validate(args, &standby_face))
        .transpose()?;
    let (standby_image, health) =
        qmp_display::capture(&mut qmp, &mut reader, directory, "owner-standby")?;
    if let Some(error) = health {
        return Err(error);
    }
    if let (Some(todo_face), false) = (&todo_face, return_route_available) {
        if expected_todo.is_some_and(|args| args.look_selection) {
            return Err(refusal("native-todo-look-requires-owner-return-route"));
        }
        if child
            .try_wait()
            .map_err(|error| ConduitosError::refusal("native-todo-face-qemu", error.to_string()))?
            .is_some()
        {
            return Err(refusal("native-todo-face-guest-not-live-at-capture"));
        }
        return Ok(json!({
            "schema":"conduit.conduitos/native-todo-face-proof@1",
            "proof_class":"live-local-qmp-installed-owner-read-only",
            "source_commit":route.source_identity,
            "spore_build_id":route.build_id,
            "spore_sha256":route.artifact_sha256,
            "candidate_id":route.candidate_id,
            "reachability":route.reachability,
            "qemu_argv":qemu_args,
            "guest_part":standby_part,
            "owner_todo_face":todo_face,
            "face_shown":standby_face,
            "show_ack":null,
            "native_return_route_available":false,
            "interactions_admitted":false,
            "screenshots":[standby_image],
            "qemu_alive_at_capture":true,
            "coordinated":false,
            "mutations":0,
        }));
    }
    if coordinate {
        write_checkpoint(
            directory,
            "native-standby.json",
            &json!({
                "schema":"conduit.conduitos/native-owner-coordination@1",
                "stage":"standby",
                "guest_part":standby_part.clone(),
                "face":standby_face.clone(),
            }),
        )?;
        wait_for_resume(
            directory,
            "resume-native-activation",
            child,
            Duration::from_secs(35 * 60),
        )?;
    }
    // An actual native user requests this Mask after the owner has selected
    // its admitted route. No policy changes occur in the guest or harness.
    journey_input::key_pair(&mut qmp, &mut reader, "f5", "native-owner-activate")?;
    let (part, before, before_ack) =
        wait_for_arrival(serial_path, child, Duration::from_secs(120))?;
    if part != standby_part
        || (expected_todo_action.is_none() && before.get("face_id") != standby_face.get("face_id"))
    {
        return Err(refusal("native-owner-standby-basis-changed"));
    }
    let (before_image, health) =
        qmp_display::capture(&mut qmp, &mut reader, directory, "owner-before")?;
    if let Some(error) = health {
        return Err(error);
    }
    if let Some(args) = expected_todo_action {
        return todo_action::continue_after_arrival(
            args,
            isolated.ok_or_else(|| refusal("native-todo-action-isolation-absent"))?,
            directory,
            serial_path,
            child,
            &mut qmp,
            &mut reader,
            route,
            qemu_args,
            &standby_part,
            &standby_face,
            &part,
            &before,
            &before_ack,
            &standby_image,
            &before_image,
        );
    }
    if let Some(args) = expected_todo {
        let look_selection = if args.look_selection {
            Some(todo_look::exercise(
                args,
                directory,
                serial_path,
                child,
                &mut qmp,
                &mut reader,
                &before,
            )?)
        } else {
            None
        };
        if child
            .try_wait()
            .map_err(|error| ConduitosError::refusal("native-todo-face-qemu", error.to_string()))?
            .is_some()
        {
            return Err(refusal("native-todo-face-guest-not-live-at-capture"));
        }
        return Ok(json!({
            "schema":"conduit.conduitos/native-todo-face-proof@1",
            "proof_class":"live-local-qmp-installed-owner-read-only",
            "source_commit":route.source_identity,
            "spore_build_id":route.build_id,
            "spore_sha256":route.artifact_sha256,
            "candidate_id":route.candidate_id,
            "reachability":route.reachability,
            "qemu_argv":qemu_args,
            "guest_part":part,
            "face_standby":standby_face,
            "owner_todo_face":todo_face,
            "face_shown":before,
            "show_ack":before_ack,
            "screenshots":[standby_image,before_image],
            "qemu_alive_at_capture":true,
            "coordinated":coordinate,
            "mutations":0,
            "look_selection":look_selection,
        }));
    }
    if coordinate {
        write_checkpoint(
            directory,
            "native-arrived.json",
            &json!({
                "schema":"conduit.conduitos/native-owner-coordination@1",
                "stage":"arrived",
                "guest_part":part,
                "face":before,
                "show_ack":before_ack,
            }),
        )?;
        wait_for_resume(
            directory,
            "resume-native-action",
            child,
            Duration::from_secs(120),
        )?;
    }
    // The first available control is Wake; the second is the checked clock
    // interval argument. Traverse both through the ordinary native Mask.
    for key in ["tab", "tab", "5", "0", "0", "ret"] {
        journey_input::key_pair(&mut qmp, &mut reader, key, "native-owner-clock-input")?;
    }
    let (action, after, after_ack) = wait_for_action(serial_path, child, Duration::from_secs(30))?;
    let (after_image, health) =
        qmp_display::capture(&mut qmp, &mut reader, directory, "owner-after")?;
    if let Some(error) = health {
        return Err(error);
    }
    validate_success(&before, &before_ack, &action, &after, &after_ack)?;
    if coordinate {
        write_checkpoint(
            directory,
            "native-action.json",
            &json!({
                "schema":"conduit.conduitos/native-owner-coordination@1",
                "stage":"action",
                "action":action,
                "face":after,
                "show_ack":after_ack,
            }),
        )?;
        // The guest must remain alive while the owner completes a full spoken
        // Face and the separate model chapters before the final QMP receipt.
        wait_for_resume(
            directory,
            "resume-native-finish",
            child,
            Duration::from_secs(45 * 60),
        )?;
    }
    if child
        .try_wait()
        .map_err(|error| ConduitosError::refusal("native-owner-proof-qemu", error.to_string()))?
        .is_some()
    {
        return Err(refusal("native-owner-guest-not-live-at-capture"));
    }
    Ok(json!({
        "schema":"conduit.conduitos/native-owner-action-proof@1",
        "proof_class":"live-local-qmp-installed-owner",
        "source_commit":route.source_identity,
        "spore_build_id":route.build_id,
        "spore_sha256":route.artifact_sha256,
        "candidate_id":route.candidate_id,
        "reachability":route.reachability,
        "qemu_argv":qemu_args,
        "guest_part":part,
        "face_before":before,
        "show_ack_before":before_ack,
        "action":action,
        "face_after":after,
        "show_ack_after":after_ack,
        "screenshots":[standby_image,before_image,after_image],
        "qemu_alive_at_capture":true,
        "coordinated":coordinate,
    }))
}
