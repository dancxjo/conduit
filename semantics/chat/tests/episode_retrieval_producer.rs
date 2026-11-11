#![cfg(feature = "plot-catalog")]
use conduit_chat::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
mod common_episode_retrieval;
use common_episode_retrieval::*;
fn context() -> Vec<u8> {
    let context = serde_json::json!({"schema":"conduit.body/conversation-context-value@2","display_name":"Pete","body_id":"body/one","wake_id":"wake/one","wake_sequence":3,"basis":{"body_id":"body/one","wake_id":"wake/one","wake_sequence":3,"revision":1},"hosts":[],"active_plots":[],"current_plan_id":null,"active_play_id":null,"lines":[],"recent_sign_ids":[]});
    // The ordinary encoder supplies canonical field order and validates identity.
    let context: conduit_body::BodyConversationContext = serde_json::from_value(context).unwrap();
    encode_body_conversation_context(&context).unwrap()
}
#[test]
fn producer_preserves_packets_reference_and_explicit_clock_without_allocating() {
    let f = Fixture::new();
    let mut producer = PreparedEpisodeRetrievalProducer::prepare();
    producer.admit_context(&context()).unwrap();
    let clock = EpisodeRetrievalClockClass::MonotonicMilliseconds
        .encode()
        .unwrap();
    let (result, allocation) = allocation_probe::observe(|| {
        producer.stage(&f.experience, &f.memory, &f.intent, &f.policy, &clock)
    });
    result.unwrap();
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    let mut loader = PreparedEpisodeRetrievalPacket::prepare();
    let (snapshot, name) = loader.decode(producer.capture()).unwrap();
    assert_eq!(name, "Pete");
    assert_eq!(snapshot.input().memory, f.memory);
    assert_eq!(snapshot.input().experience, f.experience);
    assert_eq!(snapshot.input().reference.ticks, 100);
    assert_eq!(snapshot.input().retrieved.ticks, 100);
    let mut selector = conduit_ai::BoundedTemporalEvidenceFlow::prepare();
    let selected = selector.execute(producer.window(), &f.intent).unwrap();
    let mut verifier = PreparedEpisodeRetrievalWindow::prepare();
    verifier.build(&snapshot).unwrap();
    verifier.resolve(&snapshot, selected).unwrap();
    producer.commit();
    assert_eq!(
        producer.stage(&f.experience, &f.memory, &f.intent, &f.policy, &clock),
        Err(EpisodeRetrievalPacketRefusal::OutOfOrder)
    );
    assert!(producer.capture().is_empty());
    assert!(producer.window().is_empty());
}
#[test]
fn producer_refuses_wrong_owner_clock_and_packet_type_atomically() {
    let f = Fixture::new();
    let mut producer = PreparedEpisodeRetrievalProducer::prepare();
    producer.admit_context(&context()).unwrap();
    let wrong_clock = EpisodeRetrievalClockClass::UnixEpochMilliseconds
        .encode()
        .unwrap();
    assert_eq!(
        producer.stage(&f.experience, &f.memory, &f.intent, &f.policy, &wrong_clock),
        Err(EpisodeRetrievalPacketRefusal::Snapshot(
            EpisodeRetrievalSnapshotRefusal::ClockMismatch
        ))
    );
    let clock = EpisodeRetrievalClockClass::MonotonicMilliseconds
        .encode()
        .unwrap();
    let wrong_memory = memory(&[1, 2], "body/other", "wake/one");
    assert_eq!(
        producer.stage(&f.experience, &wrong_memory, &f.intent, &f.policy, &clock),
        Err(EpisodeRetrievalPacketRefusal::Context)
    );
    assert!(producer
        .stage(&f.memory, &f.memory, &f.intent, &f.policy, &clock)
        .is_err());
    assert!(producer.window().is_empty());
    assert!(producer.capture().is_empty());
    producer
        .stage(&f.experience, &f.memory, &f.intent, &f.policy, &clock)
        .unwrap();
    producer.cancel();
    assert!(producer.window().is_empty());
    assert!(producer.capture().is_empty());
}
#[test]
fn episode_integration_preserves_the_exact_reference_even_without_evidence() {
    let f = Fixture::new();
    let experience = EpisodeExperience::decode(&f.experience).unwrap();
    let reference = experience.reference().clone();
    let input = EpisodeExperienceInput::new(
        conduit_human::ExperienceIntegrationInput::new(
            experience.evidence().clone(),
            conduit_human::ExperienceTemporalPolicy::new(1, 2).unwrap(),
            reference.clone(),
        )
        .unwrap(),
        experience.observation().clone(),
    )
    .unwrap();
    let integrated = integrate_episode_experience(
        &input,
        conduit_semantic_catalog::experience_evidence_limits(),
    )
    .unwrap();
    assert_eq!(integrated.reference(), &reference);
}
