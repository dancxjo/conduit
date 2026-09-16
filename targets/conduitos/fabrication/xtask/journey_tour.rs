//! Verification of the canonical Tour and its retained shell manifestations.

use std::{collections::BTreeMap, os::unix::net::UnixStream, path::Path, process::Child};

use serde_json::Value;

use super::ConduitosError;

pub(super) fn exercise_remaining(
    qmp: &mut UnixStream,
    reader: &mut super::qmp::Reader,
    serial: &Path,
    child: &mut Child,
    artifacts: &mut super::qemu_artifacts::Artifacts,
) -> Result<(), ConduitosError> {
    let mut opened = 1;
    let mut results = 1;
    let mut transients = 1;
    let mut dismissed = 1;
    for (key, checkpoint, expect_visible_change) in [
        ("f3", "tour-one-exercise-two", true),
        ("f3", "tour-one-exercise-three", true),
        ("f5", "tour-two-exercise-one", true),
        ("f5", "tour-three-exercise-one", true),
        ("f5", "tour-four-exercise-one", true),
        // Both Chapter 4 stages deliberately present the same lesson and result.
        // Their stage and realization records are verified from the guest records;
        // a pixel delta here would depend only on incidental cursor animation.
        ("f3", "tour-four-exercise-two", false),
    ] {
        opened += 1;
        navigate(qmp, reader, serial, child, key, opened)?;
        results += 1;
        transients += 1;
        dismissed += 1;
        run_stage(
            qmp,
            reader,
            serial,
            child,
            artifacts,
            checkpoint,
            results,
            transients,
            dismissed,
            expect_visible_change,
        )?;
    }
    for (checkpoint, count) in [
        ("tour-chapter-five", 8),
        ("tour-chapter-six", 9),
        ("tour-chapter-seven", 10),
    ] {
        opened = count;
        navigate(qmp, reader, serial, child, "f5", opened)?;
        artifacts.capture(qmp, reader, checkpoint, true)?;
    }
    for _ in 0..6 {
        opened += 1;
        navigate(qmp, reader, serial, child, "f6", opened)?;
    }
    run_stage(
        qmp,
        reader,
        serial,
        child,
        artifacts,
        "tour-one-exercise-one-returned",
        8,
        8,
        8,
        true,
    )
}

fn navigate(
    qmp: &mut UnixStream,
    reader: &mut super::qmp::Reader,
    serial: &Path,
    child: &mut Child,
    key: &str,
    opened_count: usize,
) -> Result<(), ConduitosError> {
    super::journey_input::key_pair(qmp, reader, key, "tour-navigation")?;
    super::journey_input::wait_tour_status_count(serial, child, "tour-opened", opened_count)
}

#[allow(clippy::too_many_arguments)]
fn run_stage(
    qmp: &mut UnixStream,
    reader: &mut super::qmp::Reader,
    serial: &Path,
    child: &mut Child,
    artifacts: &mut super::qemu_artifacts::Artifacts,
    checkpoint: &str,
    result_count: usize,
    transient_count: usize,
    dismissed_count: usize,
    expect_visible_change: bool,
) -> Result<(), ConduitosError> {
    super::journey_input::key_pair(qmp, reader, "f10", "tour-run-stage")?;
    super::journey_input::wait_tour_status_count(serial, child, "result-visible", result_count)?;
    artifacts.capture(qmp, reader, checkpoint, expect_visible_change)?;
    super::journey_input::wait_transient_status_count(serial, child, "shown", transient_count)?;
    super::journey_input::key_pair(qmp, reader, "esc", "dismiss-stage-confirmation")?;
    super::journey_input::wait_transient_status_count(serial, child, "dismissed", dismissed_count)
}

pub(super) fn validate(records: &[Value], opened: &Value) -> Result<(), ConduitosError> {
    let by_status = records
        .iter()
        .filter_map(|record| Some((record.get("status")?.as_str()?.to_owned(), record)))
        .collect::<BTreeMap<_, _>>();
    for status in ["tour-opened", "result-visible", "patchbay-open"] {
        if !by_status.contains_key(status) {
            return Err(ConduitosError::refusal(
                "product-journey-tour-stage-missing",
                status,
            ));
        }
    }
    let tour_opened = by_status["tour-opened"];
    let tour_result = by_status["result-visible"];
    let tour_patchbay = by_status["patchbay-open"];
    if text(tour_opened, "specimen_id")? != "canonical-form:meet-one-gear"
        || tour_opened.get("plan_id") != Some(&Value::Null)
        || text(tour_result, "result")? != "HELLO"
        || tour_result.get("plan_id") == Some(&Value::Null)
        || tour_result.get("active_play_id") == Some(&Value::Null)
        || tour_patchbay.get("result") != tour_result.get("result")
        || tour_patchbay.get("plan_id") != Some(&Value::Null)
    {
        return Err(ConduitosError::refusal(
            "product-journey-tour-causality-invalid",
            "Tour open, production Play, result, and Patchbay continuity did not match",
        ));
    }
    for identity in ["profile_id", "build_id", "image_id", "host_id", "boot_id"] {
        let expected = opened.get(identity);
        if expected.is_none()
            || records
                .iter()
                .any(|record| record.get(identity) != expected)
        {
            return Err(ConduitosError::refusal(
                "product-journey-tour-identity-drift",
                identity,
            ));
        }
    }
    for record in records {
        validate_shell(record)?;
    }
    Ok(())
}

pub(super) fn validate_complete_tour(records: &[Value]) -> Result<(), ConduitosError> {
    if records.iter().any(|record| {
        record.get("schema").and_then(Value::as_str) != Some("conduit.conduitos.tour/v2")
    }) {
        return Err(ConduitosError::refusal(
            "product-journey-tour-schema-invalid",
            "every native Tour record must use the complete v2 evidence schema",
        ));
    }
    for chapter in 0..7 {
        if !records.iter().any(|record| {
            record.get("status").and_then(Value::as_str) == Some("tour-opened")
                && record.get("chapter").and_then(Value::as_u64) == Some(chapter)
        }) {
            return Err(ConduitosError::refusal(
                "product-journey-tour-page-missing",
                chapter.to_string(),
            ));
        }
    }
    let results = records
        .iter()
        .filter(|record| record.get("status").and_then(Value::as_str) == Some("result-visible"))
        .collect::<Vec<_>>();
    let expected = [
        (0, 0, "HELLO", "completed", 1, 2),
        (0, 1, "MAKE THIS LOUD", "completed", 1, 1),
        (0, 2, "SOS", "completed", 2, 1),
        (
            1,
            0,
            "Direct and recursive realizations agree",
            "completed",
            2,
            1,
        ),
        (2, 0, "1", "stopped", 2, 1),
        (3, 0, "hello across one Cord", "completed", 1, 1),
        (3, 1, "hello across one Cord", "completed", 1, 1),
    ];
    for (chapter, stage, result, terminal, manifestations, count) in expected {
        let matches = results
            .iter()
            .filter(|record| {
                record.get("chapter").and_then(Value::as_u64) == Some(chapter)
                    && record.get("stage").and_then(Value::as_u64) == Some(stage)
                    && record.get("result").and_then(Value::as_str) == Some(result)
                    && record.get("terminal").and_then(Value::as_str) == Some(terminal)
                    && record.get("manifestations").and_then(Value::as_u64) == Some(manifestations)
            })
            .count();
        if matches != count {
            return Err(ConduitosError::refusal(
                "product-journey-tour-exercise-missing",
                format!("chapter {chapter} stage {stage}: expected {count}, found {matches}"),
            ));
        }
    }
    let comparison = exact_result(&results, 1, 0)?;
    if comparison.get("comparison_expanded_form_id") == Some(&Value::Null)
        || comparison.get("comparison_plan_id") == Some(&Value::Null)
        || comparison.get("comparison_plan_id") == comparison.get("plan_id")
    {
        return Err(ConduitosError::refusal(
            "product-journey-tour-comparison-invalid",
            "direct and recursive identities were not both preserved",
        ));
    }
    for stage in 0..=1 {
        let multi = exact_result(&results, 3, stage)?;
        for field in [
            "source_fragment_id",
            "sink_fragment_id",
            "source_active_play_id",
            "sink_active_play_id",
            "line_id",
        ] {
            if multi.get(field).is_none_or(Value::is_null) {
                return Err(ConduitosError::refusal(
                    "product-journey-tour-two-host-identity-missing",
                    field,
                ));
            }
        }
        if multi.get("source_fragment_id") == multi.get("sink_fragment_id")
            || multi.get("source_active_play_id") == multi.get("sink_active_play_id")
            || multi.get("transferred_values").and_then(Value::as_u64) != Some(1)
        {
            return Err(ConduitosError::refusal(
                "product-journey-tour-two-host-invalid",
                stage.to_string(),
            ));
        }
    }
    Ok(())
}

fn exact_result<'a>(
    results: &'a [&Value],
    chapter: u64,
    stage: u64,
) -> Result<&'a Value, ConduitosError> {
    let matches = results
        .iter()
        .filter(|record| {
            record.get("chapter").and_then(Value::as_u64) == Some(chapter)
                && record.get("stage").and_then(Value::as_u64) == Some(stage)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [record] => Ok(record),
        _ => Err(ConduitosError::refusal(
            "product-journey-tour-result-ambiguous",
            format!("chapter {chapter} stage {stage}"),
        )),
    }
}

fn validate_shell(record: &Value) -> Result<(), ConduitosError> {
    let workspace_surface = text(record, "workspace_surface_id")?;
    let status_surface = text(record, "status_surface_id")?;
    let workspace_presentation = text(record, "workspace_presentation_id")?;
    let status_presentation = text(record, "status_presentation_id")?;
    let workspace_manifestation = text(record, "workspace_manifestation_id")?;
    let status_manifestation = text(record, "status_manifestation_id")?;
    if workspace_surface != "conduitos/shell/workspace"
        || status_surface != "conduitos/shell/status"
        || workspace_surface == status_surface
        || workspace_presentation == status_presentation
        || workspace_manifestation == status_manifestation
        || number(record, "surfaces_composed")? < 2
        || number(record, "damage_count")? == 0
    {
        return Err(ConduitosError::refusal(
            "product-journey-tour-shell-invalid",
            "workspace and status were not distinct simultaneous retained manifestations",
        ));
    }
    Ok(())
}

fn text(record: &Value, field: &str) -> Result<String, ConduitosError> {
    record
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| ConduitosError::refusal("product-journey-tour-shell-field-missing", field))
}

fn number(record: &Value, field: &str) -> Result<u64, ConduitosError> {
    record
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| ConduitosError::refusal("product-journey-tour-shell-field-missing", field))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn distinct_simultaneous_shell_manifestations_are_required() {
        let opened = identity_record("form-opened");
        let mut tour_opened = shell_record("tour-opened");
        tour_opened["specimen_id"] = json!("canonical-form:meet-one-gear");
        tour_opened["plan_id"] = Value::Null;
        let mut result = shell_record("result-visible");
        result["result"] = json!("HELLO");
        result["plan_id"] = json!("plan/1");
        result["active_play_id"] = json!("play/1");
        let mut patchbay = shell_record("patchbay-open");
        patchbay["result"] = json!("HELLO");
        patchbay["plan_id"] = Value::Null;
        let records = vec![tour_opened, result, patchbay];
        validate(&records, &opened).unwrap();

        let mut aliased = records;
        aliased[0]["status_manifestation_id"] = json!("manifestation/workspace");
        assert!(validate(&aliased, &opened).is_err());
    }

    fn identity_record(status: &str) -> Value {
        json!({
            "status": status,
            "profile_id": "profile/1",
            "build_id": "build/1",
            "image_id": "image/1",
            "host_id": "host/1",
            "boot_id": "boot/1"
        })
    }

    fn shell_record(status: &str) -> Value {
        let mut record = identity_record(status);
        record["workspace_surface_id"] = json!("conduitos/shell/workspace");
        record["workspace_presentation_id"] = json!("presentation/workspace");
        record["workspace_manifestation_id"] = json!("manifestation/workspace");
        record["status_surface_id"] = json!("conduitos/shell/status");
        record["status_presentation_id"] = json!("presentation/status");
        record["status_manifestation_id"] = json!("manifestation/status");
        record["surfaces_composed"] = json!(2);
        record["damage_count"] = json!(2);
        record
    }
}
