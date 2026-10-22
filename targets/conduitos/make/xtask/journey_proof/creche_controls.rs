//! Separate finite real-QMP Crèche control proof. No product/UI state is added.
use super::super::{qemu_artifacts::Artifacts, qmp};
use super::{
    creche_choices, face_correspondence, hid_qmp, io_error, journey_input, refusal, ConduitosError,
    JourneyIdentity,
};
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, os::unix::net::UnixStream, path::Path, process::Child};
mod custody;
mod evidence;
mod navigation;
use evidence::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn exercise(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    artifacts: &mut Artifacts,
    destination: &Path,
    source: &str,
    image: &str,
    metadata: &Value,
) -> Result<JourneyIdentity, ConduitosError> {
    // Ready alone precedes the final component publication on some boots.
    journey_input::wait_for_record_retaining_guest(
        serial,
        child,
        "creche-controls-ready-timeout",
        "current-world",
        |text| {
            let faces = rows(text, "CONDUIT_NATIVE_FACE ")?;
            let journeys = super::journey_records::decode(text)?;
            Ok(faces.last().zip(journeys.last()).is_some_and(|(f, j)| {
                j["status"] == "world"
                    && j["body_id"].is_null()
                    && j["presenter_implementation_id"] == custody::PRESENTER
                    && f["show"]["manifestation"]["presenter_implementation_id"]
                        == custody::PRESENTER
                    && j["face_id"] == f["face"]["identity"]
                    && j["manifestation_id"] == f["show"]["manifestation_id"]
            }) && faces
                .last()
                .zip(rows(text, FOCUS)?.last())
                .is_some_and(|(f, focus)| {
                    focus["face_id"] == f["face"]["identity"]
                        && focus["show_id"] == f["show"]["show_id"]
                }))
        },
    )?;
    let initial = snapshot(&text(serial)?)?;
    let initial_text = text(serial)?;
    let boot = super::journey_records::boot(&initial_text)?
        .ok_or_else(|| refusal("controls Boot identity absent"))?;
    let world = super::journey_records::decode(&initial_text)?
        .pop()
        .ok_or_else(|| refusal("controls world absent"))?;
    custody::validate(metadata, &boot, &world, &initial.record, source, image)?;
    let membership = members(&initial.face)?;
    let original = current(&initial.face, "creche.naming")?;
    let observed_name = current(&initial.face, "creche.name")?;
    let name = b"Naming proof".to_vec();
    let expected = controls(&initial.face)?;
    if expected.is_empty() || expected.len() > 64 {
        return Err(refusal(
            "actual control inventory exceeds finite traversal profile",
        ));
    }
    let mut run = Run {
        serial,
        child,
        stream,
        reader,
        artifacts,
        captures: 1,
        observations: Vec::new(),
    };
    run.capture("creche-controls-ready")?;
    let forward = run.traverse(&initial, &expected, false)?;
    let reverse = run.traverse(&initial, &expected, true)?;
    let primary = run.pages(&initial, "primary")?;
    run.press(&["f2"])?;
    let details = run.pages(&initial, "details")?;
    run.press(&["f2"])?;
    let probe_name_change = run.rename(&name, "creche-controls-probe-name")?;
    let baseline = snapshot(&text(run.serial)?)?;
    if retained_selection(&initial.face) != retained_selection(&baseline.face)
        || current(&baseline.face, "creche.naming")? != original
    {
        return Err(refusal(
            "checked probe rename changed unrelated draft state",
        ));
    }
    run.naming(&membership, b"welsh", "creche-naming-welsh")?;
    let welsh = snapshot(&text(run.serial)?)?;
    let suggested = current(&welsh.face, "creche.name")?;
    if suggested.is_empty() {
        return Err(refusal("owner naming suggestion empty"));
    }
    let pointer = super::creche_pointer::focus_argument(
        run.serial,
        run.child,
        run.stream,
        run.reader,
        "creche.naming",
        "value",
    )?;
    run.capture("creche-naming-pointer-focused")?;
    run.naming(&membership, &original, "creche-naming-tradition-restored")?;
    let restored_change = run.rename(&name, "creche-naming-exact-name-restored")?;
    let final_state = snapshot(&text(run.serial)?)?;
    if current(&final_state.face, "creche.naming")? != original
        || current(&final_state.face, "creche.name")? != name
        || retained_selection(&initial.face) != retained_selection(&final_state.face)
    {
        return Err(refusal(
            "naming proof failed exact name/tradition/selection/search restoration",
        ));
    }
    let final_text = text(run.serial)?;
    let boots = rows(&final_text, "CONDUIT_BOOT_SIGN ")?;
    if boots.len() != 1 || boots[0] != boot {
        return Err(refusal("controls Boot history changed or duplicated"));
    }
    let journeys = super::journey_records::decode(&final_text)?;
    for checkpoint in &journeys {
        custody::validate_world(metadata, &boot, checkpoint, source, image)?;
    }
    custody::component(&final_state.record)?;
    let parts = [
        json!({"schema":"conduit.conduitos/creche-controls-proof@1","status":"passed","proof_class":"freestanding-emulator",
            "source_commit":source,"image_sha256":image,"host_id":boot["host_id"],"boot_id":boot["boot_id"],
            "profile_id":boot["profile_id"],"build_id":boot["build_id"],"logical_image_binding":boot["image_binding"],
            "final_image_id":metadata["target_build_manifest"]["image_id"],"canonical_build_metadata":metadata,"boot_sign":boot,"initial_world":world,
        }),
        json!({
            "route":"acknowledged component presenter and checked Mask Plot Source; exact initial recipe and visual-fallback configuration are not serialized",
            "initial_acknowledged_face":initial.record,"source_action_inventory":initial.face.actions,"source_subject_words":initial.face.text,
            "expected_controls":expected,"forward_focus":forward,"reverse_focus":reverse,"primary_pages":primary,"details_pages":details,
        }),
        json!({
            "naming_membership":membership,"original_tradition":original,"observed_original_name":observed_name,"probe_baseline_name":name,"checked_probe_name_change":probe_name_change,"restoration_basis":"checked ASCII probe draft; original observed Unicode name is preserved as evidence, not retyped","welsh_suggested_name":suggested,
            "pointer_focus":pointer,"restored_name_change":restored_change,"final_acknowledged_face":final_state.record,
        }),
        json!({
        "observations":run.observations,"captures":run.captures,"capture_limit":27,"screenshots":"journey-frames/manifest.json",
        "capture_custody":"original QMP PNG and complete serial-prefix offsets/hashes via existing Artifacts",
        "rendered_rows_and_lane_wording":"unsupported: product focus receipt exposes hit/page/display bounds, not rendered row commands/text/lane mapping",
        "details_view_state":"F2 command and subsequent focus/page observations retained; no separate product Details-mode flag exists",
        "component_realization_records":rows(&text(run.serial)?,"CONDUIT_NATIVE_COMPONENT_REALIZATION ")?,
        "historical_renderer_reproduced":false,"fallback_proof":"unsupported: this entrance does not independently admit a fallback profile or infer renderer route from image chrome","physical_evidence":false,"human_enactment":false,"full_6158_acceptance":false}),
    ];
    // Keep the same flat schema without increasing the crate macro limit.
    let mut fields = serde_json::Map::new();
    for part in parts {
        if let Value::Object(part) = part {
            fields.extend(part);
        }
    }
    let proof = Value::Object(fields);
    fs::write(
        destination.join("creche-controls-proof.json"),
        serde_json::to_vec_pretty(&proof).map_err(|e| refusal(e.to_string()))?,
    )
    .map_err(io_error)?;
    let string = |key: &str| {
        world[key]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(String::from)
            .ok_or_else(|| refusal(format!("controls identity absent: {key}")))
    };
    Ok(JourneyIdentity {
        profile_id: string("profile_id")?,
        build_id: string("build_id")?,
        host_id: string("host_id")?,
        boot_id: string("boot_id")?,
        spore_join: None,
    })
}
struct Run<'a> {
    serial: &'a Path,
    child: &'a mut Child,
    stream: &'a mut UnixStream,
    reader: &'a mut qmp::Reader,
    artifacts: &'a mut Artifacts,
    captures: usize,
    observations: Vec<Value>,
}

fn retained_selection(face: &conduit_face::Face) -> Value {
    json!({"selected":face.properties.iter().filter(|p|p.name=="selected").collect::<Vec<_>>(),"search":current(face,"creche.search").ok()})
}
fn keys_for(bytes: &[u8]) -> Result<Vec<Vec<String>>, ConduitosError> {
    if bytes.is_empty() || bytes.len() > 64 {
        return Err(refusal("probe name outside finite HID input profile"));
    }
    bytes
        .iter()
        .map(|b| {
            Ok(match b {
                b'a'..=b'z' | b'0'..=b'9' => vec![char::from(*b).to_string()],
                b'A'..=b'Z' => vec![
                    "shift".into(),
                    char::from(b.to_ascii_lowercase()).to_string(),
                ],
                b' ' => vec!["spc".into()],
                b'-' => vec!["minus".into()],
                b'\'' => vec!["apostrophe".into()],
                _ => {
                    return Err(refusal(
                        "probe name needs unsupported HID spelling; no Source changed",
                    ))
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_probe_spelling_is_bounded_and_never_normalizes_unicode() {
        assert_eq!(keys_for(b"Naming proof").unwrap()[0], ["shift", "n"]);
        assert!(keys_for("Dafydd ŵ".as_bytes()).is_err());
        assert!(keys_for(&[b'a'; 65]).is_err());
        assert!(keys_for(b"").is_err());
        assert!(keys_for(b"wrong\ninput").is_err());
    }
}
