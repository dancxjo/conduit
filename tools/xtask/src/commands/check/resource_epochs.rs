//! Explicit private pinned TRAIN input; no download or resource publication.
use crate::{
    cli::GlobalOpts,
    process::{run_suite, run_suite_with_environment, Step, StepError},
    workspace::workspace_root,
};
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct ResourceEpochArgs {
    /// Existing UD English EWT TRAIN file at the documented exact pin.
    #[arg(long)]
    pub train_conllu: PathBuf,
}

const STEPS: &[Step] = &[Step::new(
    "resource-epochs.pinned-admission-source-owner",
    "Reconstruct all pinned rows and verify resource admission, Source snapshots and consuming revisions",
    "cargo",
    &[
        "test", "--locked", "-p", "conduit-language", "--lib",
        "--test", "train_lexicon", "--test", "resource_lexicon_snapshot",
        "--test", "resource_lexicon_revision", "--test", "resource_epoch_owner", "--", "--include-ignored",
        "--nocapture", "--test-threads=1",
    ],
)];

pub fn run(args: ResourceEpochArgs, opts: &GlobalOpts) -> Result<(), StepError> {
    let root = workspace_root().map_err(|e| StepError::prereq("workspace-root", e))?;
    let input = args.train_conllu.canonicalize().map_err(|e| {
        StepError::prereq(
            "resource-epochs.train-input",
            format!("existing pinned TRAIN file: {e}"),
        )
    })?;
    let metadata = input.metadata().map_err(|e| {
        StepError::prereq(
            "resource-epochs.train-input",
            format!("TRAIN metadata: {e}"),
        )
    })?;
    if !metadata.is_file() || metadata.len() != 15_034_493 {
        return Err(StepError::prereq(
            "resource-epochs.train-input",
            "expected the documented 15,034,493-byte TRAIN pin; actual hashes are verified by the conformance tests",
        ));
    }
    run_suite_with_environment(
        STEPS,
        &root,
        opts,
        &[("CONDUIT_EWT_TRAIN_CONLLU", input.as_os_str())],
    )?;
    run_suite(super::prosody_commitment::STEPS, &root, opts)
}
