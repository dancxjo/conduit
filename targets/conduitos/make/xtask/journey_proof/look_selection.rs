//! Actual product preference transitions, joined to complete Face/Show receipts.
use super::super::{qemu_artifacts::Artifacts, qmp};
use super::{face_correspondence, io_error, journey_input, refusal, ConduitosError};
use conduit_face::{Face, Show};
use serde_json::{json, Value};
use std::{fs, os::unix::net::UnixStream, path::Path, process::Child};
const PREFIX: &str = "CONDUIT_NATIVE_COMPONENT_REALIZATION ";

pub(super) fn exercise(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    artifacts: &mut Artifacts,
) -> Result<Value, ConduitosError> {
    let initial = fs::read_to_string(serial).map_err(io_error)?;
    let faces = face_correspondence::records(&initial, "CONDUIT_NATIVE_FACE ")?;
    let origin = faces
        .last()
        .ok_or_else(|| refusal("Look proof lacks initial Face"))?;
    let (face, origin_show) = decode(origin)?;
    let actions = face_correspondence::records(&initial, "CONDUIT_NATIVE_ACTION ")?.len();
    let mut previous = origin_show.clone();
    let mut seen = vec![origin_show.id.clone()];
    let mut component = None;
    let mut recipe = None;
    let mut receipts = Vec::new();
    for (key, density, palette, scale, frame) in [
        (
            "f7",
            "compact",
            "editorial",
            100,
            "creche-editorial-compact",
        ),
        (
            "f6",
            "spacious",
            "editorial",
            125,
            "creche-editorial-spacious",
        ),
        (
            "f7",
            "spacious",
            "phosphor",
            125,
            "creche-engineering-spacious",
        ),
        (
            "f6",
            "compact",
            "phosphor",
            100,
            "creche-engineering-compact",
        ),
    ] {
        let before =
            face_correspondence::records(&fs::read_to_string(serial).map_err(io_error)?, PREFIX)?
                .len();
        journey_input::key_pair(stream, reader, key, "creche-look-selection")?;
        journey_input::wait_for_record_retaining_guest(
            serial,
            child,
            "creche-look-timeout",
            frame,
            |text| Ok(face_correspondence::records(text, PREFIX)?.len() > before),
        )?;
        let text = fs::read_to_string(serial).map_err(io_error)?;
        let records = face_correspondence::records(&text, PREFIX)?;
        if records.len() != before + 1 {
            return Err(refusal("Look key produced duplicate selection records"));
        }
        let record = records.last().unwrap();
        let native = face_correspondence::records(&text, "CONDUIT_NATIVE_FACE ")?;
        let published = native
            .iter()
            .rev()
            .find(|value| value["show"]["show_id"] == record["show_id"])
            .ok_or_else(|| refusal("Look selection lacks acknowledged source Face/Show"))?;
        let (fresh_face, fresh) = decode(published)?;
        let configuration = &record["configuration"];
        let expression = &record["expression"]["expression"];
        if record["schema"] != "conduit.conduitos/component-realization@1"
            || record["selection_entrance"] != "local-creche"
            || record["local_scanout_acknowledged"] != true
            || record["owner_show_acknowledged"] != false
            || record["previous_show_id"] != json!(previous.id)
            || record["face_id"] != face.identity.as_str()
            || record["face_revision"] != face.revision
            || fresh_face != face
            || seen.contains(&fresh.id)
            || fresh.show.plan_id == previous.show.plan_id
            || fresh.show.host_id != origin_show.show.host_id
            || fresh.show.boot_id != origin_show.show.boot_id
            || fresh.show.offer_generation != origin_show.show.offer_generation
            || record["mask_plan_id"] != json!(fresh.show.plan_id)
            || record["active_play_id"] != json!(fresh.show.active_play_id)
            || record["manifestation_id"] != json!(fresh.show.manifestation_id)
            || expression["density"] != density
            || expression["palette"] != palette
            || expression["text_scale_percent"] != scale
            || !record["fallback"].is_null()
            || configuration["root"].as_str().is_none_or(str::is_empty)
            || configuration["component"].is_null()
            || configuration["recipe_canonical_sha256"]
                .as_str()
                .is_none_or(|hash| hash.len() != 64)
            || record["render"]["commands"].as_u64().is_none_or(|count| {
                count == 0 || count > conduit_face::MAX_GRAPHICS_COMMANDS as u64
            })
            || face_correspondence::records(&text, "CONDUIT_NATIVE_ACTION ")?.len() != actions
        {
            return Err(refusal(
                "Look swap changed authority, lost source binding, or missed requested expression",
            ));
        }
        if component
            .as_ref()
            .is_some_and(|first| first != &configuration["component"])
            || recipe
                .as_ref()
                .is_some_and(|first| first != &configuration["recipe_canonical_sha256"])
        {
            return Err(refusal("Look swap changed checked component or recipe"));
        }
        component = Some(configuration["component"].clone());
        recipe = Some(configuration["recipe_canonical_sha256"].clone());
        artifacts.capture(stream, reader, frame, true)?;
        seen.push(fresh.id.clone());
        previous = fresh;
        receipts.push(record.clone());
    }
    if previous.show.plan_id != origin_show.show.plan_id {
        return Err(refusal(
            "Look matrix did not return to the original planned configuration",
        ));
    }
    Ok(
        json!({"schema":"conduit.conduitos/local-look-proof@1", "status":"passed",
        "input":"real-qmp-keyboard", "face_id":face.identity,"face_revision":face.revision,
        "same_checked_component":component,"same_recipe_canonical_sha256":recipe,
        "transitions":receipts,"body_actions":0,"physical_evidence":false,"human_enactment":false}),
    )
}

#[derive(Clone)]
struct NativeShow {
    id: String,
    show: Show,
}

// The installed native-face/v1 receipt deliberately does not duplicate the
// complete planned Mask. Validate its actual compact/manifestation binding;
// the full journey's shared correspondence gate checks the same publications.
fn decode(record: &Value) -> Result<(Face, NativeShow), ConduitosError> {
    let face: Face = serde_json::from_value(record["face"].clone())
        .map_err(|error| refusal(error.to_string()))?;
    face.validate()
        .map_err(|error| refusal(error.to_string()))?;
    let receipt = &record["show"];
    let show: Show = serde_json::from_value(receipt["manifestation"].clone())
        .map_err(|error| refusal(error.to_string()))?;
    let id = receipt["show_id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| refusal("Look snapshot lacks a Show identifier"))?
        .to_owned();
    if record["schema"] != "conduit.native-face/v1"
        || face.basis.body_id.is_some()
        || receipt["face_id"] != face.identity.as_str()
        || receipt["face_revision"] != face.revision
        || show.face_id != face.identity
        || show.face_revision != face.revision
        || show.lifecycle != conduit_face::ManifestationLifecycle::Available
        || show.failure.is_some()
        || receipt["mask_plan_id"] != json!(show.plan_id)
        || receipt["mask_active_play_id"] != json!(show.active_play_id)
        || receipt["mask_plot"].is_null()
        || record["observation_sequence"]
            .as_u64()
            .is_none_or(|n| n == 0)
        || record["play_sequence"] != show.play_sequence
    {
        return Err(refusal(
            "Look proof lacks exact live zero-Body Face/manifestation binding",
        ));
    }
    for field in [
        "face_id",
        "face_revision",
        "manifestation_id",
        "host_id",
        "boot_id",
        "offer_generation",
        "lifecycle",
    ] {
        if receipt[field].is_null() || receipt[field] != receipt["manifestation"][field] {
            return Err(refusal(
                "Look Show metadata disagrees with its manifestation",
            ));
        }
    }
    Ok((face, NativeShow { id, show }))
}
