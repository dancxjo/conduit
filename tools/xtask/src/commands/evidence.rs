use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

use crate::evidence::{self, ExpectedEvidenceResult, VerificationRequest};

#[path = "evidence_home_cross_face.rs"]
mod home_cross_face;
#[path = "evidence_three_body_journey.rs"]
mod three_body_journey;
#[path = "evidence_two_faces.rs"]
mod two_faces;

#[derive(Args, Debug)]
pub struct EvidenceArgs {
    #[command(subcommand)]
    command: EvidenceCommand,
}

#[derive(Subcommand, Debug)]
enum EvidenceCommand {
    /// Publish the bounded Home index after every face supplies exact evidence.
    HomeCrossFace(HomeCrossFaceArgs),
    /// Verify three independently born Bodies against one semantic Journey contract.
    ThreeBodyJourney(ThreeBodyJourneyArgs),
    /// Retain native and pinned-browser pixels for one exact Presentation.
    OneFormTwoFaces(TwoFacesArgs),
    /// Retain four exact checkpoints from the bounded Orbium/Lenia journey.
    LittleLife(EvidenceLittleLifeArgs),
    /// Recompute and validate one evidence manifest and its declared files.
    Verify(EvidenceVerifyArgs),
    /// Promote verified complete evidence into a bounded static gallery.
    Gallery(EvidenceGalleryArgs),
    /// Verify canonical documentation links structurally and against a built gallery.
    DocsVerify(EvidenceDocsVerifyArgs),
}

#[derive(Args, Debug)]
struct HomeCrossFaceArgs {
    /// One conduit.evidence/home-face@1 receipt; exactly six distinct faces are required.
    #[arg(long = "receipt", required = true)]
    receipts: Vec<PathBuf>,

    /// New file that will receive the verified bounded index.
    #[arg(
        long,
        default_value = "target/conduit-evidence/home-cross-face/index.json"
    )]
    output: PathBuf,
}

#[derive(Args, Debug)]
struct ThreeBodyJourneyArgs {
    /// Shared conduit.evidence/semantic-journey-contract@1 document.
    #[arg(long)]
    contract: PathBuf,

    /// One conduit.evidence/body-journey-track@1 manifest; exactly three are required.
    #[arg(long = "track", required = true)]
    tracks: Vec<PathBuf>,

    /// New file that will receive the verified cross-Body index.
    #[arg(long, default_value = "target/journeys/three-bodies/index.json")]
    output: PathBuf,
}

#[derive(Args, Debug)]
struct TwoFacesArgs {
    /// New directory that will receive the bounded sibling evidence manifest.
    #[arg(long, default_value = "target/journeys/one-form-two-faces")]
    output: PathBuf,
}

#[derive(Args, Debug)]
struct EvidenceLittleLifeArgs {
    /// New directory that will receive the exact bounded evidence inventory.
    #[arg(long, default_value = "target/journeys/little-life")]
    output: PathBuf,
}

#[derive(Args, Debug)]
struct EvidenceDocsVerifyArgs {
    /// Repository root containing README.md and docs/visual-evidence.md.
    #[arg(long, default_value = ".")]
    workspace_root: PathBuf,

    /// Built gallery root for exact publication-boundary verification.
    #[arg(long, requires = "commit")]
    site_root: Option<PathBuf>,

    /// Exact accepted commit that the built gallery must advertise.
    #[arg(long, requires = "site_root")]
    commit: Option<String>,
}

#[derive(Args, Debug)]
struct EvidenceGalleryArgs {
    /// Optional complete Patchbay evidence directory bound to the checked commit.
    #[arg(long)]
    evidence_root: Option<PathBuf>,

    /// Optional complete x86_64 ConduitOS console evidence for the same commit.
    #[arg(long)]
    conduitos_evidence_root: Option<PathBuf>,

    /// Optional complete Hears and Speaks audio evidence for the same commit.
    #[arg(long)]
    hears_speaks_evidence_root: Option<PathBuf>,

    /// Optional complete One Form, Two Faces evidence for the same commit.
    #[arg(long)]
    two_faces_evidence_root: Option<PathBuf>,

    /// Optional complete Little Life evolution evidence for the same commit.
    #[arg(long)]
    little_life_evidence_root: Option<PathBuf>,

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
        EvidenceCommand::HomeCrossFace(args) => home_cross_face::run(args.receipts, args.output),
        EvidenceCommand::ThreeBodyJourney(args) => {
            three_body_journey::run(args.contract, args.tracks, args.output)
        }
        EvidenceCommand::OneFormTwoFaces(args) => two_faces::run(args.output),
        EvidenceCommand::LittleLife(args) => super::evidence_little_life::run(args.output),
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
            conduitos_evidence_root: args.conduitos_evidence_root,
            hears_speaks_evidence_root: args.hears_speaks_evidence_root,
            two_faces_evidence_root: args.two_faces_evidence_root,
            little_life_evidence_root: args.little_life_evidence_root,
            site_root: args.site_root,
            commit: args.commit,
        })
        .map_err(Into::into),
        EvidenceCommand::DocsVerify(args) => {
            evidence::verify_documentation_references(&evidence::DocumentationReferenceRequest {
                workspace_root: args.workspace_root,
                site_root: args.site_root,
                commit: args.commit,
            })
            .map_err(Into::into)
        }
    }
}
