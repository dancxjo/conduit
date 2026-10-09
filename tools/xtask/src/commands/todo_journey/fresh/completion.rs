//! Retain the supported producer's terminal outcome, then correlate its complete packet.
use super::{capture, now, sha};
use crate::cli::TodoJourneyArgs;
use serde_json::{json, Value};
use std::{fs, path::Path, process::Command};

pub(super) struct CaptureOutcome<'a> {
    pub commit: &'a str,
    pub installation: &'a Value,
    pub started: u128,
    pub result: Result<Value, String>,
}

pub(super) fn finish(
    args: &TodoJourneyArgs,
    repository: &Path,
    output: &Path,
    bin: &Path,
    outcome: CaptureOutcome<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let CaptureOutcome {
        commit,
        installation,
        started,
        result,
    } = outcome;
    let record = json!({
        "schema":"conduit.todo-journey/live-capture@1",
        "capture_entrance":"cargo xtask prove todo-journey",
        "capture_tool_commit":commit,"installed_product_source_commit":installation["release_source_identity"],
        "installed_executable_sha256":sha(&fs::read(bin)?),
        "started_at_unix_ms":started,"finished_at_unix_ms":now()?,
        "observation":result.as_ref().ok(),"error":result.as_ref().err(),
        "acceptance_limits":["Local producer outcome is separate from protected CI and deployed Pages acceptance",
            "Selected speech artifacts do not prove speaker delivery or human listening"]
    });
    fs::write(
        output.join("live-run.json"),
        serde_json::to_vec_pretty(&record)?,
    )?;
    result.map_err(|error| {
        format!(
            "fresh Todo capture failed: {error}; retained at {}",
            output.display()
        )
    })?;
    if args.direct_speech {
        let mut finalize = Command::new("timeout");
        finalize
            .args(["-k", "5s", "30s", "node"])
            .arg(repository.join("proof/browser/todo-journey-packet.mjs"))
            .arg(output)
            .current_dir(repository);
        capture(output, "publication-finalizer", &mut finalize, None)?;
        println!("Retained complete local Todo journey packet at {}. CI and Pages acceptance remain separate.", output.join("publication").display());
    } else {
        let mut partial = record.clone();
        partial["schema"] = json!("conduit.todo-journey/partial-live-capture@1");
        partial["publication_ready"] = json!(false);
        partial["chapter_scope"] = if args.cross_mask_actions {
            json!([
                "birth",
                "three-browser-adds",
                "terminal-complete",
                "browser-observe",
                "terminal-read"
            ])
        } else {
            json!(["birth", "add", "terminal-read"])
        };
        partial["missing_for_publication"] = json!([
            "Direct spoken opening and remaining items",
            "Fresh Owner Boot and browser Host reencounter",
            "Failed checkpoint refusal and exact repair"
        ]);
        fs::write(
            output.join("partial-run.json"),
            serde_json::to_vec_pretty(&partial)?,
        )?;
        println!(
            "Retained partial fresh Todo capture at {}. Publication remains incomplete.",
            output.display()
        );
    }
    Ok(())
}
