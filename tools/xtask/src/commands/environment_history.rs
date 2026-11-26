//! Recorded-history preparation and CPU native composition; no live Host claim.
use std::{error::Error, path::Path, process::Command};
pub(super) fn dispatch(root: &Path, args: &[String]) -> Option<Result<(), Box<dyn Error>>> {
    match args {
        [name] if name == "state-fore" => Some(state_fore(root)),

        [name] if name == "recording-quantity" => Some(recording_quantity(root)),
        [name] if name == "data-input" => Some(data_input(root, false)),
        [name, scope] if name == "data-input" && scope == "all" => Some(data_input(root, true)),
        [name] if name == "data-target" => Some(data_target(root)),
        [name] if name == "json-snapshot" => Some(json_snapshot(root)),
        [name] if name == "quantity-rounding" => Some(quantity_rounding(root)),
        [name] if name == "history" => Some(prove(root)),
        [name] if name == "history-owner-mask" => Some(owner_mask(root)),
        [name] if name == "history-commit" => Some(committed(root)),
        [name] if name == "history-native" => Some(native(root)),
        [name] if name == "history-shared-native" => Some(shared_native(root)),
        [name] if name == "history-root-startup" => Some(root_startup(root)),
        [name] if name == "history-target" => Some(target(root)),
        [name] if name == "history-browser" => Some(browser(root)),
        [name] if name == "history-preview" => Some(preview(root)),
        _ => None,
    }
}
// The recorder adapter remains opt-in and independent of the environmental Body.
fn recording_quantity(root: &Path) -> Result<(), Box<dyn Error>> {
    for operation in ["test", "clippy"] {
        let mut command = Command::new("cargo");
        command.current_dir(root).args([
            operation,
            "--locked",
            "-p",
            "conduit-dht11-observation",
            "--all-features",
            "--all-targets",
        ]);
        if operation == "clippy" {
            command.args(["--", "-D", "warnings"]);
        }
        super::execute(&mut command)?;
    }
    for target in ["thumbv6m-none-eabi", "wasm32-unknown-unknown"] {
        super::execute(Command::new("cargo").current_dir(root).args([
            "check",
            "--locked",
            "-p",
            "conduit-dht11-observation",
            "--no-default-features",
            "--all-features",
            "--lib",
            "--target",
            target,
        ]))?;
    }
    Ok(())
}
fn native(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduitos",
        "--lib",
        "--features",
        "hosted-tools,native-compositor",
        "native_mask_play::history_tests",
        "--",
        "--test-threads=1",
    ]))
}
fn root_startup(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-plot",
        "--lib",
        "canonical_expansion_tests::",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-plot",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "check",
        "--locked",
        "-p",
        "conduit-plot",
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
    ]))
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
    for operation in ["test", "clippy"] {
        let mut shared = Command::new("cargo");
        shared.current_dir(root).args([
            operation,
            "--locked",
            "-p",
            "conduit-face",
            "--test",
            "quantity_series",
        ]);
        if operation == "clippy" {
            shared.args(["--", "-D", "warnings"]);
        }
        super::execute(&mut shared)?;
    }
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
    super::execute(Command::new("node").current_dir(root).arg(cli).args([
        "test",
        "--config",
        "proof/browser/environment-history.config.mjs",
        "--project",
        "chromium",
        "--workers=1",
        "--retries=0",
    ]))
}

// Exact display rounding only: no Look wire, device or live Host migration.
fn quantity_rounding(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-face",
        "--test",
        "quantity_rounding",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-face",
        "--lib",
        "--test",
        "quantity_rounding",
        "--",
        "-D",
        "warnings",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "check",
        "--locked",
        "-p",
        "conduit-face",
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
    ]))?;
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
    Ok(())
}

fn committed(root: &Path) -> Result<(), Box<dyn Error>> {
    for operation in ["test", "clippy"] {
        let mut shared = Command::new("cargo");
        shared.current_dir(root).args([
            operation,
            "--locked",
            "-p",
            "conduit-resource-snapshot",
            "--features",
            "kernel-step",
            "--all-targets",
        ]);
        if operation == "clippy" {
            shared.args(["--", "-D", "warnings"]);
        }
        super::execute(&mut shared)?;
    }
    super::specifications(&["recording".into()])?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "--manifest-path",
        "bodies/environment/runtime/Cargo.toml",
        "--lib",
        "--test",
        "recording_chunks",
        "--test",
        "recording_publication",
        "--test",
        "recording_recovery",
        "--",
        "-D",
        "warnings",
    ]))?;
    prove(root)
}

fn owner_mask(root: &Path) -> Result<(), Box<dyn Error>> {
    let view = root.join("target/environment-history-owner-view.json");
    std::fs::create_dir_all(root.join("target"))?;
    super::execute(
        Command::new("cargo")
            .current_dir(root)
            .env("CONDUIT_HISTORY_OWNER_VIEW", &view)
            .args([
                "test",
                "--locked",
                "-p",
                "conduit-browser-runtime",
                "--lib",
                "owner_face_mask::quantity_series",
                "--",
                "--test-threads=1",
            ]),
    )?;
    super::execute(Command::new("node").current_dir(root).args([
        "--check",
        "targets/browser/handbook/owner-face-presentation.mjs",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-browser-runtime",
        "--lib",
        "--",
        "-D",
        "warnings",
    ]))?;
    super::execute(
        Command::new("node")
            .current_dir(root)
            .env("CONDUIT_HISTORY_OWNER_VIEW", &view)
            .args([
                "proof/browser/node_modules/@playwright/test/cli.js",
                "test",
                "--config",
                "proof/browser/playwright.config.mjs",
                "--project",
                "chromium",
                "--workers",
                "1",
                "--retries",
                "0",
                "owner-face-component.spec.mjs",
            ]),
    )?;
    Ok(())
}

fn shared_native(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduitos",
        "--lib",
        "--features",
        "hosted-tools,native-compositor",
        "native_face_scene::quantity_series::tests",
        "--",
        "--test-threads=1",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduitos",
        "--lib",
        "--features",
        "hosted-tools,native-compositor",
        "--",
        "-D",
        "warnings",
    ]))
}

fn data_input(root: &Path, all: bool) -> Result<(), Box<dyn Error>> {
    let mut test = Command::new("cargo");
    test.current_dir(root)
        .args(["test", "--locked", "-p", "conduit-data"]);
    if all {
        test.arg("--all-features");
    } else {
        test.args(["--test", "quantity_observation_input"]);
    }
    super::execute(&mut test)?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-data",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ]))
}
fn data_target(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "check",
        "--locked",
        "-p",
        "conduit-data",
        "--lib",
        "--all-features",
        "--target",
        "wasm32-unknown-unknown",
    ]))
}

// Generic caller composition, not recorder/catalog/ACK qualification.
fn state_fore(root: &Path) -> Result<(), Box<dyn Error>> {
    const FILTER: &str = "host_execution::body_durable::tests::";
    let listed = Command::new("cargo")
        .current_dir(root)
        .args([
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--lib",
            FILTER,
            "--",
            "--list",
        ])
        .output()?;
    if !listed.status.success() {
        return Err(format!(
            "state/Fore test inventory failed: {}",
            String::from_utf8_lossy(&listed.stderr)
        )
        .into());
    }
    let inventory = String::from_utf8(listed.stdout)?;
    let count = inventory
        .lines()
        .filter(|line| line.starts_with(FILTER) && line.ends_with(": test"))
        .count();
    if count < 2 {
        return Err(format!("state/Fore proof requires its focused tests, found {count}").into());
    }
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-std-host",
        "--lib",
        FILTER,
        "--",
        "--test-threads=1",
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

// Exact selected file effect + installed caller; no recorder ACK qualification.
fn json_snapshot(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-kernel",
        "--test",
        "zero_payload_cords",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-plot",
        "--test",
        "prepared_resource_reference",
    ]))?;
    for args in [
        vec![
            "test",
            "--locked",
            "-p",
            "conduit-std-offers",
            "--lib",
            "json_snapshot::tests::",
        ],
        vec![
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--test",
            "json_snapshot_file",
        ],
        vec![
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--lib",
            "installed_std::json_snapshot_back::tests::",
        ],
        vec![
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--lib",
            "installed_std::fore_outputs::tests::",
        ],
        vec![
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--lib",
            "state_value::durable::",
        ],
    ] {
        super::execute(Command::new("cargo").current_dir(root).args(args))?;
    }
    const FILTER: &str = "host_execution::json_snapshot::tests::";
    let listed = Command::new("cargo")
        .current_dir(root)
        .args([
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--lib",
            FILTER,
            "--",
            "--list",
        ])
        .output()?;
    if !listed.status.success() {
        return Err(format!(
            "selected JSON caller inventory failed: {}",
            String::from_utf8_lossy(&listed.stderr)
        )
        .into());
    }
    let inventory = String::from_utf8(listed.stdout)?;
    let count = inventory
        .lines()
        .filter(|line| line.starts_with(FILTER) && line.ends_with(": test"))
        .count();
    let required = [
        "source::authored_publication_is_the_only_reference_update_path_into_body_state",
        "authored_json_reference_state_recovers_in_fresh_process_then_reads_exact_generation",
        "malformed_json_publication_cannot_replace_committed_reference_state",
        "admission::missing_or_undersized_existing_memory_pool_refuses_before_play_and_effects",
        "admission::invalid_history_capacity_refuses_before_play_without_changing_queue_capacity",
        "preparation::admitted_preparation_profile_covers_actual_selected_provider_allocation",
    ];
    for test in required {
        let expected = format!("{FILTER}{test}: test");
        if !inventory.lines().any(|line| line == expected) {
            return Err(format!(
                "selected JSON proof is missing required test {test}; found {count} tests"
            )
            .into());
        }
    }
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-std-host",
        "--lib",
        FILTER,
        "--",
        "--test-threads=1",
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
