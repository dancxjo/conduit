use std::{env, fs, path::PathBuf};

use conduit_presentation::PresentationRole;
use patchbay_html::{
    body_workbench_fixture_snapshot, text_lab_split_snapshot, BrowserBodyWorkbenchEntrance,
    RendererSnapshot, MAX_SNAPSHOT_BYTES,
};
use serde_json::{json, Value};

const LEDGER_SCHEMA: &str = "conduit.patchbay/body-workbench-proof-ledger@1";
const MAX_LEDGER_BYTES: usize = 4 * MAX_SNAPSHOT_BYTES;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = output_path()?;
    let hosted = body_workbench_fixture_snapshot(true)?;
    let external = body_workbench_fixture_snapshot(false)?;
    let follow = text_lab_split_snapshot("http://127.0.0.1:1")?;

    let hosted_entry = workbench_entry("hosted", &hosted)?;
    let external_entry = workbench_entry("external", &external)?;
    prove_equivalent_body_truth(&hosted_entry, &external_entry)?;
    let follow_entry = follow_entry(&follow)?;

    let ledger = json!({
        "schema": LEDGER_SCHEMA,
        "bounds": {
            "workbench_entrances": 2,
            "maximum_bytes": MAX_LEDGER_BYTES,
        },
        "workbench": [hosted_entry, external_entry],
        "program_to_body_follow": follow_entry,
        "manifestations": {
            "browser": "pinned Chromium; one worker; zero retries",
            "native": "deterministic semantic and composition tests",
            "linear": "the same ordered biography records",
            "exact": "the same Body, Sign, record, Plan, Cord, and Line identities",
        },
        "close_or_remove": {
            "serialized_evidence_owner": "Body biography evidence",
            "manifestation_may_mutate_evidence": false,
            "byte_identity_required": true,
        },
    });
    let encoded = serde_json::to_vec_pretty(&ledger)?;
    if encoded.len() > MAX_LEDGER_BYTES {
        return Err(format!(
            "workbench proof identity ledger is {} bytes; maximum is {MAX_LEDGER_BYTES}",
            encoded.len()
        )
        .into());
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, &encoded)?;
    println!("PATCHBAY_BODY_WORKBENCH_LEDGER={}", output.display());
    Ok(())
}

fn output_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let output = arguments
        .next()
        .ok_or("usage: patchbay-body-workbench-proof OUTPUT.json")?;
    if arguments.next().is_some() {
        return Err("usage: patchbay-body-workbench-proof OUTPUT.json".into());
    }
    Ok(output.into())
}

fn workbench_entry(
    label: &str,
    snapshot: &RendererSnapshot,
) -> Result<Value, Box<dyn std::error::Error>> {
    let workbench = snapshot
        .body_workbench
        .as_ref()
        .ok_or("fixture did not attach a Body workbench")?;
    let evidence: Value = serde_json::from_slice(&workbench.encoded_evidence)?;
    let history = workbench.history["entries"]
        .as_array()
        .ok_or("workbench history entries are absent")?;
    if history.is_empty() || history.len() > conduit_body::MAX_BODY_BIOGRAPHY_RECORDS {
        return Err("workbench history is outside its finite record bound".into());
    }
    let navigation = snapshot
        .navigation
        .as_ref()
        .ok_or("workbench navigation is absent")?;
    let entrance = match &workbench.entrance {
        BrowserBodyWorkbenchEntrance::Hosted {
            plan_id,
            implementation_id,
        } => json!({
            "kind": "hosted",
            "plan_id": plan_id,
            "implementation_id": implementation_id,
        }),
        BrowserBodyWorkbenchEntrance::ExternalReader => json!({
            "kind": "external-reader",
            "plan_id": null,
            "implementation_id": null,
        }),
    };
    let records = evidence["records"]
        .as_array()
        .ok_or("Body evidence records are absent")?;
    let exact_records = records
        .iter()
        .map(|record| {
            json!({
                "sequence": record["sequence"],
                "sign_id": record["sign_id"],
                "kind": record["kind"],
            })
        })
        .collect::<Vec<_>>();
    let linear = history
        .iter()
        .map(|entry| entry["linear"].clone())
        .collect::<Vec<_>>();
    Ok(json!({
        "label": label,
        "snapshot_schema": snapshot.schema,
        "evidence_schema": evidence["schema"],
        "evidence_revision": workbench.evidence_revision,
        "evidence_bytes": workbench.encoded_evidence,
        "body_id": workbench.body_id,
        "source_document_id": evidence["body"]["source_document_id"],
        "checked_form_id": evidence["body"]["checked_form_id"],
        "graduation": evidence["graduation"],
        "attachment": entrance,
        "current": workbench.current,
        "history": {
            "place": workbench.history["place"],
            "aspect": workbench.history["aspect"],
            "records": exact_records,
            "linear": linear,
            "authoritative_clock_time": null,
        },
        "navigation": {
            "identity": navigation.navigation.identity,
            "projection_identity": navigation.projection.identity,
            "places": navigation.navigation.places,
            "cursor": navigation.cursor,
        },
        "selected_subjects": {
            "program": navigation.navigation.places[0].root_subject,
            "body": navigation.navigation.places[1].root_subject,
            "history_signs": history.iter().map(|entry| entry["inspect"]["subject_identity"].clone()).collect::<Vec<_>>(),
        },
    }))
}

fn prove_equivalent_body_truth(hosted: &Value, external: &Value) -> Result<(), &'static str> {
    for path in [
        "/body_id",
        "/source_document_id",
        "/checked_form_id",
        "/current/program",
        "/current/lifecycle",
        "/current/current_hosts",
        "/history/place",
        "/history/aspect",
    ] {
        if hosted.pointer(path) != external.pointer(path) {
            return Err("hosted and external entrances projected different Body truth");
        }
    }
    let hosted_records = hosted["history"]["records"]
        .as_array()
        .ok_or("hosted records are absent")?;
    let external_records = external["history"]["records"]
        .as_array()
        .ok_or("external records are absent")?;
    if hosted_records.len() != external_records.len()
        || hosted_records
            .iter()
            .zip(external_records)
            .any(|(left, right)| {
                left["sequence"] != right["sequence"] || left["sign_id"] != right["sign_id"]
            })
    {
        return Err("hosted and external entrances projected different biography identity");
    }
    Ok(())
}

fn follow_entry(snapshot: &RendererSnapshot) -> Result<Value, Box<dyn std::error::Error>> {
    let navigation = snapshot
        .navigation
        .as_ref()
        .ok_or("planned Follow navigation is absent")?;
    let exact = |name: &str| {
        snapshot
            .presentation
            .properties
            .iter()
            .filter(|property| property.name == name)
            .map(|property| json!({"subject": property.subject, "value": property.value}))
            .collect::<Vec<_>>()
    };
    let subjects = |role| {
        snapshot
            .presentation
            .subjects
            .iter()
            .filter(|subject| subject.role == role)
            .map(|subject| subject.identity.clone())
            .collect::<Vec<_>>()
    };
    let mut plan_ids = exact("plan-id");
    if let Some(plan_id) = &snapshot.presentation.basis.plan_id {
        plan_ids.push(json!({"subject": "presentation-basis", "value": plan_id}));
    }
    plan_ids.extend(
        subjects(PresentationRole::Plan)
            .into_iter()
            .map(|identity| json!({"subject": identity, "value": identity})),
    );
    let mut cord_ids = exact("cord-id");
    cord_ids.extend(
        subjects(PresentationRole::Cord)
            .into_iter()
            .map(|identity| json!({"subject": identity, "value": identity})),
    );
    let mut line_ids = exact("line-id");
    line_ids.extend(
        subjects(PresentationRole::Line)
            .into_iter()
            .map(|identity| json!({"subject": identity, "value": identity})),
    );
    if plan_ids.is_empty() || cord_ids.is_empty() || line_ids.is_empty() {
        return Err(format!(
            "planned Follow fixture identity counts: plans={} cords={} lines={}",
            plan_ids.len(),
            cord_ids.len(),
            line_ids.len()
        )
        .into());
    }
    Ok(json!({
        "presentation_identity": snapshot.presentation.identity,
        "basis": snapshot.presentation.basis,
        "navigation_identity": navigation.navigation.identity,
        "follows": navigation.navigation.follows,
        "plan_ids": plan_ids,
        "cord_ids": cord_ids,
        "line_ids": line_ids,
        "form_properties_contain_realization_ids": snapshot.presentation.properties.iter().any(|property| {
            property.subject.starts_with("form/") && matches!(property.name.as_str(), "plan-id" | "cord-id" | "line-id")
        }),
    }))
}
