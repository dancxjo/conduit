//! Actual carrier/endpoint conformance and explicit private numerical comparison.
use crate::{
    cli::GlobalOpts,
    process::{run_suite, Step, StepError},
};
use std::path::{Path, PathBuf};
pub const PUBLIC: &[Step] = &[Step::new(
    "fargan-carrier-endpoint.source-plan-play",
    "Check actual consecutive epochs, Source endpoint admission and exact PCM framing",
    "cargo", &[
        "test", "--locked", "-p", "conduit-ai", "-p", "conduit-speech", "--features",
        "conduit-ai/kernel-operation-owners,conduit-ai/hosted-catalog-cache,conduit-speech/semantic-bindings,conduit-speech/kernel",
        "--test", "fargan_endpoint_source", "--test", "fargan_carrier_frontend",
        "--test", "fargan_endpoint50", "--", "--nocapture", "--test-threads=1",
    ],
)];
const NEURAL: &[Step] = &[Step::new(
    "fargan-neural-endpoint.private-differential",
    "Compare actual carrier features, own Source warm state and all51neural frames with pinned C",
    "cargo",
    &[
        "test",
        "--locked",
        "-p",
        "conduit-ai",
        "--features",
        "kernel-operation-owners,hosted-catalog-cache",
        "--test",
        "fargan_carrier_frontend",
        "--",
        "--ignored",
        "--exact",
        "neural::actual_carrier_endpoint50_neural_uses_own_state_and_all_source_framing_outputs",
        "--nocapture",
        "--test-threads=1",
    ],
)];
pub fn run_neural(root: &Path, opts: &GlobalOpts) -> Result<(), StepError> {
    if !opts.dry_run {
        for (name, directory, fresh) in [
            ("CONDUIT_FARGAN_DEVELOPMENT_FIXTURE", true, false),
            ("CONDUIT_FARGAN_ENDPOINT50_ORACLE", false, false),
            ("CONDUIT_FARGAN_ENDPOINT50_BUILD_RECEIPT", false, false),
            ("CONDUIT_FARGAN_ENDPOINT50_OUTPUT", true, true),
        ] {
            let path = std::env::var_os(name).map(PathBuf::from).ok_or_else(|| {
                StepError::prereq(
                    "fargan-neural-endpoint.inputs",
                    format!("set {name} to an absolute private path"),
                )
            })?;
            let valid = path.is_absolute()
                && if fresh {
                    !path.exists() && path.parent().is_some_and(Path::is_dir)
                } else if directory {
                    path.is_dir()
                } else {
                    path.is_file()
                };
            if !valid {
                return Err(StepError::prereq(
                    "fargan-neural-endpoint.inputs",
                    format!(
                        "invalid {name}; use existing absolute inputs and a fresh output directory"
                    ),
                ));
            }
        }
    }
    run_suite(NEURAL, root, opts)
}
