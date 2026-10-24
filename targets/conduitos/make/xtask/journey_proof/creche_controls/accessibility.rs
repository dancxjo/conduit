//! Real local preference overrides and complete primary-control reachability.
use super::*;
const PREFIX: &str = "CONDUIT_NATIVE_COMPONENT_REALIZATION ";

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
    let incomplete = run.incomplete_argument_refusal()?;
    let name = "Café Llanfairpwllgwyngyll long Body name".as_bytes();
    let change = run.rename(name, "creche-a11y-long-name")?;
    let baseline = snapshot(&text(run.serial)?)?;
    if current(&baseline.face, "creche.name")? != name
        || retained_selection(&initial.face) != retained_selection(&baseline.face)
    {
        return Err(refusal(
            "accessibility probe changed unrelated source values",
        ));
    }
    let expected = controls(&baseline.face)?;
    let mut transitions = Vec::new();
    let mut states = Vec::new();
    let mut forward_order = None;
    for (label, keys, typography, density, palette) in [
        (
            "editorial-forced",
            &[("f9", false), ("f10", true)][..],
            "editorial",
            "compact",
            "phosphor",
        ),
        (
            "engineering-forced",
            &[("f8", true), ("f7", true), ("f6", true)][..],
            "engineering",
            "spacious",
            "editorial",
        ),
    ] {
        for (input, forced) in keys {
            transitions.push(select(run, &baseline, input, 200, *forced)?);
        }
        let selected = snapshot(&text(run.serial)?)?;
        let expression = &transitions.last().unwrap()["expression"]["expression"];
        if expression["typography"] != typography
            || expression["density"] != density
            || expression["palette"] != palette
            || controls(&selected.face)? != expected
        {
            return Err(refusal(
                "override lost the requested Look or Source controls",
            ));
        }
        // A fixed Source field anchors each exact cyclic traversal, independent
        // of the page where the previous measured layout ended.
        creche_choices::navigate_to(
            run.serial,
            run.child,
            run.stream,
            run.reader,
            "creche.name",
            Some("value"),
            false,
        )?;
        let selected = snapshot(&text(run.serial)?)?;
        let forward = run.traverse(&selected, &expected, false)?;
        let reverse = run.traverse(&selected, &expected, true)?;
        let order = forward.iter().map(key).collect::<Result<Vec<_>, _>>()?;
        if forward_order.as_ref().is_some_and(|prior| prior != &order) {
            return Err(refusal(
                "Look/override changed the exact keyboard traversal order",
            ));
        }
        forward_order = Some(order);
        let pointer = super::super::creche_pointer::focus_argument(
            run.serial,
            run.child,
            run.stream,
            run.reader,
            "creche.name",
            "value",
        )?;
        run.capture(&format!("creche-a11y-{label}-pointer"))?;
        let pages = run.pages(&selected, label)?;
        states.push(json!({"label":label,"acknowledged_face":selected.record,
            "expected_controls":expected,"forward":forward,"reverse":reverse,
            "pointer":pointer,"primary_pages":pages}));
    }
    // User requirements survive palette/type/density changes, then clearing the
    // user minimum reveals the still-requested spacious 125%, not a forged 100%.
    transitions.push(select(run, &baseline, "f10", 200, false)?);
    transitions.push(select(run, &baseline, "f9", 125, false)?);
    run.capture("creche-a11y-user-overrides-cleared")?;
    let final_state = snapshot(&text(run.serial)?)?;
    let serial = text(run.serial)?;
    let changes = rows(&serial, CHANGE)?;
    if changes.len() != 1
        || changes[0]["interaction"]["action_id"] != "creche.name"
        || rows(&serial, "CONDUIT_BOOT_SIGN ")? != vec![boot.clone()]
        || !rows(&serial, "CONDUIT_NATIVE_BIRTH ")?.is_empty()
    {
        return Err(refusal(
            "accessibility controls dispatched an unexpected Source effect",
        ));
    }
    for checkpoint in super::super::journey_records::decode(&serial)? {
        custody::validate_world(metadata, boot, &checkpoint, source, image)?;
    }
    if run.child.try_wait().map_err(io_error)?.is_some() {
        return Err(refusal(
            "guest exited before accessibility observations completed",
        ));
    }
    let proof = json!({
        "schema":"conduit.conduitos/creche-accessibility-proof@1", "status":"passed",
        "proof_class":"freestanding-emulator", "source_commit":source,"image_sha256":image,
        "canonical_build_metadata":metadata,"boot_sign":boot,"initial_world":world,
        "selected_recipe":recipe,"long_unicode_name":name,"checked_name_change":change,
        "incomplete_argument_refusal":incomplete,"override_transitions":transitions,
        "same_keyboard_order":forward_order,"states":states,
        "final_acknowledged_face":final_state.record,
        "prepared_frame_budget":frame_budget::summarize(&serial)?,
        "observations":run.observations,"captures":run.captures,"capture_limit":27,
        "screenshots":"journey-frames/manifest.json",
        "scope":"complete primary control cycles/pages at 200% in two Looks; actual pointer argument focus and incomplete-input refusal; no Body startup",
        "details_pages":"not traversed by this scenario",
        "pixel_contrast":"actual QMP captures retained for visual inspection; no OCR or inferred pixel contrast",
        "physical_evidence":false,"human_enactment":false
    });
    fs::write(
        destination.join("creche-accessibility-proof.json"),
        serde_json::to_vec_pretty(&proof).map_err(|error| refusal(error.to_string()))?,
    )
    .map_err(io_error)?;
    identity(world)
}

fn select(
    run: &mut Run<'_>,
    baseline: &Snapshot,
    input: &str,
    scale: u64,
    forced: bool,
) -> Result<Value, ConduitosError> {
    let before = snapshot(&text(run.serial)?)?;
    let count = rows(&text(run.serial)?, PREFIX)?.len();
    run.press(&[input])?;
    let serial = text(run.serial)?;
    let records = rows(&serial, PREFIX)?;
    let record = records
        .last()
        .ok_or_else(|| refusal("override realization absent"))?;
    let after = snapshot(&serial)?;
    let expression = &record["expression"];
    let structure = &records[0]["configuration"];
    if records.len() != count + 1
        || record["schema"] != "conduit.conduitos/component-realization@1"
        || record["selection_entrance"] != "local-creche"
        || record["face_id"] != baseline.face.identity.as_str()
        || record["face_revision"] != baseline.face.revision
        || record["host_id"] != after.record["show"]["host_id"]
        || record["boot_id"] != after.record["show"]["boot_id"]
        || record["offer_generation"] != after.record["show"]["offer_generation"]
        || record["mask_plan_id"] != after.record["show"]["manifestation"]["plan_id"]
        || (matches!(input, "f9" | "f10")
            && after.record["show"]["manifestation"]["plan_id"]
                != before.record["show"]["manifestation"]["plan_id"])
        || after.face != baseline.face
        || after.record["show"]["show_id"] == before.record["show"]["show_id"]
        || record["show_id"] != after.record["show"]["show_id"]
        || record["previous_show_id"] != before.record["show"]["show_id"]
        || record["local_scanout_acknowledged"] != true
        || record["owner_show_acknowledged"] != false
        || expression["expression"]["text_scale_percent"] != scale
        || expression["platform_forced_colors"] != forced
        || expression["accessibility_overrides"]["forced_colors"] != forced
        || expression["accessibility_overrides"]["reduced_motion"] != true
        || !record["fallback"].is_null()
        || [
            "component",
            "recipe_canonical_sha256",
            "layout_canonical_sha256",
        ]
        .iter()
        .any(|field| record["configuration"][*field] != structure[*field])
    {
        return Err(refusal(
            "local accessibility selection lost Source/Show/structure or hard constraints",
        ));
    }
    Ok(record.clone())
}
