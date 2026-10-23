//! Actual checked search edits; filtered inventory is never selection authority.
use super::*;
use conduit_face::{Face, FaceDisclosureLevel, FacePropertyValue};
use std::collections::BTreeMap;

#[allow(clippy::too_many_arguments)]
pub(super) fn exercise(
    run: &mut Run<'_>,
    initial: &Snapshot,
    destination: &Path,
    source: &str,
    image: &str,
    metadata: &Value,
    recipe: &Value,
    world: &Value,
    boot: &Value,
) -> Result<JourneyIdentity, ConduitosError> {
    if !current(&initial.face, "creche.search")?.is_empty() {
        return Err(refusal(
            "search proof requires the actual empty initial query",
        ));
    }
    let inventory = choices(&initial.face)?;
    if inventory.len() <= 4 || inventory.len() > 16 {
        return Err(refusal(
            "search proof requires five to sixteen real source choices",
        ));
    }
    let expected_match = inventory
        .iter()
        .filter(|(_, value)| value["name"] == "Patchbay")
        .map(|(id, value)| (id.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    if expected_match.len() != 1 {
        return Err(refusal("ordinary catalog needs one actual Patchbay choice"));
    }
    let selected_review = review(&initial.face);
    let matched_change = run.search_value(b"Patchbay")?;
    let matched = snapshot(&text(run.serial)?)?;
    if choices(&matched.face)? != expected_match {
        return Err(refusal(
            "checked search did not retain the exact offered Plot",
        ));
    }
    preserve_draft(initial, &matched, &selected_review)?;
    primary_review(&matched.face, &selected_review)?;
    run.capture("creche-search-matched")?;
    let matched_controls = controls(&matched.face)?;
    let forward = run.traverse(&matched, &matched_controls, false)?;
    let reverse = run.traverse(&matched, &matched_controls, true)?;
    let matched_pages = run.pages(&matched, "search-matched")?;

    let empty_change = run.search_value(b"zzzzzz")?;
    let empty = snapshot(&text(run.serial)?)?;
    if !choices(&empty.face)?.is_empty() {
        return Err(refusal("unmatched search invented editable choices"));
    }
    preserve_draft(initial, &empty, &selected_review)?;
    primary_review(&empty.face, &selected_review)?;
    run.capture("creche-search-no-matches")?;
    let empty_pages = run.pages(&empty, "search-empty")?;

    let clear_change = run.search_value(b"")?;
    let restored = snapshot(&text(run.serial)?)?;
    preserve_draft(initial, &restored, &selected_review)?;
    if choices(&restored.face)? != inventory
        || controls(&restored.face)? != controls(&initial.face)?
    {
        return Err(refusal(
            "clearing search failed exact inventory restoration",
        ));
    }
    run.capture("creche-search-restored")?;
    let restored_controls = controls(&restored.face)?;
    let restored_forward = run.traverse(&restored, &restored_controls, false)?;
    let restored_reverse = run.traverse(&restored, &restored_controls, true)?;
    let final_text = text(run.serial)?;
    let changes = rows(&final_text, CHANGE)?;
    if changes.len() != 3
        || changes
            .iter()
            .any(|change| change["interaction"]["action_id"] != "creche.search")
    {
        return Err(refusal(
            "search proof performed an unexpected source action",
        ));
    }
    if rows(&final_text, "CONDUIT_BOOT_SIGN ")? != vec![boot.clone()] {
        return Err(refusal("search proof Boot custody changed"));
    }
    for checkpoint in super::super::journey_records::decode(&final_text)? {
        custody::validate_world(metadata, boot, &checkpoint, source, image)?;
    }
    if run.child.try_wait().map_err(io_error)?.is_some() {
        return Err(refusal("guest exited before search proof finished"));
    }
    let proof = json!({
        "schema":"conduit.conduitos/creche-search-proof@1", "status":"passed",
        "proof_class":"freestanding-emulator", "source_commit":source, "image_sha256":image,
        "canonical_build_metadata":metadata, "boot_sign":boot, "initial_world":world,
        "selected_recipe":recipe, "initial_acknowledged_face":initial.record,
        "initial_inventory":inventory, "selected_review":selected_review,
        "matched_change":matched_change, "matched_acknowledged_face":matched.record,
        "matched_forward_focus":forward, "matched_reverse_focus":reverse, "matched_pages":matched_pages,
        "empty_change":empty_change, "empty_acknowledged_face":empty.record, "empty_pages":empty_pages,
        "clear_change":clear_change, "restored_acknowledged_face":restored.record,
        "restored_forward_focus":restored_forward, "restored_reverse_focus":restored_reverse,
        "prepared_frame_budget":frame_budget::summarize(&final_text)?,
        "observations":run.observations, "captures":run.captures, "capture_limit":27,
        "input":"real keyboard replacement drafts and Enter; explicit Backspace stages empty query",
        "draft_custody":"every pre-submit keystroke retains the exact source Face and acknowledged Show",
        "selection_custody":"source review remains exact while editable Plot inventory filters; no Plot toggle submitted",
        "physical_evidence":false, "human_enactment":false
    });
    fs::write(
        destination.join("creche-search-proof.json"),
        serde_json::to_vec_pretty(&proof).map_err(|error| refusal(error.to_string()))?,
    )
    .map_err(io_error)?;
    identity(world)
}

impl Run<'_> {
    fn search_value(&mut self, query: &[u8]) -> Result<Value, ConduitosError> {
        let spelling = if query.is_empty() {
            vec![vec!["backspace".into()]]
        } else {
            keys_for(query)?
        };
        creche_choices::navigate_to(
            self.serial,
            self.child,
            self.stream,
            self.reader,
            "creche.search",
            Some("value"),
            false,
        )?;
        let before = snapshot(&text(self.serial)?)?;
        let count = rows(&text(self.serial)?, CHANGE)?.len();
        for keys in &spelling {
            let borrowed = keys.iter().map(String::as_str).collect::<Vec<_>>();
            self.press(&borrowed)?;
            unchanged(&before, &snapshot(&text(self.serial)?)?)?;
        }
        self.commit(count)?;
        validate_change(&text(self.serial)?, &before, "creche.search", query, count)
    }
}

fn choices(face: &Face) -> Result<BTreeMap<String, Value>, ConduitosError> {
    face.actions
        .iter()
        .filter(|a| a.identity.starts_with("creche.plot."))
        .map(|action| {
            let subject = face
                .subjects
                .iter()
                .find(|subject| subject.identity == action.target)
                .ok_or_else(|| refusal("offered Plot action has no source subject"))?;
            let selected = face
                .properties
                .iter()
                .filter(|p| p.subject == action.target && p.name == "selected")
                .collect::<Vec<_>>();
            let [property] = selected.as_slice() else {
                return Err(refusal("offered Plot selection is ambiguous"));
            };
            let FacePropertyValue::Flag(selected) = &property.value else {
                return Err(refusal("offered Plot selection is not Boolean"));
            };
            Ok((
                action.identity.clone(),
                json!({"name":subject.name,"selected":selected,"availability":action.availability}),
            ))
        })
        .collect()
}
fn review(face: &Face) -> BTreeMap<String, String> {
    face.text
        .iter()
        .filter(|words| words.subject.contains("/birth-review"))
        .map(|words| (words.subject.clone(), words.text.clone()))
        .collect()
}
fn preserve_draft(
    before: &Snapshot,
    after: &Snapshot,
    selected: &BTreeMap<String, String>,
) -> Result<(), ConduitosError> {
    if selected.is_empty()
        || review(&after.face) != *selected
        || current(&before.face, "creche.name")? != current(&after.face, "creche.name")?
        || current(&before.face, "creche.naming")? != current(&after.face, "creche.naming")?
    {
        return Err(refusal(
            "search changed unrelated name/naming or selected-Plot review",
        ));
    }
    Ok(())
}
fn primary_review(face: &Face, selected: &BTreeMap<String, String>) -> Result<(), ConduitosError> {
    // Canonical Face omission means Primary, matching native and spoken Masks.
    if selected.keys().any(|subject| {
        face.disclosures
            .iter()
            .find(|d| d.subject == *subject)
            .map_or(FaceDisclosureLevel::Primary, |d| d.level)
            != FaceDisclosureLevel::Primary
    }) {
        return Err(refusal(
            "filter concealed selected Plot review from primary disclosure",
        ));
    }
    Ok(())
}
