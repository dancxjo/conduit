//! Original recording capture and selected Root construction; CPU proof only.
use std::{error::Error, io::Write, path::Path, process::Command};

pub(super) fn prove(root: &Path, selected_host: bool) -> Result<(), Box<dyn Error>> {
    if !cfg!(target_os = "linux") {
        return Err("original recording Source qualification requires Linux".into());
    }
    if selected_host {
        return tests(
            root,
            &["test", "--locked", "-p", "conduit", "--bin", "conduit",
                "selected_host_configuration::environment_recording_history_tests::",
                "--", "--test-threads=1"],
            2,
            &["ordinary_selection_initializes_readonly_boot_original_json_with_canonical_image",
                "ordinary_selection_missing_record_and_exact_reference_mismatch_refuse_without_ready_host"],
        );
    }
    tests(
        root,
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--lib",
            "environment_recording_history::",
            "--",
            "--test-threads=1",
        ],
        13,
        &[
            "closed_selected_image_freezes_original_recording_and_commits_two_channels",
            "actual_recording_becomes_two_exact_channels_then_reads_without_io_or_allocation",
            "explicit_eight_event_view_preserves_all_outcomes_and_dense_sixteen_refuses_extent",
            "independent_read_authority_and_bounded_metadata_are_required_before_capture",
            "template_defaults_do_not_supply_omitted_checked_initialization_fields",
        ],
    )?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-semantic-catalog",
        "--test",
        "environment_recording_history",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-host-hosted",
        "--lib",
        "environment_recording_history_initialization::",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-std-host",
        "--lib",
        "--",
        "-D",
        "warnings",
    ]))
}

fn tests(
    root: &Path,
    args: &[&str],
    minimum: usize,
    required: &[&str],
) -> Result<(), Box<dyn Error>> {
    tests_with_env(root, args, minimum, required, &[])
}

fn tests_with_env(
    root: &Path,
    args: &[&str],
    minimum: usize,
    required: &[&str],
    env: &[(&str, &Path)],
) -> Result<(), Box<dyn Error>> {
    println!(
        "recording_source_phase={}",
        if args.contains(&"conduit-std-host") {
            "installed"
        } else {
            "selected-root"
        }
    );
    let mut command = Command::new("cargo");
    command.current_dir(root).args(args);
    for (key, value) in env {
        command.env(key, value);
    }
    let result = command.output()?;
    std::io::stdout().write_all(&result.stdout)?;
    std::io::stderr().write_all(&result.stderr)?;
    if !result.status.success() {
        return Err(format!("recording Source proof failed: {}", result.status).into());
    }
    let stdout = String::from_utf8_lossy(&result.stdout);
    let summary = stdout.lines().any(|line| {
        line.strip_prefix("test result: ok. ")
            .and_then(|line| line.split_once(" passed;"))
            .is_some_and(|(count, rest)| {
                count.parse::<usize>().is_ok_and(|count| count >= minimum)
                    && rest.starts_with(" 0 failed; 0 ignored;")
            })
    });
    if !summary
        || required.iter().any(|name| {
            !stdout.lines().any(|line| {
                line.starts_with("test ") && line.contains(name) && line.ends_with(" ... ok")
            })
        })
    {
        return Err(
            "recording Source proof did not execute its required unignored contracts".into(),
        );
    }
    Ok(())
}

/// Fresh ordinary Owner and native rendering, then unchanged browser CPU input.
pub(super) fn owner_mask(root: &Path) -> Result<(), Box<dyn Error>> {
    if !cfg!(target_os = "linux") {
        return Err("recording Owner qualification requires Linux".into());
    }
    let run = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let directory = root
        .join("target/environment-recording-source")
        .join(format!("owner-{}-{run}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    let face = directory.join("owner-face.json");
    let view = directory.join("browser-view.json");
    let face_env = "CONDUIT_ENVIRONMENT_HISTORY_OWNER_FACE";
    tests_with_env(
        root,
        &[
            "test",
            "--locked",
            "-p",
            "conduit",
            "--bin",
            "conduit",
            "original_recording_source_reaches_actual_owner_and_ordinary_native_scene",
            "--",
            "--test-threads=1",
        ],
        1,
        &["original_recording_source_reaches_actual_owner_and_ordinary_native_scene"],
        &[(face_env, face.as_path())],
    )?;
    if !face.is_file() {
        return Err("ordinary Owner did not produce its fresh complete Face".into());
    }
    tests_with_env(root, &[
        "test", "--locked", "-p", "conduit-browser-runtime", "--lib",
        "owner_face_mask::environment_history_owner_tests::original_recording_owner_face_preserves_complete_reading_and_shared_series",
        "--", "--ignored", "--exact", "--test-threads=1",
    ], 1, &["original_recording_owner_face_preserves_complete_reading_and_shared_series"],
        &[(face_env, face.as_path()), ("CONDUIT_ENVIRONMENT_HISTORY_OWNER_VIEW", view.as_path())])?;
    println!(
        "recording_owner_face={} browser_view={}",
        face.display(),
        view.display()
    );
    Ok(())
}
