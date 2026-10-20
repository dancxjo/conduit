#![cfg(feature = "parser-model")]
//! Model-free supervised TRAIN replay; no decoder/scorer invocation or fit.
#[path = "common/causal_prefix_inputs.rs"]
mod causal_prefix_inputs;
#[path = "common/causal_prefix_parity.rs"]
mod causal_prefix_parity;
#[path = "common/causal_prefix_runtime.rs"]
mod causal_prefix_runtime;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/parser_selection_io.rs"]
mod parser_selection_io;
#[path = "common/parser_selection_values.rs"]
mod parser_selection_values;
use causal_prefix_inputs::*;
use causal_prefix_runtime::*;
use conduit_plot::rust_binding::NativeRustBinding;
use parser_selection_values::admit_state;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::PathBuf,
};
#[test]
#[ignore = "explicit pinned394TRAIN paired expectations and fresh local evidence directory; no training or inference"]
fn pinned_matched_causal_prefix_pairs_have_actual_source_parity_and_legal_targets() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}
fn run() {
    let input = PathBuf::from(
        std::env::var_os("CONDUIT_CAUSAL_PARITY_INPUT").expect("absolute paired input directory"),
    );
    let pins = PathBuf::from(
        std::env::var_os("CONDUIT_CAUSAL_PARITY_SOURCE_PINS")
            .expect("absolute frozen source-pins.json"),
    );
    let output = PathBuf::from(
        std::env::var_os("CONDUIT_CAUSAL_PARITY_OUTPUT").expect("fresh absolute output directory"),
    );
    assert!(
        std::env::var_os("CONDUIT_PARSER_SOURCE_TRACE").is_none(),
        "dedicated bounded canonical evidence replaces unrelated64MiB trace profile"
    );
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let pin_base = PathBuf::from(
        std::env::var_os("CONDUIT_CAUSAL_PARITY_PIN_BASE")
            .expect("absolute base for pinned campaign-relative source paths"),
    );
    let inputs = Inputs::read(&input, &pins, &repository, &pin_base);
    let mut store = Store::new(output);
    let profile = profile(&inputs.manifest["fixed_profile"]);
    store.write("admission-status.json",&json!({"status":"PREPARING_NOT_FIT_ELIGIBLE","fit_eligible":false,"original_sentences":394,"original_pairs":1620}));
    let quarantine = lexical_preflight(&mut store, &inputs.paired, &profile);
    let excluded: BTreeSet<_> = quarantine
        .iter()
        .map(|v| text(&v["sentence_id"]).to_owned())
        .collect();
    let excluded_pairs: Vec<_> = inputs
        .paired
        .iter()
        .filter(|v| excluded.contains(text(&v["sentence_id"])))
        .collect();
    let excluded_stops: Vec<_> = inputs
        .stops
        .iter()
        .filter(|v| excluded.contains(text(&v["sentence_id"])))
        .collect();
    store.write("quarantine.json",&json!({"actual_lexical_occurrence_mismatches":quarantine,"sentences":excluded.len(),"paired_targets":excluded_pairs.len(),"prefix_schedules":excluded_stops.len(),"both_arms_excluded_together":true,"scope":"diagnostic only; frozen394 cohort remains NOTFITELIGIBLE; no silent subset fitting"}));
    let mut graphs = Graphs::prepare();
    store.write("source-identities.json", &graphs.identities);
    store.write("input-identities.json", &inputs.hashes);
    let mut paired_file = store.stream("observed-pairs.jsonl");
    let mut stops_file = store.stream("observed-stops.jsonl");
    let mut by_sentence: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    let mut pair_ids = BTreeSet::new();
    for pair in &inputs.paired {
        if excluded.contains(text(&pair["sentence_id"])) {
            continue;
        }
        assert_eq!(pair["schema"], "causal-train-pair@1");
        assert_eq!(
            pair["admission_status"],
            "UNVALIDATED_REPLAY_REQUEST_NOT_FIT_ELIGIBLE"
        );
        assert!(pair_ids.insert(text(&pair["pair_id"]).to_owned()));
        by_sentence
            .entry(text(&pair["sentence_id"]).into())
            .or_default()
            .push(pair);
    }
    let mut stop_map = BTreeMap::new();
    for stop in &inputs.stops {
        if excluded.contains(text(&stop["sentence_id"])) {
            continue;
        }
        assert!(stop_map
            .insert(
                (
                    text(&stop["sentence_id"]).to_owned(),
                    u(&stop["prefix_count"]) as usize
                ),
                stop
            )
            .is_none());
    }
    let (mut invocation, mut pairs, mut transitions, mut prefixes, mut pauses, mut completed) =
        (0u64, 0, 0, 0, 0, 0);
    let mut action_counts: BTreeMap<u64, u64> = BTreeMap::new();
    let mut prefix_action_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut prefix_reason_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut reason_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut alternatives = BTreeMap::from([("causal", 0usize), ("full", 0usize)]);
    let mut resolved_pauses = 0;
    let diagnostic = std::env::var("CONDUIT_CAUSAL_PARITY_DIAGNOSTIC_SENTENCES")
        .ok()
        .map(|value| {
            let count = value.parse::<usize>().unwrap();
            assert!((1..=394).contains(&count));
            count
        });
    let mut processed_sentences = 0;
    let mut pause_records = Vec::new();
    let mut negatives = Vec::new();
    for (sentence_id, rows) in by_sentence {
        if diagnostic.is_some_and(|limit| processed_sentences >= limit) {
            break;
        }
        processed_sentences += 1;
        let sentence = rows[0];
        let n = u(&sentence["full_count"]) as usize;
        assert!((1..=4).contains(&n));
        assert_eq!(sentence["forms"].as_array().unwrap().len(), n);
        let gold: Vec<_> = sentence["gold_class_path"]
            .as_array()
            .unwrap()
            .iter()
            .map(u)
            .collect();
        assert!(gold.len() <= 12);
        let supervision = digest(
            &serde_json::to_vec(&json!([
                sentence["forms"],
                sentence["gold_pos"],
                sentence["gold_class_path"]
            ]))
            .unwrap(),
        );
        let mut prior = None;
        for p in 1..=n {
            prefixes += 1;
            let mut causal = arrival(
                &mut graphs,
                &mut store,
                sentence,
                p,
                false,
                prior.as_ref(),
                &profile,
            );
            let mut full = arrival(&mut graphs, &mut store, sentence, n, true, None, &profile);
            let stop = stop_map.remove(&(sentence_id.clone(), p)).unwrap();
            let path: Vec<_> = stop["replay_classes"]
                .as_array()
                .unwrap()
                .iter()
                .map(u)
                .collect();
            assert!(path.len() <= gold.len());
            assert_eq!(path, gold[..path.len()]);
            negatives.push(causal_prefix_parity::negative_queries(&causal, &full, p, n));
            let prefix_rows: Vec<_> = rows
                .iter()
                .copied()
                .filter(|row| u(&row["prefix_count"]) as usize == p)
                .collect();
            assert_eq!(prefix_rows.len(), path.len());
            for (j, &target) in path.iter().enumerate() {
                let request = prefix_rows
                    .iter()
                    .find(|row| row["replay_classes"].as_array().unwrap().len() == j)
                    .unwrap();
                assert_eq!(u(&request["target_class"]), target);
                assert_eq!(request["gold_class_path"], sentence["gold_class_path"]);
                assert_eq!(request["forms"], sentence["forms"]);
                assert_eq!(
                    request["replay_classes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(u)
                        .collect::<Vec<_>>(),
                    path[..j]
                );
                let prior_c = store.blob(&causal.state.clone().encode().unwrap());
                let prior_f = store.blob(&full.state.clone().encode().unwrap());
                let causal_features = causal_prefix_parity::arm(
                    &mut graphs,
                    &mut store,
                    &causal,
                    &request["arms"]["causal"],
                    &mut invocation,
                );
                let full_features = causal_prefix_parity::arm(
                    &mut graphs,
                    &mut store,
                    &full,
                    &request["arms"]["full"],
                    &mut invocation,
                );
                *alternatives.get_mut("causal").unwrap() +=
                    causal_features["alternatives"].as_array().unwrap().len();
                *alternatives.get_mut("full").unwrap() +=
                    full_features["alternatives"].as_array().unwrap().len();
                let (c, ce) = transition(&mut graphs, &mut store, &causal.state, target);
                let (f, fe) = transition(&mut graphs, &mut store, &full.state, target);
                if !*c.accepted() || !*f.accepted() {
                    store.write("unexpected-arrived-target-failure.json", &json!({"status":"UNEXPECTED_SOURCE_LEGALITY_FAILURE_NOT_FIT_ELIGIBLE","fit_eligible":false,"sentence_id":sentence_id,"prefix_count":p,"teacher_step":j,"target_class":target,"causal":ce,"full":fe,"policy":"fatal generator/fixture defect; never quarantine as representation mismatch"}));
                    panic!("arrived target rejected: {sentence_id} prefix{p} step{j}");
                }
                causal.state = admit_state(c.state().clone());
                full.state = admit_state(f.state().clone());
                assert_eq!(*causal.state.committed(), 0);
                assert_eq!(*full.state.committed(), 0);
                let event = json!({"status":"OBSERVED_NOT_GLOBALLY_VALIDATED","pair_id":request["pair_id"],"sentence_id":sentence_id,"prefix":p,"oracle_index":j,"target":target,"supervision":supervision,"causal":{"prior":prior_c,"lexical":store.blob(&causal.lexical.tape().clone().encode().unwrap()),"features":causal_features,"transition":ce},"full":{"prior":prior_f,"lexical":store.blob(&full.lexical.tape().clone().encode().unwrap()),"features":full_features,"transition":fe}});
                serde_json::to_writer(&mut paired_file, &event).unwrap();
                paired_file.write_all(b"\n").unwrap();
                pairs += 1;
                transitions += 2;
                *action_counts.entry(target).or_default() += 1;
                *prefix_action_counts
                    .entry(format!("prefix{p}/class{target}"))
                    .or_default() += 1;
                if pause_records.iter().any(|(sid, old_p, old_j, old_target)| {
                    sid == &sentence_id && *old_p < p && *old_j == j && *old_target == target
                }) {
                    resolved_pauses += 1;
                }
            }
            let reason = text(&stop["reason"]);
            *reason_counts.entry(reason.into()).or_default() += 1;
            *prefix_reason_counts
                .entry(format!("prefix{p}/{reason}"))
                .or_default() += 1;
            let refusal = if reason == "teacher_complete" {
                assert_eq!(path, gold);
                assert!(graphs.complete(&causal.state));
                completed += 1;
                json!(null)
            } else {
                assert!(*causal.state.unread() >= p as u64);
                let target = u(&stop["next_gold_class"]);
                assert_eq!(gold[path.len()], target);
                let (result, evidence) = transition(&mut graphs, &mut store, &causal.state, target);
                assert!(!*result.accepted(), "offline pause must actually refuse");
                for class in [0, 2, 39] {
                    let (rejected, _) = transition(&mut graphs, &mut store, &causal.state, class);
                    assert!(!*rejected.accepted());
                }
                pauses += 1;
                pause_records.push((sentence_id.clone(), p, path.len(), target));
                evidence
            };
            let event = json!({"sentence_id":sentence_id,"prefix":p,"reason":reason,"next_gold_class":stop["next_gold_class"],"accepted_replay_classes":path,"state":store.blob(&causal.state.clone().encode().unwrap()),"evidence":refusal,"emitted_pause_action_samples":0});
            serde_json::to_writer(&mut stops_file, &event).unwrap();
            stops_file.write_all(b"\n").unwrap();
            prior = Some(causal);
        }
    }
    if diagnostic.is_some() {
        store.write("validation-manifest.json",&json!({"status":"DIAGNOSTIC_PARTIAL_NOT_FIT_ELIGIBLE","fit_eligible":false,"original_sentences":394,"original_pairs":1620,"original_prefixes":650,"processed_sentences":processed_sentences,"observed_pairs":pairs,"feature_queries":invocation,"quarantined_sentences":excluded.len(),"quarantined_pairs":excluded_pairs.len(),"remaining_prefix_schedules":stop_map.len(),"inputs":inputs.hashes}));
        return;
    }
    assert!(stop_map.is_empty());
    assert_eq!(pairs + excluded_pairs.len(), 1620);
    assert_eq!(prefixes + excluded_stops.len(), 650);
    assert_eq!(completed + excluded.len(), 394);
    assert_eq!(
        pauses
            + excluded_stops
                .iter()
                .filter(|v| v["reason"] != "teacher_complete")
                .count(),
        256
    );
    assert!(resolved_pauses > 0);
    for (name, count) in &alternatives {
        assert_eq!(
            (*count
                + excluded_pairs
                    .iter()
                    .map(|v| v["arms"][name]["choice_vectors"].as_array().unwrap().len())
                    .sum::<usize>()) as u64,
            u(&inputs.manifest["alternative_query_counts"][name])
        );
    }
    for (reason, count) in &reason_counts {
        assert_eq!(
            *count
                + excluded_stops
                    .iter()
                    .filter(|v| text(&v["reason"]) == reason)
                    .count() as u64,
            u(&inputs.manifest["stop_counts"][reason])
        );
    }
    let word = inputs.manifest["fixed_profile"]
        .as_object()
        .unwrap()
        .keys()
        .find(|word| !word.is_empty() && word.chars().all(char::is_alphabetic))
        .unwrap();
    let unavailable = causal_prefix_parity::no_available_occurrence(&profile, word);
    store.write("negative-scope.json",&json!({"native_negative_counts":negatives,"incomplete_occurrence":unavailable,"unavailable_actions_source_refused":pauses*4,"future_pauses_with_later_accepted_replay":resolved_pauses,"note":"forged result indices are rejected by actual Source comparison, not Native result shape alone"}));
    store.write("validation-manifest.json",&json!({"status":if excluded.is_empty(){"ALL_SOURCE_PARITY_VALIDATED_MODEL_FREE_NOT_TRAINED"}else{"QUARANTINED_SOURCE_SEGMENTATION_NOT_FIT_ELIGIBLE"},"fit_eligible":excluded.is_empty(),"original_sentences":394,"original_pairs":1620,"quarantined_sentences":excluded.len(),"quarantined_pairs":excluded_pairs.len(),"source_validated_sentences":394-excluded.len(),"prefix_actions":prefix_action_counts,"prefix_pauses":prefix_reason_counts,"inputs":inputs.hashes,"pairs":pairs,"source_accepted_transitions":transitions,"source_feature_queries":invocation,"prefixes":prefixes,"actions":action_counts,"stops":reason_counts,"alternative_queries":alternatives,"canonical_blob_bytes":store.bytes,"canonical_blob_count":store.blobs,"multiplicity":inputs.manifest["multiplicity"],"schedule":"synthetic supervised TRAIN token arrivals, trailing spaces, partial Source revisions, explicit full observed stability; no natural timing or commitment","scope":"conditional teacher replay only; previously fitted TRAIN reused; no calibration, fit, inference, blind quality or installed streaming proof"}));
}
