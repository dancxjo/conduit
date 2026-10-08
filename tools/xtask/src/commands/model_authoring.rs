//! Dependency-light hosted model proof, shared by the ordinary xtask dispatcher.
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::PathBuf, process::Command};

pub fn run(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut output = PathBuf::from("work/model-authoring");
    let mut cuda = false;
    let mut documented_command = false;
    let mut options = arguments.iter();
    while let Some(option) = options.next() {
        match option.as_str() {
            "--output" => {
                output = PathBuf::from(options.next().ok_or("--output needs a new directory")?)
            }
            "--cuda" => cuda = true,
            "--documented-command" => documented_command = true,
            "--help" | "-h" => {
                println!("cargo xtask prove model-authoring [--output NEW_DIRECTORY] [--cuda | --documented-command]\nProves the hosted Burn library contracts; ConduitVoice/ordinary Plot integration remains separate.");
                return Ok(());
            }
            _ => return Err(format!("unknown model-authoring option: {option}").into()),
        }
    }
    if documented_command {
        if cuda {
            return Err("documented CPU command cannot be combined with --cuda".into());
        }
        return run_documented_command(output);
    }
    if output.exists() {
        return Err("proof output directory already exists; preserve prior evidence and choose a new directory".into());
    }
    fs::create_dir_all(&output)?;
    let mut command = Command::new("cargo");
    command.args([
        "test",
        "--locked",
        "--jobs",
        "2",
        "-p",
        "conduit-burn-model",
    ]);
    if cuda {
        command.args([
            "--features",
            "cuda",
            "--test",
            "device_contract",
            "--",
            "--ignored",
            "--nocapture",
        ]);
    }
    if !cuda {
        command.env(
            "CONDUIT_MODEL_PROOF_ARTIFACTS",
            fs::canonicalize(&output)?.join("artifacts"),
        );
        command.args(["--", "--nocapture"]);
    }
    command
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_PROFILE_TEST_DEBUG", "0")
        .env("CARGO_INCREMENTAL", "0");
    let source_before = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    let status_before = Command::new("git")
        .args(["status", "--porcelain"])
        .output()?;
    let result = command.output()?;
    fs::write(output.join("stdout.log"), &result.stdout)?;
    fs::write(output.join("stderr.log"), &result.stderr)?;
    let source = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .output()?;
    let source_unchanged = source_before.status.success()
        && source.status.success()
        && status_before.status.success()
        && status.status.success()
        && source_before.stdout == source.stdout
        && status_before.stdout == status.stdout;
    let passed = result.status.success() && source_unchanged;
    let mut hash = Sha256::new();
    hash.update(&result.stdout);
    hash.update(&result.stderr);
    let device_evidence = if cuda {
        Some(cuda_device_evidence()?)
    } else {
        None
    };
    let manifest = json!({
        "schema":"conduit.proof/model-authoring@1","proof_class":if cuda {"physical-local-hardware"} else {"deterministic-unit"},"scope":"hosted-model-library",
        "source_head":String::from_utf8_lossy(&source_before.stdout).trim(),"working_tree_dirty":!status_before.stdout.is_empty() || !status.stdout.is_empty(),"source_unchanged":source_unchanged,
        "build_profile":{"debug_symbols":false,"incremental":false,"jobs":2},"device_evidence":device_evidence,"cuda_requested":cuda,"passed":passed,"exit_code":result.status.code(),
        "log_sha256":format!("{:x}",hash.finalize()),
        "establishes":if passed {Some(if cuda {"explicit CUDA training, checkpoint and resume"} else {"Burn authoring, atomic training/evaluation, safe checkpoint and fresh-runtime resume contracts"})} else {None},
        "does_not_establish":["ordinary Plot/HostCall execution","ConduitVoice training","FARGAN reconstruction","attended listening","stable acceptance"]
    });
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    print!("{}", String::from_utf8_lossy(&result.stdout));
    eprint!("{}", String::from_utf8_lossy(&result.stderr));
    if !passed {
        return Err(format!(
            "model-authoring proof failed; retained evidence at {}",
            output.display()
        )
        .into());
    }
    println!("hosted model proof retained at {}", output.display());
    Ok(())
}

fn documented_arguments(source: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let block = source
        .split("<!-- model-authoring-command -->")
        .nth(1)
        .ok_or("missing documented command marker")?
        .strip_prefix("\n```sh\n")
        .ok_or("documented command must be a shell block")?
        .split("\n```")
        .next()
        .ok_or("unclosed documented command")?;
    let arguments: Vec<_> = block.split_whitespace().map(str::to_owned).collect();
    if arguments.len() != 6
        || arguments[..5] != ["cargo", "xtask", "prove", "model-authoring", "--output"]
    {
        return Err("documented hosted proof command has an unsupported structure".into());
    }
    Ok(arguments)
}
fn run_documented_command(output: PathBuf) -> Result<(), Box<dyn Error>> {
    let source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../wiki/Creating-models.md"),
    )?;
    let mut arguments = documented_arguments(&source)?;
    if output.exists() {
        return Err("documentation proof output already exists".into());
    }
    fs::create_dir_all(&output)?;
    arguments[5] = output.join("command").display().to_string();
    let result = Command::new(&arguments[0]).args(&arguments[1..]).output()?;
    fs::write(output.join("stdout.log"), &result.stdout)?;
    fs::write(output.join("stderr.log"), &result.stderr)?;
    let manifest = json!({"schema":"conduit.proof/model-authoring-docs@1","proof_class":"deterministic-unit","scope":"hosted-model-library",
        "document_sha256":format!("{:x}",Sha256::digest(source.as_bytes())),"executed_arguments":arguments,
        "passed":result.status.success(),"exit_code":result.status.code(),"output_substitution":"host-local evidence directory"});
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    print!("{}", String::from_utf8_lossy(&result.stdout));
    eprint!("{}", String::from_utf8_lossy(&result.stderr));
    if !result.status.success() {
        return Err("extracted documentation command failed; see retained logs".into());
    }
    Ok(())
}

fn cuda_device_evidence() -> Result<serde_json::Value, Box<dyn Error>> {
    use std::io::Read;
    let mut host = String::new();
    std::fs::File::open("/etc/hostname")?
        .take(256)
        .read_to_string(&mut host)?;
    let mut devices = Vec::new();
    for entry in fs::read_dir("/proc/driver/nvidia/gpus")?.take(16) {
        let mut information = String::new();
        std::fs::File::open(entry?.path().join("information"))?
            .take(4096)
            .read_to_string(&mut information)?;
        devices.push(information);
    }
    if host.trim().is_empty() || devices.len() != 1 {
        return Err("CUDA proof requires one unambiguous identified local GPU; preserve logs and provide an explicit multi-device proof entrance before claiming it".into());
    }
    Ok(
        json!({"host":host.trim(),"driver_device_information":devices,"selected_profile":"burn/cuda/0","precision":"number/ieee754-f32-le","absolute_output_tolerance":0.0001}),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn model_authoring_documented_commands() {
        let source = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../wiki/Creating-models.md"),
        )
        .unwrap();
        assert_eq!(
            super::documented_arguments(&source).unwrap()[5],
            "work/model-authoring"
        );
        assert!(super::documented_arguments("<!-- model-authoring-command -->\n```sh\ncargo xtask prove model-authoring; rm -rf anything\n``` ").is_err());
    }
}
