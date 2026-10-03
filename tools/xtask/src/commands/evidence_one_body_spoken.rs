//! One live installed Body Face through finite Presenter and ordinary spoken Mask.
//! This is one producer chapter, not a complete journey or a publication route.

use crate::evidence::{
    verify, EvidenceKind, EvidenceManifest, EvidenceResult, ExpectedEvidenceResult,
    VerificationRequest,
};
use clap::{Args as ClapArgs, ValueEnum};
use conduit_std_host::hosted_speech_synthesis::EspeakDiscovery;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const PROOF_ID: &str = "journey-one-body-spoken-chapter";
const SUITE_ID: &str = "journey-gallery";

#[path = "evidence_one_body_spoken/direct.rs"]
mod direct;
#[path = "evidence_one_body_spoken/llm.rs"]
mod llm;
#[path = "evidence_one_body_spoken/owner.rs"]
mod owner;
#[path = "evidence_one_body_spoken/retention.rs"]
mod retention;
use owner::{face_bytes, git, hash_file, parse_snapshot, OwnerSnapshot};
use retention::retain;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum SpeechMode {
    Direct,
    LlmAssisted,
}

struct LiveContext<'a> {
    workspace: &'a Path,
    source_commit: &'a str,
    bin: &'a Path,
    state_dir: &'a Path,
    bin_sha256: &'a str,
    face: &'a OwnerSnapshot,
    run_id: &'a str,
    voice: &'a str,
}

#[derive(ClapArgs, Debug)]
pub(crate) struct Args {
    /// Newly built installed owner executable from this exact source.
    #[arg(long)]
    conduit_bin: PathBuf,
    /// Isolated state directory of a running installed owner.
    #[arg(long)]
    state_dir: PathBuf,
    /// New evidence directory; no existing output is replaced.
    #[arg(long)]
    output: PathBuf,
    /// Capture owner's preassigned run token, supplied before this action begins.
    #[arg(long)]
    run_id: Option<String>,
    /// Mechanical full-Face reading or finite model-assisted wording.
    #[arg(long, value_enum, default_value_t = SpeechMode::LlmAssisted)]
    mode: SpeechMode,
    /// Already-local Ollama model name.
    #[arg(long)]
    model: Option<String>,
    #[arg(long, default_value = "http://127.0.0.1:11434")]
    ollama_endpoint: String,
    #[arg(long, default_value_t = 2048)]
    admitted_memory_mib: u32,
    /// Exact installed eSpeak executable, data tree, and engine closure.
    #[arg(long)]
    speech_executable: PathBuf,
    #[arg(long)]
    speech_data: PathBuf,
    #[arg(long, required = true, num_args = 1..)]
    speech_engine: Vec<PathBuf>,
    #[arg(long, default_value = "en-us")]
    speech_voice: String,
}

pub(super) fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    match (args.mode, args.model.as_deref()) {
        (SpeechMode::LlmAssisted, None) => {
            return Err("--model is required for --mode llm-assisted".into());
        }
        (SpeechMode::Direct, Some(_)) => {
            return Err("--model applies only to --mode llm-assisted".into());
        }
        _ => {}
    }
    let workspace = crate::workspace::workspace_root()?;
    let source_commit = git(&workspace, &["rev-parse", "HEAD"])?;
    let changed = git(&workspace, &["status", "--porcelain"])?;
    if !changed.is_empty() {
        return Err("live journey producer requires a clean exact source tree".into());
    }
    let bin = fs::canonicalize(&args.conduit_bin)?;
    let state_dir = fs::canonicalize(&args.state_dir)?;
    if !bin.is_file() || !state_dir.is_dir() {
        return Err("installed owner executable and state directory are required".into());
    }
    let bin_sha256 = hash_file(&bin)?;
    let first_bytes = face_bytes(&bin, &state_dir)?;
    let first = parse_snapshot(&first_bytes)?;
    let body_id = first
        .presentation
        .basis
        .body_id
        .as_ref()
        .ok_or("owner Face has no Body identity")?;
    let run_nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let run_seed = format!(
        "{}/{}/{}/{}",
        source_commit,
        body_id.as_str(),
        first.presentation.revision,
        run_nonce,
    );
    let run_digest = format!("{:x}", Sha256::digest(run_seed.as_bytes()));
    let run_id = args
        .run_id
        .unwrap_or_else(|| format!("one-body-spoken-{}", &run_digest[..32]));
    if run_id.is_empty()
        || run_id.len() > 128
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("run ID must be a bounded alphanumeric, dash, or underscore token".into());
    }
    let execution_id = format!("journey-spoken-{}", &run_digest[..24]);
    let speech = EspeakDiscovery::inspect(
        &args.speech_executable,
        &args.speech_data,
        &args.speech_voice,
        &args.speech_engine,
    )?;

    fs::create_dir(&args.output)?;
    let mut manifest = EvidenceManifest::new(&args.output, &workspace, PROOF_ID, SUITE_ID)?;
    retain(
        &mut manifest,
        "owner-face",
        "owner-face.json",
        EvidenceKind::MachineReadableManifest,
        "application/json",
        &first_bytes,
        &run_id,
        &first,
        None,
    )?;
    let context = LiveContext {
        workspace: &workspace,
        source_commit: &source_commit,
        bin: &bin,
        state_dir: &state_dir,
        bin_sha256: &bin_sha256,
        face: &first,
        run_id: &run_id,
        voice: &args.speech_voice,
    };
    if args.mode == SpeechMode::Direct {
        return direct::run(&context, &mut manifest, speech);
    }
    let model = args.model.as_deref().ok_or("model preflight failed")?;
    llm::run(
        &context,
        &mut manifest,
        speech,
        &args.ollama_endpoint,
        model,
        args.admitted_memory_mib,
        &execution_id,
    )
}

fn check_current(
    context: &LiveContext<'_>,
    manifest: &mut EvidenceManifest,
) -> Result<(), Box<dyn std::error::Error>> {
    let result = (|| {
        let last = parse_snapshot(&face_bytes(context.bin, context.state_dir)?)?;
        if &last != context.face
            || hash_file(context.bin).map_err(|error| error.to_string())? != context.bin_sha256
            || git(context.workspace, &["rev-parse", "HEAD"])? != context.source_commit
            || !git(context.workspace, &["status", "--porcelain"])?.is_empty()
        {
            return Err(
                "source, owner executable, Face, or Host/Boot changed during spoken Mask Play"
                    .to_owned(),
            );
        }
        Ok::<(), String>(())
    })();
    if let Err(error) = result {
        manifest.finish(EvidenceResult::DiagnosticIncomplete)?;
        return Err(error.into());
    }
    Ok(())
}

fn finish_manifest(
    manifest: &mut EvidenceManifest,
    source_commit: &str,
    result: EvidenceResult,
) -> Result<(), Box<dyn std::error::Error>> {
    manifest.finish(result)?;
    verify(&VerificationRequest {
        root: manifest.root().to_path_buf(),
        commit: source_commit.into(),
        result: if result == EvidenceResult::Complete {
            ExpectedEvidenceResult::Complete
        } else {
            ExpectedEvidenceResult::DiagnosticIncomplete
        },
        proof_id: PROOF_ID.into(),
        suite_id: SUITE_ID.into(),
    })?;
    Ok(())
}
