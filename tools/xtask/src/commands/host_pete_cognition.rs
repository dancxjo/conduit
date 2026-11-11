//! Repository proof entry; execution remains the ordinary installed Body runner.
use crate::cli::GlobalOpts;
use clap::Args;
use conduit_ai::LocalModelKindProfile;
use conduit_std_host::hosted_local_model::OllamaDiscovery;
use std::path::{Component, PathBuf};
#[path = "host_pete_cognition/debugger.rs"]
mod debugger;
#[path = "host_pete_cognition/evidence.rs"]
mod evidence;
#[path = "host_pete_cognition/execution.rs"]
mod execution;
#[path = "host_pete_cognition/plans.rs"]
mod plans;
#[derive(Args, Debug)]
pub(super) struct CognitionProofRequest {
    /// Exact already-local Ollama model; no model is downloaded.
    #[arg(long)]
    model: String,
    /// Exact loopback Ollama origin.
    #[arg(long, default_value = conduit_std_host::hosted_local_model::DEFAULT_OLLAMA_ENDPOINT)]
    ollama_endpoint: String,
    /// Finite admitted model RAM/VRAM ceiling in MiB.
    #[arg(long)]
    admitted_memory_mib: u32,
    /// Fresh run directory beneath ignored workspace target/ output.
    #[arg(long, default_value = "target/conduit-evidence/pete-cognition")]
    output: PathBuf,
}
pub(super) fn prove(
    request: CognitionProofRequest,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = crate::workspace::workspace_root()?;
    let output = root.join(&request.output);
    if request.output.is_absolute()
        || request
            .output
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || !request.output.starts_with("target")
    {
        return Err(
            "cognition proof output must be a relative directory beneath workspace target/".into(),
        );
    }
    if opts.dry_run {
        if !opts.quiet {
            println!("would run the situated cognition Body with already-local model {} and retain evidence under {}", request.model, output.display());
        }
        return Ok(());
    }
    std::fs::create_dir_all(&output)?;
    if !output
        .canonicalize()?
        .starts_with(root.join("target").canonicalize()?)
    {
        return Err("cognition proof output resolves outside ignored target/".into());
    }
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    let run_output = output.join(format!("run-{run_id}-{}", std::process::id()));
    std::fs::create_dir(&run_output)?;
    let adapter = OllamaDiscovery::discover_at(&request.ollama_endpoint, &request.model)?
        .initialize(
            request.admitted_memory_mib,
            vec![LocalModelKindProfile::GenerateRecoverableFlow],
        )?;
    let receipt = execution::run(adapter, &run_output)?;
    if opts.json {
        println!("{}", serde_json::to_string(&receipt)?);
    } else if !opts.quiet {
        println!("LIVE COGNITION BODY: two scripted observations, one admitted local model");
        println!("{}", serde_json::to_string_pretty(&receipt)?);
        println!("evidence: {}", run_output.display());
    }
    Ok(())
}
