//! Real Current integration, retention, checkpoint recovery, and selected model data.
use super::*;
use conduit_human::*;
use conduit_plot::rust_binding::BoundedSequence;

const SOURCE: &str = include_str!("../../../../plots/pete-cognition-resumable/main.conduit");

// Fenced sensory adapter: the ordinary integrator still constructs CurrentExperience.
// Only certainty varies; identity, content, domain, time, and sources are identical.
fn classified_input(bytes: &[u8], certainty: ExperienceCertainty) -> Result<Vec<u8>, u16> {
    let encoded = conduit_pete::prepare_pete_experience_input_bytes(bytes)?;
    let input = conduit_chat::EpisodeExperienceInput::decode(&encoded).unwrap();
    let integration = input.integration();
    let evidence = integration.evidence();
    let items = evidence.evidence().get().iter().map(|item| {
        ExperienceEvidenceItem::new(
            *item.availability(),
            certainty,
            item.content_kind().clone(),
            *item.domain(),
            item.encoded_content().clone(),
            item.identity().clone(),
            item.observed_at().clone(),
            *item.origin(),
            item.provenance_revision().clone(),
            item.recorded_at().clone(),
            *item.revision(),
            item.sources().clone(),
            *item.temporal_role(),
        )
        .unwrap()
    });
    let evidence = ExperienceEvidenceRequest::new(
        ExperienceEvidenceItems::new(BoundedSequence::try_from_iter(items).unwrap()).unwrap(),
        evidence.selection().clone(),
    )
    .unwrap();
    conduit_chat::EpisodeExperienceInput::new(
        ExperienceIntegrationInput::new(
            evidence,
            *integration.policy(),
            integration.reference().clone(),
        )
        .unwrap(),
        input.observation().clone(),
    )
    .unwrap()
    .encode()
    .map_err(|_| 1)
}
fn certain(bytes: &[u8]) -> Result<Vec<u8>, u16> {
    classified_input(bytes, ExperienceCertainty::Certain)
}
fn uncertain(bytes: &[u8]) -> Result<Vec<u8>, u16> {
    classified_input(bytes, ExperienceCertainty::Uncertain)
}

#[test]
fn current_classifications_survive_checkpoint_and_selected_model_context() {
    for (transform, certainty) in [
        (certain as fn(&[u8]) -> Result<Vec<u8>, u16>, "certain"),
        (uncertain, "uncertain"),
    ] {
        let initial = conduit_chat::initial_episode_memory();
        let (model, first_requests) = common::fenced_model();
        let mut harness = pete_harness_with_transform(
            model,
            SOURCE,
            "pete-cognition-resumable",
            Some(&initial),
            transform,
        );
        let mut incoming = inputs(&[]);
        incoming.push(frame_input(frame(
            1,
            "The hallway label is COPPER KITE.",
            true,
        )));
        let mut collector = Collector::default();
        let report = run(
            &mut harness,
            &incoming,
            &mut collector,
            &RunControl::default(),
        )
        .unwrap();
        assert_eq!(terminal(&report), &TerminalDisposition::Completed);
        let first = common::request_data(&first_requests.lock().unwrap()[0]);
        let current = common::model_projection::expanded_model_evidence(
            &first["current_experience"],
            "The hallway label is COPPER KITE.",
        );
        let original = current["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["classifications"]["origin"] == "human_reported")
            .unwrap();
        assert_eq!(original["classifications"]["certainty"], certainty);
        let checkpoint = &collector
            .0
            .iter()
            .rev()
            .find(|item| item.front_port_id.as_str() == "checkpoints")
            .unwrap()
            .bytes;
        assert!(checkpoint.len() <= conduit_chat::MAXIMUM_EPISODE_CHECKPOINT_BYTES);
        let recovered = conduit_chat::EpisodeMemoryCell::from_checkpoint(checkpoint).unwrap();
        let (model, later_requests) = common::fenced_model();
        let mut resumed = pete_harness_for(
            model,
            SOURCE,
            "pete-cognition-resumable",
            Some(recovered.current()),
        );
        let mut incoming = inputs(&[]);
        incoming.push(frame_input(frame(2, "What was the hallway label?", false)));
        let mut later = Collector::default();
        let report = run(&mut resumed, &incoming, &mut later, &RunControl::default()).unwrap();
        assert_eq!(terminal(&report), &TerminalDisposition::Completed);
        let requests = later_requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].len() <= conduit_chat::MAXIMUM_EPISODE_RETRIEVAL_REQUEST_BYTES);
        let data = common::request_data(&requests[0]);
        let recalled = &data["recalled"][0];
        assert_eq!(recalled["episode"], 1);
        assert_eq!(recalled["origin"], "remembered");
        let selected = &recalled["historical_evidence"]["evidence"][0];
        for field in [
            "domain",
            "origin",
            "availability",
            "certainty",
            "temporal_role",
        ] {
            assert_eq!(
                selected[field], original["classifications"][field],
                "lost {field}: {certainty}"
            );
        }
        assert_eq!(selected["identity"], original["identity"]);
        assert_eq!(selected["sources"], original["sources"]);
        assert_eq!(selected["utf8_content"], original["utf8_content"]);
    }
}
