//! Explicit local recipe selection, never retrospective initial-configuration proof.
use super::*;
const PREFIX: &str = "CONDUIT_NATIVE_COMPONENT_REALIZATION ";
const EFFECTS: [&str; 3] = [CHANGE, "CONDUIT_NATIVE_ACTION ", "CONDUIT_NATIVE_BIRTH "];

#[derive(Clone, Copy)]
struct Choice {
    density: &'static str,
    scale: u64,
    root: &'static str,
    identity: &'static str,
}
const CHOICES: [Choice; 2] = [
    Choice {
        density: "spacious",
        scale: 125,
        root: "creche-mask-columns-spacious-phosphor",
        identity: "conduit.face/spacious-phosphor@1",
    },
    Choice {
        density: "compact",
        scale: 100,
        root: "creche-mask-columns",
        identity: "conduit.face/compact@1",
    },
];
fn effects(text: &str) -> Result<[usize; 3], ConduitosError> {
    let mut counts = [0; 3];
    for (count, prefix) in counts.iter_mut().zip(EFFECTS) {
        *count = rows(text, prefix)?.len();
    }
    Ok(counts)
}
pub(super) fn select(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    arrival: &Snapshot,
) -> Result<Value, ConduitosError> {
    let original_text = text(serial)?;
    let baseline = effects(&original_text)?;
    let boot = super::super::journey_records::boot(&original_text)?
        .ok_or_else(|| refusal("recipe selection Boot absent"))?;
    let mut previous = arrival.record.clone();
    let mut transitions = Vec::new();
    let mut configuration = None;
    let mut seen_shows = vec![arrival.record["show"]["show_id"].clone()];
    for choice in CHOICES {
        let count = rows(&text(serial)?, PREFIX)?.len();
        journey_input::held_key(
            |down| {
                hid_qmp::send_named_keys(
                    stream,
                    reader,
                    &["f6"],
                    down,
                    "creche-controls-recipe-select",
                )
            },
            || {
                journey_input::wait_for_record_retaining_guest(
                    serial,
                    child,
                    "creche-controls-recipe-timeout",
                    "fresh-component",
                    |s| {
                        let records = rows(s, PREFIX)?;
                        let faces = rows(s, "CONDUIT_NATIVE_FACE ")?;
                        let focus = rows(s, FOCUS)?;
                        Ok(records.len() > count
                            && records
                                .last()
                                .zip(faces.last())
                                .zip(focus.last())
                                .is_some_and(|((r, f), focus)| {
                                    r["show_id"] == f["show"]["show_id"]
                                        && r["face_id"] == f["face"]["identity"]
                                        && focus["show_id"] == r["show_id"]
                                        && focus["face_id"] == r["face_id"]
                                        && focus["face_revision"] == r["face_revision"]
                                }))
                    },
                )
            },
        )?;
        let observed = text(serial)?;
        let records = rows(&observed, PREFIX)?;
        let record = one_new(&records, count)?;
        // snapshot reuses the actual full Face/manifestation decoder and checks
        // current focus, component presenter, checked Mask Source and Host/Boot.
        let fresh = snapshot(&observed)?;
        source_unchanged(
            &arrival.record,
            &fresh.record,
            baseline,
            effects(&observed)?,
        )?;
        new_show(&seen_shows, &fresh.record["show"]["show_id"])?;
        fields(record, &previous, &fresh.record, &boot, choice)?;
        seen_shows.push(fresh.record["show"]["show_id"].clone());
        let identity = json!({"component":record["configuration"]["component"],
            "recipe_canonical_sha256":record["configuration"]["recipe_canonical_sha256"],
            "recipe_canonical_bytes":record["configuration"]["recipe_canonical_bytes"]});
        if configuration
            .as_ref()
            .is_some_and(|prior| prior != &identity)
        {
            return Err(refusal(
                "two F6 selections changed checked component or recipe",
            ));
        }
        configuration = Some(identity);
        previous = fresh.record;
        transitions.push(record.clone());
    }
    Ok(
        json!({"schema":"conduit.conduitos/chosen-creche-recipe@1","status":"passed",
        "input":"exactly two real F6 presses; each fresh realization and focus observed",
        "arrival_show_id":arrival.record["show"]["show_id"],"transitions":transitions,
        "selected_show_id":previous["show"]["show_id"],"selected_root":CHOICES[1].root,
        "same_checked_component_and_recipe":configuration,"source_effects":0,
        "initial_expression":"not serialized; no initial configuration or restoration claim",
        "selection_basis":"only the second explicitly selected compact/phosphor100 Show is the control proof baseline",
        "physical_evidence":false}),
    )
}
fn new_show(seen: &[Value], fresh: &Value) -> Result<(), ConduitosError> {
    if fresh.as_str().is_none_or(str::is_empty) || seen.contains(fresh) {
        return Err(refusal("recipe selection replayed a prior Show"));
    }
    Ok(())
}

fn one_new(records: &[Value], before: usize) -> Result<&Value, ConduitosError> {
    if records.len() != before + 1 {
        return Err(refusal(
            "one F6 must produce exactly one new component realization",
        ));
    }
    records
        .last()
        .ok_or_else(|| refusal("new component realization absent"))
}
fn source_unchanged(
    before: &Value,
    after: &Value,
    original: [usize; 3],
    current: [usize; 3],
) -> Result<(), ConduitosError> {
    if before["face"] != after["face"] || original != current {
        return Err(refusal(
            "local recipe selection changed Source or dispatched an owner effect",
        ));
    }
    Ok(())
}
fn fields(
    r: &Value,
    before: &Value,
    fresh: &Value,
    boot: &Value,
    choice: Choice,
) -> Result<(), ConduitosError> {
    let config = &r["configuration"];
    let expression = &r["expression"];
    let show = &fresh["show"];
    if r["schema"] != "conduit.conduitos/component-realization@1"
        || r["selection_entrance"] != "local-creche"
        || r["local_scanout_acknowledged"] != true
        || r["owner_show_acknowledged"] != false
        || !r["fallback"].is_null()
        || r["previous_show_id"] != before["show"]["show_id"]
        || r["show_id"] != show["show_id"]
        || r["show_id"] == r["previous_show_id"]
        || r["face_id"] != fresh["face"]["identity"]
        || r["face_revision"] != fresh["face"]["revision"]
        || ["observation_sequence", "play_sequence"].iter().any(|key| {
            fresh[*key]
                .as_u64()
                .zip(before[*key].as_u64())
                .is_none_or(|(next, prior)| next <= prior)
        })
        || ["host_id", "boot_id", "offer_generation"]
            .iter()
            .any(|key| r[*key] != boot[*key] || show[*key] != boot[*key])
        || r["mask_plan_id"] != show["mask_plan_id"]
        || r["mask_plan_id"] == before["show"]["mask_plan_id"]
        || r["active_play_id"] != show["mask_active_play_id"]
        || r["manifestation_id"] != show["manifestation_id"]
        || config["root"] != choice.root
        || config["invocation_path"] != json!([choice.root, "component"])
        || config["implementation_id"] != custody::PRESENTER
        || config["execution_profile_id"] != "conduitos/bounded-mask@1"
        || config["placement_id"] != show["manifestation"]["placement_id"]
        || ["source_document_id", "checked_plot_id", "fore_id"]
            .iter()
            .any(|key| config["component"][*key].as_str().is_none_or(str::is_empty))
        || config["component"]["source_document_id"] != show["mask_plot"]["source_document_id"]
        || config["recipe_canonical_bytes"]
            .as_u64()
            .is_none_or(|n| n == 0)
        || config["recipe_canonical_sha256"]
            .as_str()
            .is_none_or(|h| h.len() != 64 || !h.bytes().all(|b| b.is_ascii_hexdigit()))
        || expression["outcome"] != "Selected"
        || expression["platform_forced_colors"] != false
        || expression["accessibility_overrides"]
            != json!({"forced_colors":false,"reduced_motion":true,"text_scale":false})
        || expression["expression"]
            != json!({"version":2,"typography":"editorial","density":choice.density,"palette":"phosphor","text_scale_percent":choice.scale,"identity":choice.identity,"motion":"none"})
        || r["render"]["commands"]
            .as_u64()
            .is_none_or(|n| n == 0 || n > conduit_face::MAX_GRAPHICS_COMMANDS as u64)
    {
        return Err(refusal(
            "chosen recipe lacks exact local configuration/expression/Source/Show/Boot binding",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
