use conduit_tongues::{run_speech, OutputCondition, SpeechFault};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut json = false;
    let mut quiet = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--json" => json = true,
            "--quiet" => quiet = true,
            _ => return Err(format!("unsupported argument: {argument}").into()),
        }
    }
    let receipt = run_speech(OutputCondition::DegradedWavArtifact, SpeechFault::None)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&receipt)?);
    } else if !quiet {
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
