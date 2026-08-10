use crate::cli::GlobalOpts;
use conduit_tongues::{run_speech, OutputCondition, SpeechFault};

pub fn run(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        if !opts.quiet {
            println!("would run pinned Tongues text-to-speech starter as degraded WAV artifact");
        }
        return Ok(());
    }
    let receipt = run_speech(OutputCondition::DegradedWavArtifact, SpeechFault::None)?;
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&receipt)?);
    } else if !opts.quiet {
        println!(
            "Tongues starter completed: plan={} condition={:?} outcome={:?} signs={} kernel-events={} sign-digest={}",
            receipt.plan_id,
            receipt.condition,
            receipt.outcome,
            receipt.sign_count,
            receipt.kernel_event_count,
            receipt.sign_digest
        );
    }
    Ok(())
}
