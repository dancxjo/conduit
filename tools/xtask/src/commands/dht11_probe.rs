//! Focused environmental Body specification, Mask and physical diagnostic proof.
#[path = "environment_appliance.rs"]
mod environment_appliance;
#[path = "environment_body_image.rs"]
mod environment_body_image;
#[path = "environment_esp32.rs"]
mod environment_esp32;
#[path = "environment_face_publication.rs"]
mod environment_face_publication;
#[path = "environment_output.rs"]
mod environment_output;
#[path = "environment_source.rs"]
mod environment_source;
use std::{error::Error, path::Path, process::Command};

pub fn dispatch(arguments: &[String]) -> Option<Result<(), Box<dyn Error>>> {
    let [prove, name, rest @ ..] = arguments else {
        return None;
    };
    if prove == "prove" && name == "environment" {
        return Some(specifications(rest));
    }
    if prove != "prove" || name != "dht11" {
        return None;
    }
    Some(run(rest))
}

fn specifications(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let root = crate::workspace::workspace_root()?;
    if arguments == ["body-image"] {
        return environment_body_image::prove(&root);
    }
    if arguments == ["appliance"] {
        return environment_appliance::prove(&root);
    }
    if arguments == ["face-publication"] {
        return environment_face_publication::prove(&root);
    }
    if arguments == ["display-output"] {
        return environment_output::prove(&root);
    }
    if arguments == ["source"] {
        return environment_source::prove(&root);
    }
    if arguments == ["recording"] {
        return execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "test",
                    "--locked",
                    "--manifest-path",
                    "bodies/environment/runtime/Cargo.toml",
                    "--test",
                    "observation_forms",
                    "--test",
                    "recording_chunks",
                    "--test",
                    "recording_recovery",
                    "--test",
                    "recording_publication",
                ])
                .env(
                    "CARGO_TARGET_DIR",
                    std::env::var_os("CARGO_TARGET_DIR")
                        .map(std::path::PathBuf::from)
                        .unwrap_or_else(|| root.join("target")),
                ),
        );
    }
    if arguments == ["http"] {
        return execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "test",
                    "--locked",
                    "--manifest-path",
                    "bodies/environment/http/Cargo.toml",
                ])
                .env("CARGO_TARGET_DIR", root.join("target")),
        );
    }
    if arguments == ["display-esp32"]
        || arguments == ["native-esp32"]
        || arguments == ["native-body-esp32"]
    {
        return environment_esp32::build(
            &root,
            arguments != ["display-esp32"],
            arguments == ["native-body-esp32"],
        );
    }
    if arguments == ["display-target"] {
        return execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "check",
                    "--locked",
                    "--manifest-path",
                    "targets/rp2040/tm1637/Cargo.toml",
                    "--target",
                    "thumbv6m-none-eabi",
                ])
                .env(
                    "CARGO_TARGET_DIR",
                    root.join("targets/rp2040/proof/dht11/target"),
                ),
        );
    }
    if arguments == ["display-wire"] {
        return execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "test",
                    "--locked",
                    "--manifest-path",
                    "mechanisms/io/tm1637/Cargo.toml",
                ])
                .env("CARGO_TARGET_DIR", root.join("target")),
        );
    }
    if arguments == ["cadence"] {
        execute(Command::new("cargo").current_dir(&root).args([
            "test",
            "--locked",
            "-p",
            "conduit-time",
            "--features",
            "kernel-step",
            "--test",
            "counted_tick",
            "--test",
            "tick_trigger",
        ]))?;
        return execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "test",
                    "--locked",
                    "--manifest-path",
                    "bodies/environment/runtime/Cargo.toml",
                    "--test",
                    "cadence",
                ])
                .env("CARGO_TARGET_DIR", root.join("target")),
        );
    }
    if arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
        == ["face"]
    {
        for (package, test) in [
            ("conduit-core", "value_constraint_transport"),
            ("conduit-face", "prepared_revision"),
        ] {
            execute(
                Command::new("cargo")
                    .current_dir(&root)
                    .args(["test", "--locked", "-p", package, "--test", test]),
            )?;
        }
        return execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "test",
                    "--locked",
                    "--manifest-path",
                    "bodies/environment/face/Cargo.toml",
                ])
                .env("CARGO_TARGET_DIR", root.join("target")),
        );
    }
    if arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
        == ["kernel"]
    {
        execute(Command::new("cargo").current_dir(&root).args([
            "test",
            "--locked",
            "-p",
            "conduit-assigned-plan",
        ]))?;
        execute(Command::new("cargo").current_dir(&root).args([
            "test",
            "--locked",
            "-p",
            "conduit-embedded-build",
        ]))?;
        return execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "test",
                    "--locked",
                    "--manifest-path",
                    "bodies/environment/runtime/Cargo.toml",
                ])
                .env("CARGO_TARGET_DIR", root.join("target")),
        );
    }
    if arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
        == ["kernel-target"]
    {
        return execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "check",
                    "--locked",
                    "--manifest-path",
                    "bodies/environment/runtime/Cargo.toml",
                    "--target",
                    "thumbv6m-none-eabi",
                ])
                .env(
                    "CARGO_TARGET_DIR",
                    root.join("targets/rp2040/proof/dht11/target"),
                ),
        );
    }
    if arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
        == ["pulse-target"]
    {
        return execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "check",
                    "--locked",
                    "--manifest-path",
                    "targets/rp2040/pulse-capture/Cargo.toml",
                    "--target",
                    "thumbv6m-none-eabi",
                    "--features",
                    "host-call",
                ])
                .env(
                    "CARGO_TARGET_DIR",
                    root.join("targets/rp2040/proof/dht11/target"),
                ),
        );
    }
    if arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
        == ["pulse"]
    {
        return execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "test",
                    "--locked",
                    "--manifest-path",
                    "mechanisms/io/pulse-capture/Cargo.toml",
                    "--features",
                    "plot-catalog,host-call",
                ])
                .env("CARGO_TARGET_DIR", root.join("target")),
        );
    }
    if arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
        == ["mask"]
    {
        execute(Command::new("cargo").current_dir(&root).args([
            "test",
            "--locked",
            "-p",
            "conduit-plot",
            "--test",
            "boolean_forms",
        ]))?;
        return execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "test",
                    "--locked",
                    "--manifest-path",
                    "plots/seven-segment-mask/Cargo.toml",
                ])
                .env("CARGO_TARGET_DIR", root.join("target")),
        );
    }
    if arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
        != ["specifications"]
    {
        return Err(
            "expected cargo xtask prove environment specifications, body-image, appliance, source, display-output, face-publication, face, recording, mask, display-esp32, native-esp32, native-body-esp32, pulse, pulse-target, kernel, or kernel-target".into(),
        );
    }
    execute(
        Command::new("cargo")
            .current_dir(&root)
            .args([
                "run",
                "--locked",
                "--manifest-path",
                "bodies/environment/make/Cargo.toml",
            ])
            .env("CARGO_TARGET_DIR", root.join("target")),
    )
}

fn run(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let root = crate::workspace::workspace_root()?;
    let manifest = root.join("targets/rp2040/proof/dht11/Cargo.toml");
    match arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["check"] => execute(Command::new("cargo").current_dir(&root).args([
            "test",
            "--locked",
            "-p",
            "conduit-plot",
            "--test",
            "dht11",
            "--test",
            "quantity_forms",
        ])),
        [mode @ ("build" | "build-source" | "build-pio" | "build-kernel" | "build-display" | "build-http")] => {
            let target = manifest.parent().unwrap().join("target");
            let features = match *mode {
                "build-source" => "source-evaluation",
                "build-pio" => "pio-capture",
                "build-kernel" => "kernel-evaluation",
                "build-display" => "display-wire",
                "build-http" => "http-ap",
                _ => "",
            };
            execute(
                Command::new("cargo")
                    .current_dir(&root)
                    .args([
                        "build",
                        "--locked",
                        "--release",
                        "--target",
                        "thumbv6m-none-eabi",
                        "--features",
                        features,
                        "--manifest-path",
                    ])
                    .arg(&manifest)
                    .env("CARGO_TARGET_DIR", &target),
            )?;
            let elf = target.join("thumbv6m-none-eabi/release/conduit-pico-dht11-probe");
            let uf2 = elf.with_extension("uf2");
            execute(Command::new("elf2uf2-rs").arg(&elf).arg(&uf2))?;
            println!("Diagnostic UF2: {}", uf2.display());
            Ok(())
        }
        ["decode", capture] => decode(&root, capture),
        [] | ["--help"] => {
            println!(
                "cargo xtask prove dht11 check\ncargo xtask prove dht11 build\ncargo xtask prove dht11 build-source\ncargo xtask prove dht11 build-pio\ncargo xtask prove dht11 build-kernel\ncargo xtask prove dht11 build-display\ncargo xtask prove dht11 build-http\ncargo xtask prove dht11 decode <capture.ndjson>\n\nDiagnostic only: Source decoding and finite GP2 capture; no logger or Wi-Fi claims."
            );
            Ok(())
        }
        _ => Err(
            "expected dht11 check, build, build-source, build-pio, build-kernel, build-display, or decode <capture.ndjson>"
                .into(),
        ),
    }
}

fn decode(root: &Path, capture: &str) -> Result<(), Box<dyn Error>> {
    // Resolve caller-relative capture paths before changing command directory.
    let capture = std::fs::canonicalize(capture)?;
    execute(
        Command::new("cargo")
            .current_dir(root)
            .args([
                "run",
                "--locked",
                "-p",
                "conduit-plot",
                "--example",
                "dht11_decode",
                "--",
            ])
            .arg(capture),
    )
}

fn execute(command: &mut Command) -> Result<(), Box<dyn Error>> {
    let status = command.status()?;
    if !status.success() {
        return Err(format!("DHT11 diagnostic step failed: {status}").into());
    }
    Ok(())
}
