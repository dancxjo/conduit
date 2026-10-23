//! Focused environmental Body, Mask and native target proof dispatch.
use super::execute;
use std::{error::Error, process::Command};

pub(super) fn specifications(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let root = crate::workspace::workspace_root()?;
    if arguments == ["recording-storage"] {
        execute(Command::new("cargo").current_dir(&root).args([
            "test",
            "--locked",
            "-p",
            "conduit-web",
            "--test",
            "json_text",
        ]))?;
        execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "test",
                    "--locked",
                    "--manifest-path",
                    "bodies/environment/make/Cargo.toml",
                    "--test",
                    "recording_recovery",
                    "full_first_class_recording_fits_the_snapshot_text_profile",
                ])
                .env("CARGO_TARGET_DIR", root.join("target")),
        )?;
        return execute(Command::new("cargo").current_dir(&root).args([
            "test",
            "--locked",
            "-p",
            "conduit-browser-runtime",
            "resource_effect",
            "--lib",
        ]));
    }
    if arguments == ["network"] {
        execute(Command::new("cargo").current_dir(&root).args([
            "test",
            "--locked",
            "-p",
            "conduit-net",
            "--test",
            "station_attempt",
        ]))?;
        for (manifest, test) in [
            ("bodies/environment/http/Cargo.toml", "network_setup_tests"),
            ("bodies/environment/make/Cargo.toml", "network_setup"),
        ] {
            let mut command = Command::new("cargo");
            command
                .current_dir(&root)
                .args(["test", "--locked", "--manifest-path", manifest]);
            if manifest.contains("/make/") {
                command.args(["--test", test]);
            } else {
                command.arg(test);
            }
            command.env("CARGO_TARGET_DIR", root.join("target"));
            execute(&mut command)?;
        }
        return Ok(());
    }
    if arguments == ["native-construction"] {
        return execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "test",
                    "--locked",
                    "--manifest-path",
                    "bodies/environment/make/Cargo.toml",
                    "--test",
                    "native",
                    "--test",
                    "network_assignment",
                    "--test",
                    "selected_port_shapes",
                    "--test",
                    "credential_owner",
                    "--test",
                    "network_play",
                    "--test",
                    "root_control",
                ])
                .args(["--", "--nocapture"])
                .env("CARGO_TARGET_DIR", root.join("target")),
        );
    }
    if arguments == ["http"] || arguments == ["network-owner"] {
        let manifest = if arguments == ["http"] {
            "bodies/environment/http/Cargo.toml"
        } else {
            "mechanisms/io/network-join/Cargo.toml"
        };
        return execute(
            Command::new("cargo")
                .current_dir(&root)
                .args(["test", "--locked", "--manifest-path", manifest])
                .env("CARGO_TARGET_DIR", root.join("target")),
        );
    }
    if arguments == ["recording"] {
        execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "test",
                    "--locked",
                    "--manifest-path",
                    "bodies/environment/runtime/Cargo.toml",
                    "--test",
                    "recording_chunks",
                    "--test",
                    "recording_recovery",
                    "--test",
                    "recording_publication",
                ])
                .env("CARGO_TARGET_DIR", root.join("target/environment-runtime")),
        )?;
        return execute(
            Command::new("cargo")
                .current_dir(&root)
                .args([
                    "test",
                    "--locked",
                    "--manifest-path",
                    "bodies/environment/make/Cargo.toml",
                    "--test",
                    "recording_recovery",
                ])
                .env("CARGO_TARGET_DIR", root.join("target")),
        );
    }
    if arguments == ["display-esp32"]
        || arguments == ["pulse-esp32"]
        || arguments == ["capture-esp32"]
        || arguments == ["live-esp32"]
        || arguments == ["ap-esp32"]
        || arguments == ["root-esp32"]
        || arguments == ["network-esp32"]
    {
        let pulse = arguments == ["pulse-esp32"];
        let network = arguments == ["network-esp32"];
        let package = root.join(if network {
            "targets/esp32/network-switch"
        } else if pulse {
            "targets/esp32/pulse-capture"
        } else {
            "targets/esp32/proof/tm1637"
        });
        let toolchain: toml::Value = toml::from_str(&std::fs::read_to_string(
            package.join("rust-toolchain.toml"),
        )?)?;
        let channel = toolchain
            .get("toolchain")
            .and_then(|v| v.get("channel"))
            .and_then(toml::Value::as_str)
            .ok_or("display diagnostic has no pinned toolchain")?;
        let mut command = Command::new("cargo");
        command
            .current_dir(&package)
            .arg(format!("+{channel}"))
            .args([
                if pulse || network { "check" } else { "build" },
                "--locked",
                "--release",
            ])
            .env("CARGO_TARGET_DIR", root.join("target/environment-esp32"));
        if pulse {
            command.args(["--features", "host-call"]);
        } else if arguments == ["capture-esp32"] {
            command.args(["--features", "capture"]);
        } else if arguments == ["root-esp32"] {
            command.args(["--features", "native-root"]);
        } else if arguments == ["ap-esp32"] {
            command.args(["--features", "ap"]);
        } else if arguments == ["live-esp32"] {
            command.args(["--features", "live"]);
        }
        return execute(&mut command);
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
    if arguments == ["publication"] || arguments == ["publication-32"] {
        let mut command = Command::new("cargo");
        command.current_dir(&root).args([
            "test",
            "--locked",
            "--manifest-path",
            "bodies/environment/runtime/Cargo.toml",
            "--features",
            "construction",
            "--test",
            "publication",
            "--test",
            "construction",
            "--test",
            "appliance_kernel",
        ]);
        if arguments == ["publication-32"] {
            command.args(["--target", "i686-unknown-linux-gnu", "--", "--nocapture"]);
            command.env("CARGO_TARGET_DIR", root.join("target/environment-i686"));
        } else {
            command.env("CARGO_TARGET_DIR", root.join("target"));
        }
        return execute(&mut command);
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
            "expected cargo xtask prove environment specifications, face, http, publication, publication-32, recording, recording-storage, native-construction, network, network-owner, mask, display-esp32, pulse-esp32, capture-esp32, live-esp32, ap-esp32, root-esp32, network-esp32, pulse, pulse-target, kernel, or kernel-target".into(),
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
