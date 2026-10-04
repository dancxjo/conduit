//! Repository entrance for native type/Back conformance and early WAV samples.
use crate::{
    cli::{GlobalOpts, NativeSpeechArgs},
    workspace::workspace_root,
};
use std::process::Command;
#[path = "native_speech_stack.rs"]
mod stack;
pub fn run(args: NativeSpeechArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let output = if args.output.is_absolute() {
        args.output
    } else {
        root.join(args.output)
    };
    if opts.dry_run {
        println!(
            "would check native speech contracts, compare compiled plots, and retain WAVs in {}",
            output.display()
        );
        return Ok(());
    }
    let invoke = |arguments: &[&str]| -> Result<(), Box<dyn std::error::Error>> {
        let status = Command::new("cargo")
            .args(arguments)
            .current_dir(&root)
            .status()?;
        if !status.success() {
            return Err(format!("native speech step failed: {status}").into());
        }
        Ok(())
    };
    invoke(&[
        "test",
        "--locked",
        "-p",
        "conduit-plot",
        "--test",
        "native_expression_construction",
    ])?;
    invoke(&[
        "test",
        "--locked",
        "-p",
        "conduit-speech",
        "--features",
        "semantic-bindings,kernel",
        "--tests",
    ])?;
    invoke(&[
        "test",
        "--locked",
        "-p",
        "conduit-std-host",
        "--test",
        "native_speech",
    ])?;
    invoke(&[
        "test",
        "--locked",
        "-p",
        "conduit",
        "--test",
        "native_speech",
    ])?;
    let status = Command::new("cargo")
        .args([
            "run",
            "--locked",
            "-p",
            "conduit-speech",
            "--features",
            "semantic-bindings",
            "--example",
            "first_samples",
            "--",
        ])
        .arg(&output)
        .current_dir(&root)
        .status()?;
    if !status.success() {
        return Err(format!("native speech samples failed: {status}").into());
    }
    if args.microcontroller {
        let fixture = root.join("proof/fixtures/native-speech-footprint");
        for binary in [
            "conduit-native-speech-footprint",
            "conduit-native-text-footprint",
        ] {
            let status = Command::new("cargo")
                .args([
                    "rustc",
                    "--locked",
                    "--release",
                    "--bin",
                    binary,
                    "--manifest-path",
                ])
                .arg(fixture.join("Cargo.toml"))
                .args(["--target", "thumbv6m-none-eabi", "--", "-C"])
                .arg(format!(
                    "link-arg=-T{}",
                    fixture.join("linker.ld").display()
                ))
                .current_dir(&root)
                .status()?;
            if !status.success() {
                return Err(format!("Cortex-M0+ link-only probe failed: {status}").into());
            }
            let artifact = fixture
                .join("target/thumbv6m-none-eabi/release")
                .join(binary);
            println!("Cortex-M0+ linked probe: {}", artifact.display());
            match Command::new("size").arg(&artifact).status() {
            Ok(status) if status.success() => println!("Section totals only: text includes code/constants; data/bss are static RAM. Full call-chain stack, body, device timing and playback remain unmeasured."),
            Ok(status) => return Err(format!("footprint section inspection failed: {status}").into()),
            Err(error) if error.kind()==std::io::ErrorKind::NotFound => return Err("linked probe retained, but GNU size is missing for footprint inspection".into()),
            Err(error) => return Err(error.into()),
        }
            stack::inspect(&root, &artifact, &output, binary)?;
        }
    }
    println!("Retained native voice WAVs: {}. Conformance and synthesis Back evidence; no device playback or Klatt/eSpeak parity acceptance.",output.display());
    Ok(())
}
