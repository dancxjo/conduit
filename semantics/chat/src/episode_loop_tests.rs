use alloc::{string::ToString, vec, vec::Vec};

use conduit_ai::{
    llm_contract, LlmDeterminismProfile, ModelDerivedResult, ModelFailure, ModelResultDisposition,
    ModelResultProvenance, ModelWorkAccounting, LLM_GENERATE_FLOW_KIND,
};
use conduit_body::{Body, BodyConversationContext, BodyConversationContextBasis};
use conduit_core::{CheckedPlotId, KindSemanticLaw, PortTemporal, SignId, SourceDocumentId};
use conduit_plot::rust_binding::NativeRustBinding;

use crate::*;

fn context(wake_sequence: u64, name: &str) -> Vec<u8> {
    let body = Body::born(
        SourceDocumentId::from("source/episode"),
        CheckedPlotId::from("checked/episode"),
        1,
        SignId::from("sign/episode/born"),
    )
    .unwrap();
    let (_, wake) = body
        .wake(wake_sequence, SignId::from("sign/episode/wake"))
        .unwrap();
    encode_body_conversation_context(&BodyConversationContext {
        schema: "conduit.body/conversation-context-value@2".into(),
        display_name: name.into(),
        body_id: wake.body_id.clone(),
        wake_id: wake.wake_id.clone(),
        wake_sequence,
        basis: BodyConversationContextBasis {
            body_id: wake.body_id,
            wake_id: wake.wake_id,
            wake_sequence,
            revision: 1,
        },
        hosts: vec![],
        active_plots: vec![],
        current_plan_id: None,
        active_play_id: None,
        lines: vec![],
        recent_sign_ids: vec![],
    })
    .unwrap()
}

fn observation(identity: &str, content: &str, retain: bool) -> Vec<u8> {
    EpisodeObservation::new(content.into(), identity.into(), retain)
        .unwrap()
        .encode()
        .unwrap()
}

fn result(text: &str, disposition: ModelResultDisposition) -> Vec<u8> {
    let contract = llm_contract(LLM_GENERATE_FLOW_KIND).unwrap();
    let payload = text.as_bytes().to_vec();
    serde_json::to_vec(&ModelDerivedResult {
        provenance: ModelResultProvenance::ModelDerived,
        payload_kind: contract.result_payload_kind.as_str().to_string(),
        accounting: ModelWorkAccounting {
            input_bytes: 64,
            context_items: 1,
            output_bytes: payload.len() as u64,
            work_units: 1,
            history_items: 0,
        },
        payload,
        implementation_identity: "fixture/episode-model".into(),
        request_identity: "episode-request/fixture".into(),
        run_identity: "run/fixture".into(),
        confidence: None,
        disposition,
        determinism: LlmDeterminismProfile::DeterministicValidationFixture,
    })
    .unwrap()
}

/// Runs one episode through composer, retainer, and memory cell.
fn episode(
    composer: &mut EpisodeComposer,
    memory: &mut EpisodeMemoryCell,
    seen: &str,
    retain: bool,
    answer: &str,
) -> (serde_json::Value, EpisodeRecord) {
    composer
        .compose(&observation(seen, seen, retain), memory.current())
        .unwrap();
    let prompt: serde_json::Value = serde_json::from_slice(composer.request()).unwrap();
    let pending = composer.pending().to_vec();
    composer.commit();
    let mut retainer = EpisodeRetainer::new();
    let record = retainer
        .retain(&pending, &result(answer, ModelResultDisposition::Produced))
        .unwrap()
        .to_vec();
    memory.apply(&record).unwrap();
    memory.commit();
    (prompt, EpisodeRecord::decode(&record).unwrap())
}

#[test]
fn second_episode_recalls_only_the_selected_first_episode() {
    let mut composer = EpisodeComposer::new();
    let mut memory = EpisodeMemoryCell::new().unwrap();
    composer.admit_context(&context(2, "Pete")).unwrap();

    let (first_prompt, first) = episode(&mut composer, &mut memory, "door", true, "hello");
    assert_eq!(first_prompt["recalled"].as_array().unwrap().len(), 0);
    assert_eq!(*first.episode(), 1);
    assert!(first.recalled().get().is_empty());

    let (second_prompt, second) = episode(&mut composer, &mut memory, "lamp", false, "again");
    let recalled = second_prompt["recalled"].as_array().unwrap();
    assert_eq!(recalled.len(), 1);
    assert_eq!(recalled[0]["observation"]["origin"], "observation");
    assert_eq!(recalled[0]["observation"]["content"], "door");
    assert_eq!(recalled[0]["response"]["origin"], "model_derived");
    assert_eq!(recalled[0]["response"]["content"], "hello");
    assert_ne!(
        first_prompt["request_identity"],
        second_prompt["request_identity"]
    );
    assert_eq!(second.recalled().get().len(), 1);
    assert_eq!(second.body_id(), first.body_id());
    assert_eq!(second.wake_id(), first.wake_id());
    assert!(!*second.retained());

    // The unselected second episode advanced memory but was not retained.
    let state = EpisodeMemoryState::decode(memory.current()).unwrap();
    assert_eq!(*state.completed(), 2);
    assert_eq!(state.memories().get().len(), 1);
}

#[test]
fn memory_is_a_bounded_ring_and_rejects_out_of_order_records() {
    let mut composer = EpisodeComposer::new();
    let mut memory = EpisodeMemoryCell::new().unwrap();
    composer.admit_context(&context(1, "Pete")).unwrap();
    let mut pending = Vec::new();
    for index in 0..6 {
        let seen = alloc::format!("seen-{index}");
        composer
            .compose(&observation(&seen, &seen, true), memory.current())
            .unwrap();
        pending = composer.pending().to_vec();
        composer.commit();
        let record = EpisodeRetainer::new()
            .retain(&pending, &result("ok", ModelResultDisposition::Produced))
            .unwrap()
            .to_vec();
        memory.apply(&record).unwrap();
        memory.commit();
    }
    let state = EpisodeMemoryState::decode(memory.current()).unwrap();
    let kept: Vec<u64> = state
        .memories()
        .get()
        .iter()
        .map(|m| *m.episode())
        .collect();
    assert_eq!(kept, vec![3, 4, 5, 6]);

    let stale = EpisodeRetainer::new()
        .retain(&pending, &result("ok", ModelResultDisposition::Produced))
        .unwrap()
        .to_vec();
    assert_eq!(memory.apply(&stale), Err(EpisodeRefusal::MemoryOutOfOrder));
    // A memory Current that lags the begun episodes cannot start another one.
    let lagging = initial_episode_memory();
    assert_eq!(
        composer.compose(&observation("x", "x", true), &lagging),
        Err(EpisodeRefusal::MemoryOutOfOrder)
    );
}

#[test]
fn context_and_inputs_are_refused_truthfully() {
    let mut composer = EpisodeComposer::new();
    let memory = EpisodeMemoryCell::new().unwrap();
    assert_eq!(
        composer.compose(&observation("a", "a", true), memory.current()),
        Err(EpisodeRefusal::ContextMissing)
    );
    assert_eq!(
        composer.admit_context(b"{}"),
        Err(EpisodeRefusal::ContextMalformed)
    );
    composer.admit_context(&context(1, "Pete")).unwrap();
    composer.admit_context(&context(1, "Renamed")).unwrap();
    assert_eq!(
        composer.admit_context(&context(2, "Pete")),
        Err(EpisodeRefusal::ContextChangedIdentity)
    );
    assert_eq!(
        composer.compose(b"not a value", memory.current()),
        Err(EpisodeRefusal::ObservationMalformed)
    );
    assert_eq!(composer.begun(), 0);

    composer
        .compose(&observation("a", "a", true), memory.current())
        .unwrap();
    composer.cancel();
    composer.commit();
    assert_eq!(composer.begun(), 0);
    composer
        .compose(&observation("a", "a", true), memory.current())
        .unwrap();
    let pending = composer.pending().to_vec();
    let mut retainer = EpisodeRetainer::new();
    assert_eq!(
        retainer.retain(
            &pending,
            &result(
                "",
                ModelResultDisposition::Failed(ModelFailure::ImplementationFailure)
            )
        ),
        Err(EpisodeRefusal::ResponseNotProduced)
    );
    assert_eq!(
        retainer.retain(b"{}", &result("x", ModelResultDisposition::Produced)),
        Err(EpisodeRefusal::PendingMalformed)
    );
    let mut padded = result("x", ModelResultDisposition::Produced);
    padded.push(b' ');
    assert_eq!(
        retainer.retain(&pending, &padded),
        Err(EpisodeRefusal::ResponseMalformed)
    );
}

#[test]
fn prompt_drops_oldest_recall_to_stay_within_its_bound() {
    let mut composer = EpisodeComposer::new();
    let mut memory = EpisodeMemoryCell::new().unwrap();
    composer.admit_context(&context(1, "Pete")).unwrap();
    let wide = "w".repeat(250);
    let answer = "a".repeat(500);
    for _ in 0..4 {
        composer
            .compose(&observation("wide", &wide, true), memory.current())
            .unwrap();
        let pending = composer.pending().to_vec();
        composer.commit();
        let record = EpisodeRetainer::new()
            .retain(&pending, &result(&answer, ModelResultDisposition::Produced))
            .unwrap()
            .to_vec();
        memory.apply(&record).unwrap();
        memory.commit();
    }
    composer
        .compose(&observation("wide", &wide, true), memory.current())
        .unwrap();
    assert!(composer.request().len() <= MAXIMUM_EPISODE_PROMPT_BYTES);
    let pending = EpisodePending::decode(composer.pending()).unwrap();
    let recalled: Vec<u64> = pending
        .recalled()
        .get()
        .iter()
        .map(|m| *m.episode())
        .collect();
    assert!(recalled.len() < 4);
    assert_eq!(recalled.last(), Some(&4));
}

#[test]
fn kind_contracts_declare_the_memory_cell_as_the_only_initialized_delay() {
    let compose = episode_compose_semantic_contract();
    let retain = episode_retain_semantic_contract();
    let memory = episode_memory_semantic_contract();
    assert!(compose.semantic_laws.is_empty());
    assert!(retain.semantic_laws.is_empty());
    assert_eq!(
        retain.outputs[0].temporal,
        PortTemporal::Flow { closes: false }
    );
    let law = memory
        .semantic_laws
        .iter()
        .find_map(|law| match law {
            KindSemanticLaw::InitializedState(law) => Some(law),
            _ => None,
        })
        .unwrap();
    assert_eq!(law.update_port_id.as_str(), "update");
    assert_eq!(law.current_port_id.as_str(), "current");
    assert_eq!(law.initial, initial_episode_memory());
    let empty = EpisodeMemoryState::decode(&law.initial).unwrap();
    assert_eq!(*empty.completed(), 0);
    assert!(empty.memories().get().is_empty());
}

#[test]
fn retained_response_accepts_512_utf8_bytes_and_refuses_513_without_truncating() {
    let mut composer = EpisodeComposer::new();
    composer.admit_context(&context(2, "Pete")).unwrap();
    let memory = EpisodeMemoryCell::new().unwrap();
    composer
        .compose(
            &observation("obs/boundary", "A light.", true),
            memory.current(),
        )
        .unwrap();
    let pending = composer.pending().to_vec();
    let mut retainer = EpisodeRetainer::new();
    let fits = "é".repeat(256);
    let record = retainer
        .retain(&pending, &result(&fits, ModelResultDisposition::Produced))
        .unwrap();
    assert_eq!(
        EpisodeRecord::decode(record)
            .unwrap()
            .response()
            .unwrap()
            .content(),
        &fits
    );
    let too_long = alloc::format!("{fits}x");
    assert_eq!(
        retainer.retain(
            &pending,
            &result(&too_long, ModelResultDisposition::Produced)
        ),
        Err(EpisodeRefusal::ResponseBoundExceeded)
    );
}

#[test]
fn experience_recall_ranks_observed_terms_and_excludes_model_only_matches() {
    let mut composer = EpisodeComposer::new();
    let mut memory = EpisodeMemoryCell::new().unwrap();
    composer.admit_context(&context(2, "Pete")).unwrap();
    episode(
        &mut composer,
        &mut memory,
        "hallway switch label",
        true,
        "first",
    );
    episode(&mut composer, &mut memory, "switch relay", true, "second");
    episode(
        &mut composer,
        &mut memory,
        "garden temperature",
        true,
        "hallway switch label",
    );

    let trigger = EpisodeObservation::new(
        "What hallway switch label LABEL?".into(),
        "query/1".into(),
        false,
    )
    .unwrap();
    composer
        .compose_episode(trigger, memory.current(), Some(b"{}"))
        .unwrap();
    let request: serde_json::Value = serde_json::from_slice(composer.request()).unwrap();
    let recalled = request["recalled"].as_array().unwrap();
    assert_eq!(recalled.len(), 2);
    assert_eq!(recalled[0]["episode"], 2);
    assert_eq!(recalled[0]["shared_observation_terms"], 1);
    assert_eq!(recalled[1]["episode"], 1);
    assert_eq!(recalled[1]["shared_observation_terms"], 3);
    let pending = EpisodePending::decode(composer.pending()).unwrap();
    assert_eq!(
        pending
            .recalled()
            .get()
            .iter()
            .map(|item| *item.episode())
            .collect::<Vec<_>>(),
        vec![2, 1]
    );
}
