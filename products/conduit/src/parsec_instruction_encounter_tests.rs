//! Reconstruction of actual retained component Source frames outside Face.
//! The source capture exercised software input, terminal Show and supplement ACK.
//! Native frames, linear records and aural scripts below are reconstructions of
//! that captured occurrence, not live Shows, local input grants or voice delivery.
use super::parsec_cross_mask_counterexample_tests::{correlate, prepared_instruction_wording};
use conduit_core::{
    PlotIdentity, StructuredFieldValue, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_face::{
    checked_parsec_input_instruction, plan_face_utterances, render_linear_face, Face,
    FaceUtteranceProvenance, GraphicsClipClass, MaskShow, PreparedParsecInputInstruction,
    PreparedParsecInstructionFrameInspection,
};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduit_plot::{expand_canonical_plot_for_authoring, ProfileCatalog};
use serde_json::Value;
use sha2::{Digest, Sha256};

struct Validation {
    producer: PreparedParsecInputInstruction,
    inspector: PreparedParsecInstructionFrameInspection,
    checked_plot: PlotIdentity,
}
impl Validation {
    fn prepare() -> Self {
        let checked = checked_parsec_input_instruction().unwrap();
        let expanded = expand_canonical_plot_for_authoring(
            &checked,
            "parsec/input-instruction",
            &ProfileCatalog::new(),
        )
        .unwrap()
        .expanded;
        Self {
            producer: prepared_instruction_wording(),
            inspector: PreparedParsecInstructionFrameInspection::prepare().unwrap(),
            checked_plot: PlotIdentity {
                source_document_id: expanded.source_document_id,
                checked_plot_id: expanded.checked_plot_id,
                expanded_plot_id: expanded.expanded_plot_id,
            },
        }
    }
    fn occurrence(&mut self, exchange: &Value, index: usize) -> Result<Encounter, String> {
        let (face, expected) = correlate(exchange, index, &mut self.producer)?;
        let observation = &exchange["instructions"][index];
        let effect = &observation["effect"];
        let canonical: Vec<u8> = serde_json::from_value(effect["canonical_instruction"].clone())
            .map_err(|e| format!("missing/malformed actual Source frame: {e:?}"))?;
        if canonical != expected {
            return Err("retained frame differs from exact checked instruction output".into());
        }
        let inspected = self
            .inspector
            .inspect(&canonical)
            .map_err(|e| format!("component frame: {e:?}"))?;
        if inspected.withdrawal() || inspected.canonical_bytes() != canonical {
            return Err(
                "positive captured occurrence has withdrawn or changed component data".into(),
            );
        }
        let action = inspected
            .binding()
            .record_field("offered_action")
            .map_err(|e| format!("{e:?}"))?
            .ok_or("missing action owner")?
            .record_field("action_identity")
            .map_err(|e| format!("{e:?}"))?
            .ok_or("missing component action")?
            .primitive_bytes(conduit_core::TEXT_INFO_ID)
            .map_err(|e| format!("{e:?}"))?;
        if action != face.actions[0].identity.as_bytes() {
            return Err("component instruction rewrites the original Face action".into());
        }
        let wording = inspected.wording().to_owned();
        let plot: PlotIdentity =
            serde_json::from_value(effect["instruction_producer_plot"].clone())
                .map_err(|e| format!("{e:?}"))?;
        if plot != self.checked_plot {
            return Err("instruction producer differs from current checked source".into());
        }
        let plan = effect["instruction_producer_plan"]
            .as_str()
            .ok_or("missing producer Plan")?;
        if plan.len() != 64
            || !plan
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || effect["instruction_producer_plan"]
                != exchange["instructions"][0]["effect"]["instruction_producer_plan"]
        {
            return Err(
                "documentary instruction Plan identity is malformed or changes within this scope"
                    .into(),
            );
        }
        // The selected producer Plot is Host-independent and compared exactly.
        // Its recorded Plan is NOT reproduced on the computational validation Host.
        let show: MaskShow =
            serde_json::from_value(observation["show"].clone()).map_err(|e| format!("{e:?}"))?;
        let [fragment] = show.planned_mask.plan.fragments.as_slice() else {
            return Err("captured Mask is not one local fragment".into());
        };
        let host = &observation["host"];
        let host_matches = |host_id: &str, boot_id: &str, generation: u64| {
            host["host_id"] == host_id
                && host["boot_id"] == boot_id
                && host["offer_generation"] == generation
        };
        if !host_matches(
            fragment.host_id.as_str(),
            fragment.boot_id.as_str(),
            fragment.offer_generation.0,
        ) || fragment.placements.iter().any(|placement| {
            !host_matches(
                placement.host_id.as_str(),
                placement.boot_id.as_str(),
                placement.offer_generation.0,
            )
        }) {
            return Err("Mask fragment/placements differ from complete recorded Host scope".into());
        }
        for plot in exchange["body_plan"]["plots"]
            .as_array()
            .ok_or("missing Body plots")?
        {
            for part in plot["plan"]["fragments"]
                .as_array()
                .ok_or("missing Body fragments")?
            {
                if part["host_id"] != host["host_id"]
                    || part["boot_id"] != host["boot_id"]
                    || part["offer_generation"] != host["offer_generation"]
                {
                    return Err("input Body fragment differs from recorded Host scope".into());
                }
            }
        }
        let lowered = lower_plan_fragment(fragment).map_err(|e| format!("{e:?}"))?;
        if fragment.placements.len() != 5
            || lowered.cords.len() != 8
            || lowered.host_calls.len() != 4
        {
            return Err("capture lacks the admitted internal Source/effect profile".into());
        }
        let operation = |contract: &str| {
            let mut operations = lowered
                .host_calls
                .iter()
                .filter(|operation| operation.contract_id.as_str() == contract);
            let first = operations.next().ok_or("missing instruction operation")?;
            if operations.next().is_some() {
                return Err("duplicate instruction operation");
            }
            Ok(first)
        };
        let source = operation("conduit.host/parsec-current-input-instruction@1")?;
        let supplement = operation("conduit.host/parsec-input-instruction-effect@1")?;
        if effect["source_node"] != source.node.0
            || effect["source_call"] != source.call.0
            || effect["effect_node"] != supplement.node.0
            || effect["effect_call"] != supplement.call.0
            || effect["source_request"] != 0
            || effect["effect_request"] != 0
            || source.maximum_in_flight != 1
            || supplement.maximum_in_flight != 1
            || source.binding.maximum_output_bytes != 4096
            || supplement.binding.maximum_input_bytes != 4096
            || supplement.binding.maximum_output_bytes != 0
            || effect["value_bytes"] != canonical.len()
            || !lowered.cords.iter().any(|cord| {
                matches!(cord.spec.source, conduit_kernel::CordEndpoint::Local { node, .. } if node == source.node)
                    && matches!(cord.spec.sink, conduit_kernel::CordEndpoint::Local { node, .. } if node == supplement.node)
            })
        {
            return Err("recorded Source/effect facts differ from exact lowered Mask route".into());
        }
        let slot = effect["value_slot"].as_u64().ok_or("missing value slot")?;
        let generation = effect["value_generation"]
            .as_u64()
            .ok_or("missing value generation")?;
        // The actual profile prepares C+H store items. These are recorded bounds,
        // not proof that a copied generation exists in a current live store.
        if slot >= u64::from(lowered.cord_value_slots) + lowered.host_calls.len() as u64
            || generation == 0
            || generation > u64::from(u16::MAX)
        {
            return Err("recorded ValueRef exceeds exact prepared value-store bounds".into());
        }
        Ok(Encounter {
            face,
            wording,
            provenance: Provenance {
                body_play: observation["body_play"].clone(),
                host: host.clone(),
                source_show: show.show_id.as_str().into(),
                action: core::str::from_utf8(action)
                    .map_err(|e| format!("{e:?}"))?
                    .into(),
                mapping_revision: observation["binding"]["mapping_revision"]
                    .as_u64()
                    .ok_or("missing map revision")?,
                producer_plot: plot,
                producer_plan: plan.into(),
                frame_sha256: Sha256::digest(&canonical).into(),
            },
        })
    }
}

#[derive(serde::Serialize)]
struct Provenance {
    body_play: Value,
    host: Value,
    source_show: String,
    action: String,
    mapping_revision: u64,
    producer_plot: PlotIdentity,
    producer_plan: String,
    frame_sha256: [u8; 32],
}
struct Encounter {
    face: Face,
    wording: String,
    provenance: Provenance,
}
/// An ordinary component record accompanying the unchanged Face document.
/// This is deliberately not a Face utterance clause or a serialized runtime lease.
#[derive(serde::Serialize)]
struct ComponentReading<'a> {
    wording: &'a str,
    provenance: &'a Provenance,
}
fn reconstruct(encounter: &Encounter) {
    let face = &encounter.face;
    let action = &face.actions[0];
    assert_eq!(encounter.provenance.action, action.identity);
    assert_eq!(
        encounter.provenance.body_play["active_play_id"],
        face.basis.active_play_id.as_ref().unwrap().as_str()
    );
    assert!(!encounter.provenance.source_show.is_empty());
    assert!(!encounter.provenance.host["boot_id"]
        .as_str()
        .unwrap()
        .is_empty());
    assert!(encounter.provenance.mapping_revision > 0);
    assert_eq!(encounter.provenance.producer_plan.len(), 64);
    assert!(!encounter
        .provenance
        .producer_plot
        .checked_plot_id
        .as_str()
        .is_empty());
    assert_ne!(encounter.provenance.frame_sha256, [0; 32]);
    let mut native =
        conduitos::native_face_scene::NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    // This bounded existing furniture accepts the captured specimen. Longer or
    // non-ASCII custom labels must refuse honestly; no truncation or guessed key.
    native.set_local_notice(&encounter.wording).unwrap();
    assert_eq!(native.face(), face);
    let mut hit = false;
    for _ in 0..native.pages() {
        let frame = native.frame().unwrap();
        assert_eq!(frame.face_id, &face.identity);
        assert_eq!(frame.revision, face.revision);
        assert!(frame
            .scene
            .commands()
            .iter()
            .any(|command| command.payload() == encounter.wording));
        assert!(frame
            .scene
            .commands()
            .iter()
            .all(|command| command.clip_class() == GraphicsClipClass::FullyVisible));
        for target in &frame.hits {
            if target.action == action {
                assert_eq!(
                    native
                        .resolve(&face.identity, face.revision, target.control)
                        .unwrap()
                        .0,
                    action
                );
                hit = true;
            }
        }
        native.turn_page(true);
    }
    assert!(
        hit,
        "painted semantic action has an exact native control mapping"
    );
    let linear = render_linear_face(face).unwrap();
    assert_eq!(linear.face_id, face.identity);
    assert_eq!(linear.revision, face.revision);
    assert!(linear.lines.iter().any(|line| line.contains(&action.name)));
    let linear_component = ComponentReading {
        wording: &encounter.wording,
        provenance: &encounter.provenance,
    };
    let aural = plan_face_utterances(face).unwrap();
    assert_eq!(aural.source_face_identity, face.identity.as_str());
    assert_eq!(aural.source_face_revision, face.revision);
    assert!(aural
        .clauses
        .iter()
        .any(|clause| matches!(&clause.provenance,
        FaceUtteranceProvenance::Action(source) if source.identity() == &action.identity)));
    let aural_component = ComponentReading {
        wording: &encounter.wording,
        provenance: &encounter.provenance,
    };
    // A separate deterministic-linear component record carries the exact source
    // words plus their provenance; it does not append invented Face facts.
    let linear_record = serde_json::to_vec(&linear_component).unwrap();
    let linear_record: Value = serde_json::from_slice(&linear_record).unwrap();
    assert_eq!(linear_record["wording"], encounter.wording);
    assert_eq!(linear_record["provenance"]["action"], action.identity);
    // A separate aural component segment accompanies every unchanged Face
    // clause. This is a bounded reading script, not synthesized/delivered audio.
    assert!(aural.clauses.len() < conduit_face::MAX_FACE_UTTERANCE_CLAUSES);
    assert!(
        aural.encoded_bytes + aural_component.wording.len()
            < conduit_face::MAX_FACE_UTTERANCE_PLAN_BYTES
    );
    let script = aural
        .clauses
        .iter()
        .map(|clause| clause.text.as_str())
        .chain(core::iter::once(aural_component.wording))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(script.ends_with(aural_component.wording));
    assert_eq!(linear_component.wording, aural_component.wording);
    assert_eq!(linear_component.provenance.action, action.identity);
    assert_eq!(
        aural_component.provenance.source_show,
        encounter.provenance.source_show
    );
    // Withdrawal clears component furniture, without rewriting the historical Face.
    assert!(native.clear_local_notice());
    assert!(!native
        .frame()
        .unwrap()
        .scene
        .commands()
        .iter()
        .any(|command| command.payload() == encounter.wording));
    assert_eq!(native.face(), face);
}

fn replace_leaf(value: &StructuredInfoValue, path: &[&str], bytes: &[u8]) -> StructuredInfoValue {
    if path.is_empty() {
        return StructuredInfoValue::leaf(value.value_type().clone(), bytes.to_vec()).unwrap();
    }
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("native record path");
    };
    StructuredInfoValue::record(
        value.value_type().clone(),
        fields
            .iter()
            .map(|field| {
                StructuredFieldValue::new(
                    field.name(),
                    if field.name() == path[0] {
                        replace_leaf(field.value(), &path[1..], bytes)
                    } else {
                        field.value().clone()
                    },
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

#[test]
#[ignore = "requires actual Source-frame artifact in CONDUIT_CROSS_MASK_PARSEC_ARTIFACT; reconstruction only"]
fn actual_parsec_component_frames_survive_native_linear_and_aural_reconstruction() {
    let path = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_CROSS_MASK_PARSEC_ARTIFACT")
            .expect("explicit actual Source-frame artifact required; no fixture fallback"),
    );
    assert!(path.is_absolute());
    let exchange: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(
        exchange["schema"],
        "conduit.proof/parsec-input-mask-exchange@1"
    );
    assert_eq!(exchange["final_prompt_withdrawn"], true);
    assert_eq!(exchange["instructions"].as_array().unwrap().len(), 4);
    let mut validation = Validation::prepare();
    let mut browser_records = Vec::new();
    for index in 0..4 {
        let encounter = validation.occurrence(&exchange, index).unwrap();
        reconstruct(&encounter);
        browser_records.push(serde_json::json!({
            "wording": encounter.wording,
            "provenance": encounter.provenance,
            "face_revision_decimal": encounter.face.revision.to_string(),
            "mapping_revision_decimal": encounter.provenance.mapping_revision.to_string(),
            "face": encounter.face,
        }));
    }
    // Every corrupted record is rejected before invoking any renderer. These
    // refusals establish evidence consistency, not unforgeable live custody.
    let mut refused = Vec::new();
    let mut changed = exchange.clone();
    changed["instructions"][3]["effect"] = Value::Null;
    refused.push(changed);
    let mut changed = exchange.clone();
    changed["instructions"][3]["binding"] = exchange["instructions"][2]["binding"].clone();
    refused.push(changed);
    let mut changed = exchange.clone();
    changed["instructions"][3]["binding"] = Value::Null;
    refused.push(changed);
    let mut changed = exchange.clone();
    changed["instructions"][3]["effect"]["canonical_instruction"] =
        exchange["instructions"][2]["effect"]["canonical_instruction"].clone();
    changed["instructions"][3]["effect"]["value_bytes"] =
        exchange["instructions"][2]["effect"]["value_bytes"].clone();
    refused.push(changed);
    for (key, value) in [
        ("value_bytes", 0),
        ("value_slot", 65_535),
        ("value_generation", 0),
        ("source_node", 65_535),
        ("effect_call", 65_535),
    ] {
        let mut changed = exchange.clone();
        changed["instructions"][3]["effect"][key] = value.into();
        refused.push(changed);
    }
    let frame: Vec<u8> = serde_json::from_value(
        exchange["instructions"][3]["effect"]["canonical_instruction"].clone(),
    )
    .unwrap();
    let value = StructuredInfoValue::from_canonical_bytes(&frame).unwrap();
    let rewritten = replace_leaf(
        &value,
        &["binding", "offered_action", "action_identity"],
        b"game/start",
    )
    .canonical_bytes()
    .unwrap();
    let mut changed = exchange.clone();
    changed["instructions"][3]["effect"]["value_bytes"] = rewritten.len().into();
    changed["instructions"][3]["effect"]["canonical_instruction"] =
        serde_json::to_value(rewritten).unwrap();
    refused.push(changed); // Look cannot rewrite a controller action.
    let forged = replace_leaf(&value, &["b"], b"Space")
        .canonical_bytes()
        .unwrap();
    let mut changed = exchange.clone();
    changed["instructions"][3]["effect"]["value_bytes"] = forged.len().into();
    changed["instructions"][3]["effect"]["canonical_instruction"] =
        serde_json::to_value(forged).unwrap();
    let invented = "\r\u{1b}[2KInput: Space to Shoot.";
    changed["instructions"][3]["prompt_text"] = invented.into();
    changed["instructions"][3]["prompt_text_sha256"] =
        serde_json::to_value(Sha256::digest(invented.as_bytes()).to_vec()).unwrap();
    refused.push(changed); // A recomputed text digest cannot establish authored wording.
    for changed in refused {
        assert!(validation.occurrence(&changed, 3).is_err());
    }
    // Optional private bridge to a real DOM reconstruction. Export only after
    // all positive and negative checks pass; these records carry no authority.
    if let Some(path) = std::env::var_os("CONDUIT_PARSEC_BROWSER_RECORDS") {
        let path = std::path::PathBuf::from(path);
        assert!(path.is_absolute());
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        serde_json::to_writer(
            file,
            &serde_json::json!({
                "schema": "conduit.proof/parsec-component-reconstruction@1",
                "records": browser_records,
            }),
        )
        .unwrap();
    }
}
