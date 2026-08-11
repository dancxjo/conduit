use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

use crate::evidence::{self, ExpectedEvidenceResult, VerificationRequest};

#[derive(Args, Debug)]
pub struct EvidenceArgs {
    #[command(subcommand)]
    command: EvidenceCommand,
}

#[derive(Subcommand, Debug)]
enum EvidenceCommand {
    /// Recompute and validate one evidence manifest and its declared files.
    Verify(EvidenceVerifyArgs),
    /// Promote verified complete evidence into a bounded static gallery.
    Gallery(EvidenceGalleryArgs),
}

#[derive(Args, Debug)]
struct EvidenceGalleryArgs {
    /// Complete evidence directory already bound to the checked commit.
    #[arg(long)]
    evidence_root: PathBuf,

    /// Existing or empty gallery root to update atomically by accepted commit.
    #[arg(long)]
    site_root: PathBuf,

    /// Exact 40-character accepted main commit to publish.
    #[arg(long)]
    commit: String,
}

#[derive(Args, Debug)]
struct EvidenceVerifyArgs {
    /// Evidence directory containing manifest.json and its declared outputs.
    #[arg(long)]
    root: PathBuf,

    /// Exact 40-character commit SHA expected in the manifest.
    #[arg(long)]
    commit: String,

    /// Required evidence disposition.
    #[arg(long)]
    result: EvidenceResultArg,

    /// Exact proof identity expected in the manifest.
    #[arg(long, default_value = "browser-host")]
    proof: String,

    /// Exact suite identity expected in the manifest.
    #[arg(long, default_value = "prove.browser-host")]
    suite: String,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum EvidenceResultArg {
    Complete,
    DiagnosticIncomplete,
}

pub fn run(args: EvidenceArgs) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        EvidenceCommand::Verify(args) => {
            let result = match args.result {
                EvidenceResultArg::Complete => ExpectedEvidenceResult::Complete,
                EvidenceResultArg::DiagnosticIncomplete => {
                    ExpectedEvidenceResult::DiagnosticIncomplete
                }
            };
            evidence::verify(&VerificationRequest {
                root: args.root,
                commit: args.commit,
                result,
                proof_id: args.proof,
                suite_id: args.suite,
            })?;
            Ok(())
        }
        EvidenceCommand::Gallery(args) => evidence::publish_gallery(&evidence::GalleryRequest {
            evidence_root: args.evidence_root,
            site_root: args.site_root,
            commit: args.commit,
        })
        .map_err(Into::into),
    }
}
