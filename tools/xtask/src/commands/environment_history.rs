//! Pure recorded-history Face and Mask preparation; no live Host claim.
use std::{error::Error, path::Path, process::Command};
pub(super) fn dispatch(root: &Path, args: &[String]) -> Option<Result<(), Box<dyn Error>>> {
    match args {
        [name] if name == "history" => Some(prove(root)),
        [name] if name == "history-look" => Some(look(root)),
        [name] if name == "history-preferences" => Some(preferences(root)),
        [name] if name == "history-commit" => Some(committed(root)),
        [name] if name == "history-native" => Some(native(root)),
        [name] if name == "history-installed-native" => Some(installed_native(root)),
        [name] if name == "history-installed-native-target" => Some(installed_native_target(root)),
        [name] if name == "history-installed-browser" => Some(installed_browser(root)),
        [name] if name == "history-owner-control-target" => Some(owner_control_target(root)),
        [name] if name == "history-owner-control" => Some(owner_control(root)),
        [name] if name == "history-native-controls" => Some(native_controls(root)),
        [name] if name == "history-native-unit-preview" => Some(native_unit_preview(root)),
        [name] if name == "history-target" => Some(target(root)),
        [name] if name == "history-browser" => Some(browser(root)),
        [name] if name == "history-owner-browser" => Some(owner_browser(root)),
        [name] if name == "history-owner-target" => Some(owner_target(root)),
        [name] if name == "history-cross-mask" => Some(cross_mask(root)),
        [name] if name == "history-preview" => Some(preview(root)),
        _ => None,
    }
}
fn native_unit_preview(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "test", "--locked", "-p", "conduitos", "--lib", "--features",
        "native-compositor",
        "native_face_mask::history_tests::independent_history_looks_use_one_body_face_and_exact_routed_kernel_scanout",
    ]))
}
fn native_controls(root: &Path) -> Result<(), Box<dyn Error>> {
    for filter in [
        "native_face_mask::",
        "native_owner_return::refresh::",
        "product_front_door::face_arrival::history_selection::",
        "product_front_door::owner_selection::tests",
        "product_front_door::face_arrival::selection::tests",
        "product_front_door::face_arrival::owner_selection::tests",
    ] {
        super::execute(Command::new("cargo").current_dir(root).args([
            "test",
            "--locked",
            "-p",
            "conduitos",
            "--lib",
            "--features",
            "native-compositor",
            filter,
        ]))?;
    }
    Ok(())
}
fn owner_control(root: &Path) -> Result<(), Box<dyn Error>> {
    for filter in [
        "native_mask_route::selection::tests",
        "remote_native_mask_accepts_only_the_offered_scoped_present_binding",
    ] {
        super::execute(Command::new("cargo").current_dir(root).args([
            "test", "--locked", "-p", "conduit", "--bin", "conduit", filter,
        ]))?;
    }
    Ok(())
}
fn owner_control_target(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(
        Command::new("cargo")
            .current_dir(root)
            .args(["check", "--locked", "-p", "conduit", "--bin", "conduit"]),
    )
}
fn installed_browser(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("node").current_dir(root).args([
        "--test",
        "targets/browser/host/assets/browser-membership-wardrobe.test.mjs",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-std-host",
        "--lib",
        "browser_admission::component_selection",
    ]))?;
    super::execute(
        Command::new("cargo")
            .current_dir(root)
            .args([
                "test",
                "--locked",
                "-p",
                "conduit-browser-runtime",
                "--lib",
                "owner_face_mask::history_tests::owner_entry",
            ])
            .env("CONDUIT_HISTORY_BROWSER_PROOF_OUTPUT", browser_output(root)),
    )?;
    super::execute(
        Command::new("cargo")
            .current_dir(root)
            .args(["check", "--locked", "-p", "conduit", "--bin", "conduit"]),
    )
}
fn installed_native(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-conduitos-mask-offer",
        "--lib",
        "history",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduitos",
        "--lib",
        "--features",
        "native-compositor-core",
        "native_face_mask::",
    ]))
}
fn installed_native_target(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "check",
        "--locked",
        "-p",
        "conduitos",
        "--lib",
        "--target",
        "x86_64-unknown-none",
        "--features",
        "native-compositor",
    ]))
}
fn owner_target(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "check",
        "--locked",
        "-p",
        "conduit-browser-runtime",
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
    ]))
}
fn native(root: &Path) -> Result<(), Box<dyn Error>> {
    native_with_face(root, None, None)
}
fn native_with_face(
    root: &Path,
    face: Option<&Path>,
    output: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new("cargo");
    command.current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduitos",
        "--lib",
        "--features",
        "native-compositor-core",
        "native_mask_play::history_tests",
    ]);
    if let Some(face) = face {
        command.env("CONDUIT_HISTORY_NATIVE_PROOF_FACE", face);
        command.env(
            "CONDUIT_HISTORY_NATIVE_WINDOW_FACE",
            face.with_file_name("history-window-face.json"),
        );
    }
    if let Some(output) = output {
        command.env("CONDUIT_HISTORY_NATIVE_PROOF_OUTPUT", output);
    }
    super::execute(&mut command)
}
fn command(root: &Path, operation: &str) -> Command {
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args([
            operation,
            "--locked",
            "--manifest-path",
            "bodies/environment/history/Cargo.toml",
        ])
        .env(
            "CARGO_TARGET_DIR",
            std::env::var_os("CARGO_TARGET_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| root.join("target")),
        );
    command
}
fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(&mut command(root, "test"))?;
    super::execute(command(root, "clippy").args(["--all-targets", "--", "-D", "warnings"]))
}
fn target(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(command(root, "check").args(["--lib", "--target", "wasm32-unknown-unknown"]))
}
fn preview(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(
        command(root, "run")
            .args(["--example", "preview", "--"])
            .arg(root.join("target/environment-history-preview")),
    )
}

fn browser(root: &Path) -> Result<(), Box<dyn Error>> {
    preview(root)?;
    chromium(root, "proof/browser/environment-history.config.mjs", None)
}

fn browser_output(root: &Path) -> std::path::PathBuf {
    std::env::var_os("CONDUIT_HISTORY_BROWSER_PROOF_OUTPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("target/environment-owner-history"))
}
fn cross_mask(root: &Path) -> Result<(), Box<dyn Error>> {
    owner_browser(root)?;
    let output = browser_output(root);
    native_with_face(
        root,
        Some(&output.join("history-face.json")),
        Some(&output.join("native")),
    )?;
    let view: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("history-view.json"))?)?;
    let face: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("history-face.json"))?)?;
    let id = view["face_id"]
        .as_str()
        .ok_or("browser view lacks Face identity")?;
    let revision = view["face_revision"]
        .as_str()
        .ok_or("browser view lacks exact revision")?;
    let native = std::fs::read_to_string(output.join("native/native-history.txt"))?;
    if face["identity"].as_str() != Some(id)
        || face["revision"]
            .as_u64()
            .map(|value| value.to_string())
            .as_deref()
            != Some(revision)
        || !native.lines().any(|line| line == format!("Face: {id}"))
        || !native
            .lines()
            .any(|line| line == format!("Revision: {revision}"))
    {
        return Err("browser/native proof did not retain the same exact Face revision".into());
    }
    println!("history_cross_mask_face={id} revision={revision}");
    Ok(())
}
fn owner_browser(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-browser-mask-offer",
        "-p",
        "conduit-conduitos-mask-offer",
        "--lib",
    ]))?;
    super::execute(Command::new("node").current_dir(root).args([
        "--test",
        "targets/browser/sdk/package-browser-bundle.test.mjs",
    ]))?;
    let output = browser_output(root);
    super::execute(
        Command::new("cargo")
            .current_dir(root)
            .args([
                "test",
                "--locked",
                "-p",
                "conduit-browser-runtime",
                "--lib",
                "owner_face_mask::history_tests",
            ])
            .env("CONDUIT_HISTORY_BROWSER_PROOF_OUTPUT", &output),
    )?;
    // The same exported Rust view is consumed by the real DOM component.
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-browser-runtime",
        "-p",
        "conduit-browser-mask-offer",
        "-p",
        "conduit-conduitos-mask-offer",
        "--lib",
        "--",
        "-D",
        "warnings",
    ]))?;
    chromium(
        root,
        "proof/browser/environment-owner-history.config.mjs",
        Some(&output),
    )
}

fn chromium(root: &Path, config: &str, output: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let cli = root.join("proof/browser/node_modules/@playwright/test/cli.js");
    if !cli.is_file() {
        super::execute(Command::new("npm").current_dir(root).args([
            "--prefix",
            "proof/browser",
            "ci",
            "--ignore-scripts",
            "--prefer-offline",
        ]))?;
    }
    let mut command = Command::new("node");
    command.current_dir(root).arg(cli).args([
        "test",
        "--config",
        config,
        "--project",
        "chromium",
        "--workers=1",
        "--retries=0",
    ]);
    if let Some(output) = output {
        command.env("CONDUIT_HISTORY_BROWSER_PROOF_OUTPUT", output);
    }
    super::execute(&mut command)
}

fn committed(root: &Path) -> Result<(), Box<dyn Error>> {
    super::specifications(&["recording".into()])?;
    super::execute(
        Command::new("cargo")
            .current_dir(root)
            .args([
                "clippy",
                "--locked",
                "--manifest-path",
                "bodies/environment/runtime/Cargo.toml",
                "--lib",
                "--test",
                "recording_publication",
                "--test",
                "recording_chunks",
                "--test",
                "recording_recovery",
                "--test",
                "observation_forms",
                "--",
                "-D",
                "warnings",
            ])
            .env(
                "CARGO_TARGET_DIR",
                std::env::var_os("CARGO_TARGET_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| root.join("target")),
            ),
    )?;
    prove(root)
}

// Shared canonical Look and quantity formatter; no installed Host or default
// storage claim. The separate Mask workspace retains its own conformance.
fn look(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-face",
        "--test",
        "quantity_format",
        "--test",
        "look_contract",
        "--test",
        "look_selection",
        "--test",
        "look_browser_binding",
        "--test",
        "look_options",
        "--test",
        "look_preferences",
    ]))?;
    super::execute(
        Command::new("node")
            .current_dir(root)
            .args(["--test", "proof/browser/look-wire.test.mjs"]),
    )?;
    for operation in ["test", "clippy"] {
        let mut command = Command::new("cargo");
        command.current_dir(root).args([
            operation,
            "--locked",
            "--manifest-path",
            "plots/seven-segment-mask/Cargo.toml",
        ]);
        if operation == "clippy" {
            command.args(["--all-targets", "--", "-D", "warnings"]);
        }
        super::execute(&mut command)?;
    }
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-face",
        "--lib",
        "--",
        "-D",
        "warnings",
    ]))
}

fn preferences(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-plot",
        "--lib",
        "canonical_expansion_tests::root_startup",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-face",
        "--test",
        "configured_mask",
        "--test",
        "look_preferences",
        "--test",
        "look_browser_binding",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-plot",
        "-p",
        "conduit-face",
        "--lib",
        "--",
        "-D",
        "warnings",
    ]))
}
