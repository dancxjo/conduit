//! Reconstruct linear meaning from the exact Faces emitted by the live guest.
//! This creates no owner state and does not manufacture another Mask Show.
use conduit_face::{render_linear_face, Face, FaceAction, FaceActionAvailability, FaceInteraction};
use serde_json::{json, Value};
use std::{fs, path::Path};

use super::{io_error, refusal, ConduitosError};

const FACE_PREFIX: &str = "CONDUIT_NATIVE_FACE ";
const ACTION_PREFIX: &str = "CONDUIT_NATIVE_ACTION ";
const BIRTH_PREFIX: &str = "CONDUIT_NATIVE_BIRTH ";

#[path = "birth_evidence.rs"]
mod birth_evidence;
const MAX_RECORDS: usize = 256;

pub(super) fn records(serial: &str, prefix: &str) -> Result<Vec<Value>, ConduitosError> {
    let mut records = Vec::new();
    for line in serial
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
    {
        let Some(encoded) = line.strip_prefix(prefix) else {
            continue;
        };
        if records.len() == MAX_RECORDS || encoded.len() > 2 * 1024 * 1024 {
            return Err(refusal("native correspondence record bound exceeded"));
        }
        records.push(serde_json::from_str(encoded).map_err(|error| refusal(error.to_string()))?);
    }
    Ok(records)
}

pub(super) fn refused_wake_count(serial: &str) -> Result<usize, ConduitosError> {
    Ok(records(serial, ACTION_PREFIX)?
        .iter()
        .filter(|record| {
            record["requested_intent"] == "conduit.intent/wake@1"
                && record["disposition"].get("Refused").is_some()
        })
        .count())
}

pub(super) fn retain(
    serial: &str,
    destination: &Path,
    source: &str,
    image: &str,
) -> Result<(), ConduitosError> {
    let faces = records(serial, FACE_PREFIX)?;
    let actions = records(serial, ACTION_PREFIX)?;
    let births = records(serial, BIRTH_PREFIX)?;
    validate_receipt_order(serial)?;
    if faces.is_empty() || actions.is_empty() {
        return Err(refusal(
            "actual native Face and action receipts are required",
        ));
    }
    let mut paired = Vec::new();
    let mut decoded = Vec::new();
    for record in &faces {
        if record["schema"] != "conduit.native-face/v1" {
            return Err(refusal("unexpected native Face schema"));
        }
        let face: Face = serde_json::from_value(record["face"].clone())
            .map_err(|error| refusal(error.to_string()))?;
        face.validate()
            .map_err(|error| refusal(error.to_string()))?;
        let show = &record["show"];
        if show["face_id"] != json!(face.identity)
            || show["face_revision"] != face.revision
            || show["show_id"].as_str().is_none_or(str::is_empty)
            || show["mask_plan_id"].as_str().is_none_or(str::is_empty)
            || show["mask_active_play_id"]
                .as_str()
                .is_none_or(str::is_empty)
            || show["manifestation_id"].as_str().is_none_or(str::is_empty)
            || show["lifecycle"] != "Available"
            || show["mask_plot"].is_null()
            || record["observation_sequence"]
                .as_u64()
                .is_none_or(|value| value == 0)
            || record["play_sequence"]
                .as_u64()
                .is_none_or(|value| value == 0)
        {
            return Err(refusal(
                "native acknowledged Show lacks exact Face/Mask identity",
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
            if show[field].is_null() || show["manifestation"][field] != show[field] {
                return Err(refusal(
                    "retained manifestation differs from acknowledged Show encounter",
                ));
            }
        }
        if show["manifestation"]["plan_id"] != show["mask_plan_id"]
            || show["manifestation"]["active_play_id"] != show["mask_active_play_id"]
            || !show["manifestation"]["failure"].is_null()
        {
            return Err(refusal(
                "native Show lacks its successful exact Mask manifestation",
            ));
        }
        let linear = render_linear_face(&face).map_err(|error| refusal(error.to_string()))?;
        paired.push(json!({"native":record,"linear":{
            "proof_class":"deterministic-semantic-reconstruction",
            "face_id":linear.face_id,"face_revision":linear.revision,
            "lines":linear.lines,"encoded_bytes":linear.encoded_bytes,
            "acknowledged_show":null,"input_enactment":false,
            "actions":face.actions,"basis":face.basis,"context":face.interaction_context,
        }}));
        decoded.push(face);
    }
    let birth = birth_evidence::validate(&births, &faces, &decoded, &actions)?;
    let mut accepted = 0;
    let mut refused = 0;
    let mut refusal_seen = false;
    let mut recovered = false;
    let mut body = None;
    for action in &actions {
        if action["schema"] != "conduit.native-action/v1" {
            return Err(refusal("unexpected native action schema"));
        }
        let index = faces
            .iter()
            .rposition(|record| {
                record["show"]["show_id"] == action["show_id"]
                    && record["show"]["face_id"] == action["face_id"]
                    && record["show"]["face_revision"] == action["face_revision"]
            })
            .ok_or_else(|| refusal("native action has no retained source Face/Show"))?;
        let face = &decoded[index];
        validate_owner_effect(action)?;
        validate_encounter(&faces[index]["show"], &action["before"])?;
        let current_body = &action["before"]["body_id"];
        if json!(face.basis.body_id) != *current_body {
            return Err(refusal("action Face does not observe its owner Body"));
        }
        if body.as_ref().is_some_and(|prior| prior != current_body) {
            return Err(refusal("native lifecycle actions changed Body identity"));
        }
        body = Some(current_body.clone());
        if action["disposition"].get("Accepted").is_some() {
            validate_source_interaction(face, action)?;
            accepted += 1;
            recovered |= refusal_seen;
        } else if let Some(rejection) = action["disposition"].get("Refused") {
            if rejection["reason"].as_str().is_none_or(str::is_empty)
                || action["before"] != action["after"]
                || !face.actions.iter().any(|candidate| {
                    candidate.intent == action["requested_intent"].as_str().unwrap_or("")
                        && !matches!(candidate.availability, FaceActionAvailability::Available)
                })
            {
                return Err(refusal("installed unavailable refusal changed owner or lacks unavailable source action"));
            }
            refused += 1;
            refusal_seen = true;
        } else {
            return Err(refusal("native action lacks terminal disposition"));
        }
    }
    if accepted == 0 || refused == 0 || !recovered {
        return Err(refusal(
            "native journey needs accepted, unavailable-refused, and fresh recovery actions",
        ));
    }
    let evidence = json!({"schema":"conduit.native-shell/paired-linear@1",
        "source_commit":source,"image_sha256":image,"proof_class":"freestanding-emulator",
        "body_id":body,"birth":birth,"birth_count":1,"paired_faces":paired,"actions":actions,
        "accepted_action_count":accepted,"refused_action_count":refused,"fresh_action_after_refusal":recovered,
        "screenshots":"journey-frames/manifest.json",
        "input_evidence":{"class":"real-qmp-keyboard-and-pointer","transcript":"journey-qmp.log",
            "unavailable_attempt":{"qmp_key":"f4","hid_usage":61,"binding":"wake",
                "requested_intent":"conduit.intent/wake@1","label":"journey-unavailable-wake",
                "precondition":"quiescent-awaiting-input","completion":"CONDUIT_PRODUCT_ACTION handled wake"}},
        "pointer":super::creche_pointer::receipt(serial)?,
        "browser":{"status":"unsupported","reason":"no installed source-equivalent native shell browser route"},
        "stale_show_replay":{"coverage":"separate deterministic admission suite; not asserted by this harness","installed_enactment":false},
        "parent_5356_acceptance":"open","physical_evidence":false,"human_enactment":false});
    fs::write(
        destination,
        serde_json::to_vec_pretty(&evidence).map_err(|error| refusal(error.to_string()))?,
    )
    .map_err(io_error)
}

fn validate_receipt_order(serial: &str) -> Result<(), ConduitosError> {
    let mut shown = std::collections::BTreeSet::new();
    let mut current_show: Option<String> = None;
    for line in serial
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
    {
        if let Some(encoded) = line.strip_prefix(FACE_PREFIX) {
            let record: Value =
                serde_json::from_str(encoded).map_err(|error| refusal(error.to_string()))?;
            let show_id = record["show"]["show_id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or_else(|| refusal("native Show receipt lacks identity"))?;
            if !shown.insert(show_id.to_owned()) {
                return Err(refusal("native Show identity was published more than once"));
            }
            current_show = Some(show_id.to_owned());
        }
        if let Some(encoded) = line
            .strip_prefix(ACTION_PREFIX)
            .or_else(|| line.strip_prefix(BIRTH_PREFIX))
        {
            let record: Value =
                serde_json::from_str(encoded).map_err(|error| refusal(error.to_string()))?;
            if current_show.as_deref() != record["show_id"].as_str() || current_show.is_none() {
                return Err(refusal(
                    "action lacks its current unconsumed acknowledged native Show",
                ));
            }
            // Accepted submissions consume the Mask session. Unavailable
            // shortcuts immediately replace their Show with refusal context,
            // so recovery must also originate from a fresh acknowledged Show.
            current_show = None;
        }
    }
    Ok(())
}

fn validate_encounter(show: &Value, owner: &Value) -> Result<(), ConduitosError> {
    for field in ["host_id", "boot_id", "offer_generation"] {
        if show[field].is_null() || show[field] != owner[field] {
            return Err(refusal(
                "action owner differs from acknowledged Host/Boot/offer encounter",
            ));
        }
    }
    Ok(())
}

fn validate_source_interaction(face: &Face, action: &Value) -> Result<(), ConduitosError> {
    let interaction: FaceInteraction = serde_json::from_value(action["interaction"].clone())
        .map_err(|error| refusal(error.to_string()))?;
    let semantic = face
        .actions
        .iter()
        .find(|candidate| {
            candidate.identity == interaction.action_id
                && candidate.target == interaction.target
                && candidate.intent == action["requested_intent"].as_str().unwrap_or("")
                && matches!(candidate.availability, FaceActionAvailability::Available)
        })
        .ok_or_else(|| refusal("accepted interaction lacks current available source action"))?;
    if interaction.show_id != action["show_id"].as_str().unwrap_or("")
        || interaction.face_id != face.identity.as_str()
        || interaction.face_revision != face.revision
    {
        return Err(refusal(
            "accepted interaction does not bind the acknowledged Face/Show",
        ));
    }
    validate_arguments(semantic, &interaction)?;
    Ok(())
}

fn validate_owner_effect(action: &Value) -> Result<(), ConduitosError> {
    let before = &action["before"];
    let after = &action["after"];
    for identity in ["body_id", "born_sign_id", "host_id", "boot_id"] {
        if before[identity].as_str().is_none_or(str::is_empty)
            || before[identity] != after[identity]
        {
            return Err(refusal(format!(
                "native lifecycle action lost stable {identity}"
            )));
        }
    }
    let accepted = action["disposition"].get("Accepted").is_some();
    if accepted {
        if action["accepted_effects"] != 1
            || before == after
            || action["disposition"]["Accepted"]["operation_request_id"].is_null()
            || action["disposition"]["Accepted"]["operation_request_id"] != after["last_request_id"]
        {
            return Err(refusal(
                "accepted native action lacks one committed operation receipt",
            ));
        }
    } else if action["disposition"].get("Refused").is_none()
        || action["accepted_effects"] != 0
        || before != after
    {
        return Err(refusal(
            "refused native action changed owner or claimed an effect",
        ));
    }
    Ok(())
}

fn validate_arguments(
    action: &FaceAction,
    interaction: &FaceInteraction,
) -> Result<(), ConduitosError> {
    if action.arguments.len() != interaction.arguments.len() {
        return Err(refusal(
            "native interaction argument count differs from source contract",
        ));
    }
    for declaration in &action.arguments {
        let mut matches = interaction
            .arguments
            .iter()
            .filter(|argument| argument.name == declaration.name);
        let argument = matches
            .next()
            .ok_or_else(|| refusal("native interaction missing declared argument"))?;
        if matches.next().is_some()
            || declaration.contract.value_kind.as_str() != argument.value_kind
        {
            return Err(refusal(
                "native interaction duplicate argument or wrong value kind",
            ));
        }
        declaration
            .contract
            .validate(&argument.value)
            .map_err(|error| {
                refusal(format!(
                    "native interaction violates source value contract: {error:?}"
                ))
            })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_native_receipts_are_not_checkpoints() {
        let serial = concat!(
            "CONDUIT_NATIVE_ACTION {\"requested_intent\":\"conduit.intent/wake@1\",\"disposition\":{\"Refused\":{\"reason\":\"unavailable\"}}}\n",
            "CONDUIT_NATIVE_ACTION {\"requested_intent\":\"conduit.intent/wake@1\""
        );
        assert_eq!(refused_wake_count(serial).unwrap(), 1);
    }

    #[test]
    fn malformed_complete_native_receipts_fail_closed() {
        assert!(records("CONDUIT_NATIVE_FACE invalid\n", FACE_PREFIX).is_err());
        let serial = "CONDUIT_NATIVE_FACE {}\n".repeat(MAX_RECORDS + 1);
        assert!(records(&serial, FACE_PREFIX).is_err());
    }

    #[test]
    fn refused_owner_receipt_cannot_hide_mutation_or_an_accepted_effect() {
        let owner = json!({"body_id":"body/a", "born_sign_id":"born/a", "host_id":"host/a", "boot_id":"boot/a", "revision":7});
        let record = json!({"before":owner, "after":owner,
            "accepted_effects":0, "disposition":{"Refused":{"reason":"UnavailableAction"}}});
        validate_owner_effect(&record).unwrap();
        let mut changed = record.clone();
        changed["after"]["revision"] = json!(8);
        assert!(validate_owner_effect(&changed).is_err());
        let mut forged = record;
        forged["accepted_effects"] = json!(1);
        assert!(validate_owner_effect(&forged).is_err());
    }

    #[test]
    fn accepted_owner_receipt_requires_original_birth_and_terminal_operation() {
        let before = json!({"body_id":"body/a", "born_sign_id":"born/a", "host_id":"host/a", "boot_id":"boot/a", "revision":7});
        let mut after = before.clone();
        after["revision"] = json!(8);
        after["last_request_id"] = json!("request/8");
        let record = json!({"before":before,"after":after,"accepted_effects":1,
            "disposition":{"Accepted":{"operation_request_id":"request/8"}}});
        validate_owner_effect(&record).unwrap();
        let mut reborn = record.clone();
        reborn["after"]["born_sign_id"] = json!("born/another");
        assert!(validate_owner_effect(&reborn).is_err());
        let mut uncommitted = record;
        uncommitted["disposition"]["Accepted"]["operation_request_id"] = Value::Null;
        assert!(validate_owner_effect(&uncommitted).is_err());
    }

    #[test]
    fn recorded_arguments_obey_the_actual_source_value_contract() {
        let action = FaceAction {
            identity: "action/name".into(),
            intent: "intent/name".into(),
            target: "body/a".into(),
            name: "Name".into(),
            arguments: vec![conduit_face::FaceActionArgument::text(
                "name".into(),
                "Name".into(),
                1,
                4,
            )
            .unwrap()],
            disclosure: conduit_face::FaceDisclosureLevel::Primary,
            availability: FaceActionAvailability::Available,
        };
        // Receipt-shape fixture only; this test cannot admit an owner interaction.
        let mut interaction: FaceInteraction = serde_json::from_value(json!({
            "identity":"interaction/fixture", "face_id":"face/fixture", "face_revision":1,
            "show_id":"show/fixture", "action_id":"action/name", "target":"body/a", "sequence":1,
            "arguments":[{"name":"name", "value_kind":conduit_face::UTF8_TEXT_VALUE_KIND,"value":[65]}],
        })).unwrap();
        validate_arguments(&action, &interaction).unwrap();
        interaction.arguments[0].value = vec![65; 5];
        assert!(validate_arguments(&action, &interaction).is_err());
        interaction.arguments[0].value = vec![65];
        interaction.arguments[0].value_kind = "kind/wrong".into();
        assert!(validate_arguments(&action, &interaction).is_err());
        interaction.arguments[0].value_kind = conduit_face::UTF8_TEXT_VALUE_KIND.into();
        interaction.arguments.push(interaction.arguments[0].clone());
        assert!(validate_arguments(&action, &interaction).is_err());
    }

    #[test]
    fn a_future_show_cannot_retroactively_authorize_a_recorded_action() {
        let serial = concat!(
            "CONDUIT_NATIVE_ACTION {\"show_id\":\"show/later\"}\n",
            "CONDUIT_NATIVE_FACE {\"show\":{\"show_id\":\"show/later\"}}\n",
        );
        assert!(validate_receipt_order(serial).is_err());
    }
}

#[cfg(test)]
#[path = "receipt_chronology_tests.rs"]
mod receipt_chronology_tests;
