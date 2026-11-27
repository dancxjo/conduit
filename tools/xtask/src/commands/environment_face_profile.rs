//! Portable preparation models for exact ordinary Face/selected-Fore inputs.
use std::{error::Error, io::Write, path::Path, process::Command};

pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    for (package, target, minimum) in [
        ("conduit-core", "selected_fore_verification_layout", 3),
        ("conduit-plan-lowering", "owned_fore_line_layout", 3),
        ("conduit-face", "json_model_preflight", 3),
    ] {
        tests(
            root,
            &["test", "--locked", "-p", package, "--test", target],
            minimum,
        )?;
    }
    tests(
        root,
        &[
            "test",
            "--locked",
            "-p",
            "conduit-face",
            "--lib",
            "json_model_preflight::",
        ],
        3,
    )?;
    tests(
        root,
        &[
            "test",
            "--locked",
            "-p",
            "conduit-face",
            "--lib",
            "mask_plot::wire::",
        ],
        4,
    )?;
    tests(
        root,
        &[
            "test",
            "--locked",
            "-p",
            "conduit-face",
            "--test",
            "mask_plot",
            "wire::",
        ],
        3,
    )?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-core",
        "-p",
        "conduit-plan-lowering",
        "-p",
        "conduit-face",
        "--lib",
        "--tests",
        "--",
        "-D",
        "warnings",
    ]))?;
    for (package, target) in [
        ("conduit-core", "thumbv6m-none-eabi"),
        ("conduit-face", "wasm32-unknown-unknown"),
    ] {
        super::execute(Command::new("cargo").current_dir(root).args([
            "check",
            "--locked",
            "-p",
            package,
            "--no-default-features",
            "--target",
            target,
        ]))?;
    }
    Ok(())
}
/// Full raw profile storage qualification, separate from platform enactment.
pub(super) fn runtime(root: &Path) -> Result<(), Box<dyn Error>> {
    for (package, filter, minimum) in [
        (
            "conduit-host-browser-make",
            "line_realizations_bind_exact_contracts_limits_and_outbound_authority",
            1,
        ),
        (
            "conduit-browser-runtime",
            "owner_face_mask::full_profile_pool::tests::",
            3,
        ),
        (
            "conduit-browser-runtime",
            "owner_face_mask::view_memory::tests::",
            3,
        ),
        (
            "conduit-browser-runtime",
            "owner_face_mask::full_ack::tests::",
            1,
        ),
        (
            "conduit-browser-runtime",
            "owner_face_mask::full_refusal::tests::",
            7,
        ),
    ] {
        tests(
            root,
            &["test", "--locked", "-p", package, "--lib", filter],
            minimum,
        )?;
    }
    // Preserve ordinary Body/Face preparation on the native default stack.
    // Keep the browser shard's package feature union for these regressions.
    for filter in [
        "creche::tests::workspace_handoff::stem_play::foreign_fail_job_calls_preserve_the_original_pending_producer",
        "face_nucleus::tests::ordinary_browser_plans_execute_layout_and_graphics_through_the_kernel",
        "plot_runner::application_state::tests::browser_resident_tour_runs_every_chapter_one_exercise_exactly",
    ] {
        tests(root, &[
            "test", "--locked", "-p", "conduit-browser-host", "-p", "conduit-browser-runtime",
            "-p", "conduit-browser-mask-offer", "--lib", filter,
            "--", "--exact", "--test-threads=1",
        ], 1)?;
    }
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-host-browser-make",
        "-p",
        "conduit-browser-mask-offer",
        "-p",
        "conduit-browser-runtime",
        "--lib",
        "--tests",
        "--",
        "-D",
        "warnings",
    ]))?;
    Ok(())
}

fn tests(root: &Path, args: &[&str], minimum: usize) -> Result<(), Box<dyn Error>> {
    let result = Command::new("cargo")
        .current_dir(root)
        .args(args)
        .output()?;
    std::io::stdout().write_all(&result.stdout)?;
    std::io::stderr().write_all(&result.stderr)?;
    if !result.status.success() {
        return Err(format!("Face preparation model proof failed: {}", result.status).into());
    }
    let stdout = String::from_utf8_lossy(&result.stdout);
    let qualified = stdout.lines().any(|line| {
        line.strip_prefix("test result: ok. ")
            .and_then(|line| line.split_once(" passed;"))
            .is_some_and(|(count, rest)| {
                count.parse::<usize>().is_ok_and(|count| count >= minimum)
                    && rest.starts_with(" 0 failed; 0 ignored;")
            })
    });
    if !qualified {
        return Err("Face preparation model proof skipped required tests".into());
    }
    Ok(())
}
