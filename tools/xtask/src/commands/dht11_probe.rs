//! Focused environmental Body specification, Mask and physical diagnostic proof.
use std::{error::Error, path::Path, process::Command};
#[path = "environment_proof.rs"]
mod environment;
#[path = "environment_host_wifi.rs"]
mod host_wifi;

pub fn dispatch(arguments: &[String]) -> Option<Result<(), Box<dyn Error>>> {
    if let Some(result) = host_wifi::dispatch(arguments) {
        return Some(result);
    }
    let [prove, name, rest @ ..] = arguments else {
        return None;
    };
    if prove == "prove" && name == "environment" {
        return Some(environment::specifications(rest));
    }
    if prove != "prove" || name != "dht11" {
        return None;
    }
    Some(run(rest))
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
