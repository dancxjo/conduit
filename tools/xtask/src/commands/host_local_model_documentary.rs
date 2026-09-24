//! Preserve actual Presenter speech, without manufacturing narration from receipts.
use super::PreparedOrifinaJourney;
use conduit_presentation::GeneratedContentRole;
use conduit_std_host::local_model_proof::LocalModelLiveProofReceipt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub(super) fn retain(
    output: &Path,
    journey: &PreparedOrifinaJourney,
    receipt: &LocalModelLiveProofReceipt,
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = output.join("track.json");
    let mut track: Value = serde_json::from_slice(&fs::read(&manifest)?)?;
    // Several moments inspect the same retained state. Preserve the original
    // request and output instead of implying that another inference happened.
    for (step_id, request_index) in [
        ("body.born", 0),
        ("body.awake", 1),
        ("form.used", 1),
        ("body.inspected", 1),
        ("workload.revised", 2),
        ("host.added", 2),
        ("fault.observed", 3),
        ("body.repaired", 4),
        ("body.long-running", 4),
        ("body.lulled", 5),
        ("body.fulfilled", 6),
    ] {
        let request = &journey.requests[request_index];
        let Some(proof) = receipt
            .presenter_requests
            .iter()
            .find(|proof| proof.request_identity == request.request_identity)
        else {
            continue; // Single-state conformance is not a complete documentary.
        };
        let speech = proof
            .manifestation
            .content
            .iter()
            .filter(|segment| segment.role == GeneratedContentRole::Speech)
            .map(|segment| std::str::from_utf8(&segment.bytes))
            .collect::<Result<Vec<_>, _>>()?
            .join("\n\n");
        if speech.is_empty() {
            return Err(format!("{step_id} has no retained outward speech").into());
        }
        let step = track["steps"]
            .as_array_mut()
            .ok_or("missing steps")?
            .iter_mut()
            .find(|step| step["step_id"] == step_id)
            .ok_or("missing step")?;
        let caption = if receipt.proof_class == "ollama-http-fixture" {
            "Deterministic HTTP fixture response—not live model inference."
        } else {
            "The model's actual outward words for this retained Body state."
        };
        for (suffix, class, bytes, description) in [
            ("txt", "transcript", speech.into_bytes(), caption),
            (
                "json",
                "presenter-receipt",
                serde_json::to_vec_pretty(&json!({
                    "request": request, "execution": proof, "proof_class": receipt.proof_class,
                }))?,
                "Exact structured source, request, execution and generated manifestation.",
            ),
        ] {
            let path = format!("artifacts/{step_id}-speech.{suffix}");
            crate::commands::body_journey_track::write_new(&output.join(&path), &bytes)?;
            step["evidence"]
                .as_array_mut()
                .ok_or("missing evidence")?
                .push(json!({
                    "artifact_id": format!("hosted-generative/{step_id}/{class}"),
                    "evidence_class": class, "assertion_rung": "generated-manifestation",
                    "documentary_description": description, "path": path,
                    "sha256": format!("sha256:{:x}", Sha256::digest(&bytes)),
                }));
        }
    }
    fs::write(manifest, serde_json::to_vec_pretty(&track)?)?;
    Ok(())
}
