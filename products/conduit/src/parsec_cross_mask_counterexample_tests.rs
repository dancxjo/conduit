//! Artifact-driven architecture diagnostic, not binding admission or Mask acceptance.
//! The producer exercised an actual software-input/terminal Body. These consumers
//! reconstruct its Faces; they do not prove scanout, delivered speech or browser Show.
use conduit_face::{
    plan_face_utterances, render_linear_face, ActionInputBindingProposalInfo, Face,
    FaceUtteranceProvenance, GraphicsClipClass, GraphicsTextRole, MaskShow,
    ParsecInputInstructionLook, ParsecMapTransitionResult, PreparedParsecInputInstruction,
};
use conduit_plot::rust_binding::NativeRustBinding;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Independent computational validation on an explicitly selected software Host.
/// This fixture never supplies current input, Show custody or a runtime grant.
pub(super) fn prepared_instruction_wording() -> PreparedParsecInputInstruction {
    use conduit_std_host::flow_activation::{
        required_parsec_input_instruction_offer, ParsecInputInstructionSourcePlan,
    };
    let baseline = conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
        host_id: "proof/parsec-artifact-wording".into(),
        boot_id: "proof/parsec-artifact-wording/boot".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let mut advertisement = baseline.advertisement().clone();
    let offer = required_parsec_input_instruction_offer().unwrap();
    if let Some(existing) = advertisement
        .capabilities
        .iter()
        .find(|existing| existing.capability_id == offer.capability_id)
    {
        assert_eq!(existing, &offer);
    } else {
        advertisement.capabilities.push(offer);
    }
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    // Exact source offer is selected before constructing this computational Host.
    let host = conduit_std_host::StdHost::from_advertisement(advertisement).unwrap();
    ParsecInputInstructionSourcePlan::prepare(host.advertisement())
        .unwrap()
        .evaluator()
        .unwrap()
}

pub(super) fn correlate(
    exchange: &Value,
    index: usize,
    instruction: &mut PreparedParsecInputInstruction,
) -> Result<(Face, Vec<u8>), String> {
    let observation = &exchange["instructions"][index];
    let face: Face = serde_json::from_value(observation["face"].clone())
        .map_err(|error| format!("{error:?}"))?;
    face.validate().map_err(|error| format!("{error:?}"))?;
    let show: MaskShow = serde_json::from_value(observation["show"].clone())
        .map_err(|error| format!("{error:?}"))?;
    show.validate(&face).map_err(|error| format!("{error:?}"))?;
    let binding = &observation["binding"];
    let equal = |left: &Value, right: &Value| !left.is_null() && left == right;
    let play = &exchange["issued_body_play"];
    if !equal(&observation["body_play"], play)
        || !equal(
            &face_json(&face)["basis"]["active_play_id"],
            &play["active_play_id"],
        )
        || !equal(&play["plan_id"], &exchange["body_plan"]["plan_id"])
        || !equal(&play["body_id"], &exchange["body_plan"]["body_id"])
        || !equal(&binding["face_identity"], &observation["face"]["identity"])
        || !equal(&binding["show_identity"], &observation["show"]["show_id"])
        || !equal(&binding["host_id"], &observation["host"]["host_id"])
        || !equal(&binding["boot_id"], &observation["host"]["boot_id"])
        || !equal(&binding["host_id"], &observation["show"]["show"]["host_id"])
        || !equal(&binding["boot_id"], &observation["show"]["show"]["boot_id"])
        || !equal(
            &observation["host"]["offer_generation"],
            &observation["show"]["show"]["offer_generation"],
        )
        || !equal(
            &binding["offered_action"]["source_face_revision"],
            &observation["face"]["revision"],
        )
        || !equal(
            &binding["input_plan_id"],
            &exchange["body_plan"]["plots"][0]["plan"]["plan_id"],
        )
        || binding["available"] != true
        || !equal(
            &exchange["show_occurrences"][index][0],
            &observation["face"],
        )
        || !equal(
            &exchange["show_occurrences"][index][1],
            &observation["show"],
        )
    {
        return Err("Face/Show/binding/Host/Body documentary correlation mismatch".into());
    }
    let action = binding["offered_action"]["action_identity"]
        .as_str()
        .ok_or("missing action")?;
    if face.actions.len() != 1 || face.actions[0].identity != action {
        return Err("source action differs from retained Face".into());
    }
    let state = if index == 0 {
        let bytes: Vec<u8> = serde_json::from_value(
            exchange["body_plan"]["plots"][0]["plan"]["activations"][0]["Scan"]
                ["initial_accumulator"]
                .clone(),
        )
        .map_err(|error| format!("{error:?}"))?;
        let initial = ParsecMapTransitionResult::from_structured(
            conduit_core::StructuredInfoValue::from_canonical_bytes(&bytes)
                .map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("{error:?}"))?;
        if *initial.state().started() || face.actions[0].identity != "game/start" {
            return Err("genesis/action correlation mismatch".into());
        }
        initial.state().clone()
    } else {
        let result = &exchange["source_results"][index - 1];
        let bytes: Vec<u8> = serde_json::from_value(result["canonical_result_bytes"].clone())
            .map_err(|error| format!("{error:?}"))?;
        let decoded = ParsecMapTransitionResult::from_structured(
            conduit_core::StructuredInfoValue::from_canonical_bytes(&bytes)
                .map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("{error:?}"))?;
        if result["state"]["mapping_revision"] != *decoded.state().mapping_revision()
            || result["state"]["keyboard"] != *decoded.state().keyboard()
            || result["state"]["started"] != *decoded.state().started()
            || result["accepted"] != *decoded.accepted()
            || !decoded.accepted()
        {
            return Err("canonical source result disagrees with documentary state".into());
        }
        decoded.state().clone()
    };
    let revision = *state.mapping_revision();
    let keyboard = *state.keyboard();
    let label = if keyboard { "Enter" } else { "Space" };
    let text = observation["prompt_text"]
        .as_str()
        .ok_or("missing flushed prompt")?;
    let digest: Vec<u8> = serde_json::from_value(observation["prompt_text_sha256"].clone())
        .map_err(|error| format!("{error:?}"))?;
    let canonical_state = state.encode().map_err(|error| format!("{error:?}"))?;
    let proposal: ActionInputBindingProposalInfo =
        serde_json::from_value(binding.clone()).map_err(|error| format!("{error:?}"))?;
    let canonical_binding = proposal.encode().map_err(|error| format!("{error:?}"))?;
    let candidate = instruction
        .evaluate(
            &canonical_state,
            &canonical_binding,
            ParsecInputInstructionLook::Linear,
            false,
        )
        .map_err(|error| format!("checked instruction wording: {error:?}"))?;
    // Re-evaluation confirms exact authored wording only. Captured IDs and this
    // candidate are documentary metadata, never Source/effect runtime custody.
    let expected_text = format!("\r\u{1b}[2K{}", candidate.wording());
    if candidate.withdrawal()
        || candidate.binding_bytes() != canonical_binding
        || binding["mapping_revision"] != revision
        || binding["control_identity"]
            != if keyboard {
                "keyboard/enter"
            } else {
                "control/fire"
            }
        || binding["control_label"] != label
        || text != expected_text
        || digest != Sha256::digest(text.as_bytes()).to_vec()
        || observation["input_view_epoch"] != observation["show"]["show"]["play_sequence"]
        || observation["producer_plan_id"]
            .as_str()
            .is_none_or(str::is_empty)
        || observation["producer_plot"]["checked_plot_id"]
            .as_str()
            .is_none_or(str::is_empty)
        || binding["input_plan_id"].as_str().is_none_or(str::is_empty)
    {
        return Err("current source-map/prompt documentary correlation mismatch".into());
    }
    Ok((face, candidate.canonical_bytes().to_vec()))
}

fn face_json(face: &Face) -> Value {
    serde_json::to_value(face).unwrap()
}

fn reconstruct_without_instruction(face: &Face) {
    let action = &face.actions[0];
    let linear = render_linear_face(face).unwrap();
    assert_eq!(linear.face_id, face.identity);
    assert_eq!(linear.revision, face.revision);
    let lines = linear.lines.join("\n");
    assert!(lines.contains(&action.name));
    let aural = plan_face_utterances(face).unwrap();
    assert_eq!(aural.source_face_identity, face.identity.as_str());
    assert_eq!(aural.source_face_revision, face.revision);
    assert!(aural
        .clauses
        .iter()
        .any(|clause| matches!(&clause.provenance,
        FaceUtteranceProvenance::Action(source) if source.identity() == &action.identity)));
    let spoken = aural
        .clauses
        .iter()
        .map(|clause| clause.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let mut native =
        conduitos::native_face_scene::NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    assert_eq!(native.face(), face);
    let mut painted = String::new();
    let mut native_actions = String::new();
    let mut hit = false;
    for details in [false, true] {
        native.show_details(details);
        for _ in 0..native.pages() {
            let frame = native.frame().unwrap();
            assert_eq!(frame.face_id, &face.identity);
            assert_eq!(frame.revision, face.revision);
            hit |= frame.hits.iter().any(|hit| hit.action == action);
            for command in frame.scene.commands() {
                assert_eq!(command.clip_class(), GraphicsClipClass::FullyVisible);
                painted.push_str(command.payload());
                painted.push('\n');
                if command.text_role() == GraphicsTextRole::Action {
                    native_actions.push_str(command.payload());
                    native_actions.push('\n');
                }
            }
            native.turn_page(true);
        }
    }
    assert!(hit, "native activation retains the exact semantic action");
    assert!(painted.contains(&action.name));
    // Generic native navigation describes its own Enter activation. It does
    // not reveal the captured source policy's Space/Enter-to-game mapping.
    assert!(painted.contains("Enter act"));
    assert!(native_actions.contains(&action.name));
    for (medium, document) in [
        ("native action labels", &native_actions),
        ("linear", &lines),
        ("aural", &spoken),
    ] {
        for key in ["Space", "Enter"] {
            assert!(
                !document.contains(key),
                "{medium} reconstruction contains {key}: {document}"
            );
        }
    }
    // This demonstrates absent information, not a correct alternate binding. Aural
    // action provenance is retained, but this script cannot communicate the key.
}

#[test]
#[ignore = "requires actual Body exchange in CONDUIT_CROSS_MASK_PARSEC_ARTIFACT; diagnostic only"]
fn actual_parsec_instruction_is_lost_at_face_only_mask_boundary() {
    let path = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_CROSS_MASK_PARSEC_ARTIFACT")
            .expect("explicit actual artifact required; no fixture fallback"),
    );
    assert!(path.is_absolute());
    let exchange: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(
        exchange["schema"],
        "conduit.proof/parsec-input-mask-exchange@1"
    );
    assert_eq!(exchange["final_prompt_withdrawn"], true);
    for key in [
        "instructions",
        "show_occurrences",
        "source_results",
        "scan_child_signs",
    ] {
        assert_eq!(exchange[key].as_array().unwrap().len(), 4);
    }
    // This consumer explicitly targets source-authored instruction captures;
    // legacy Rust-formatted Press captures remain separate historical evidence.
    let mut instruction = prepared_instruction_wording();
    for index in 0..4 {
        let receipt = &exchange["scan_child_signs"][index];
        assert_eq!(
            receipt["parent_active_play_id"],
            exchange["issued_body_play"]["active_play_id"]
        );
        assert_eq!(receipt["invocation"], index);
        assert!(receipt["sign_event_count"].as_u64().unwrap() > 0);
        assert_eq!(exchange["source_results"][index]["fore_sequence"], index);
        let (face, _) = correlate(&exchange, index, &mut instruction).unwrap();
        reconstruct_without_instruction(&face);
    }
    assert_eq!(
        exchange["instructions"][0]["binding"]["offered_action"]["action_identity"],
        "game/start"
    );
    for index in 1..4 {
        assert_eq!(
            exchange["instructions"][index]["binding"]["offered_action"]["action_identity"],
            "game/shoot"
        );
    }
    assert_eq!(
        exchange["instructions"][2]["binding"]["control_label"],
        "Space"
    );
    assert_eq!(
        exchange["instructions"][3]["binding"]["control_label"],
        "Enter"
    );
    // Mutate the actual captured record: these are evidence consistency refusals,
    // never standalone fabricated runtime authority or scheduler admission tests.
    let mut changed = exchange.clone();
    changed["instructions"][3]["binding"] = exchange["instructions"][2]["binding"].clone();
    assert!(
        correlate(&changed, 3, &mut instruction).is_err(),
        "stale/substituted binding"
    );
    changed["instructions"][3]["binding"] = Value::Null;
    assert!(
        correlate(&changed, 3, &mut instruction).is_err(),
        "missing binding"
    );
    changed = exchange.clone();
    changed["instructions"][3]["binding"]["offered_action"]["action_identity"] =
        "game/start".into();
    assert!(
        correlate(&changed, 3, &mut instruction).is_err(),
        "Look-style action rewrite"
    );
    changed = exchange.clone();
    let invented = format!(
        "{} Invented activation instruction.",
        changed["instructions"][3]["prompt_text"].as_str().unwrap()
    );
    changed["instructions"][3]["prompt_text"] = invented.clone().into();
    changed["instructions"][3]["prompt_text_sha256"] =
        serde_json::to_value(Sha256::digest(invented.as_bytes()).to_vec()).unwrap();
    assert!(
        correlate(&changed, 3, &mut instruction).is_err(),
        "rehashed non-source wording"
    );
}
