use super::*;
use conduit_body::Body;
use conduit_core::{CheckedPlotId, ExpandedPlotId, PlanId, SignId, SourceDocumentId};
use conduit_presentation::*;
use std::path::Path;

fn presentation() -> Presentation {
    presentation_with_text("Ready.")
}

pub(super) fn presentation_with_text(text: &str) -> Presentation {
    let body = Body::born(
        SourceDocumentId::from("source/presentation-fixture"),
        CheckedPlotId::from("checked/presentation-fixture"),
        1,
        SignId::from("sign/presentation-fixture/born"),
    )
    .unwrap();
    Presentation::new_with_semantics(
        1,
        PresentationBasis {
            body_id: Some(body.body_id),
            wake_id: None,
            source_document_id: Some(SourceDocumentId::from("source/presentation-fixture")),
            checked_plot_id: Some(CheckedPlotId::from("checked/presentation-fixture")),
            expanded_plot_id: Some(ExpandedPlotId::from("expanded/presentation-fixture")),
            plan_id: Some(PlanId::from("plan/presentation-fixture")),
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![PresentationSubject {
            identity: "body/current".into(),
            role: PresentationRole::Body,
            name: "Current body".into(),
        }],
        vec![],
        vec![],
        vec![PresentationText {
            subject: "body/current".into(),
            text: text.into(),
        }],
        vec![],
        vec![PresentationDisclosure {
            subject: "body/current".into(),
            level: PresentationDisclosureLevel::Primary,
        }],
    )
    .unwrap()
}

pub(super) fn retained(presentation: Presentation) -> GeneratedManifestationCandidate {
    let request = GenerativePresenterRequest::from_presentation(
        "request/retained".into(),
        GenerativePresenterPolicy {
            template_contract_revision: "template/spoken-mask@1".into(),
            narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
            instructions: "Speak the supplied text.".into(),
        },
        presentation,
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .unwrap();
    let mut manifestation = GeneratedManifestationCandidate {
        candidate_identity: String::new(),
        request_identity: request.request_identity.clone(),
        source_presentation_identity: request.semantic_data.source_presentation_identity.clone(),
        source_presentation_revision: request.semantic_data.source_presentation_revision,
        presenter_implementation_identity: "fixture/presenter@1".into(),
        provider_identity: "fixture/provider".into(),
        model_identity: "fixture/model".into(),
        template_contract_revision: request.policy.template_contract_revision.clone(),
        mask_contract_revision: conduit_presentation::SPOKEN_MASK_CONTRACT_REVISION.into(),
        generation_run_identity: "run/spoken-mask".into(),
        disposition: GeneratedManifestationDisposition::Produced,
        content: vec![GeneratedContentSegment {
            role: GeneratedContentRole::Speech,
            source_text_index: 0,
            bytes: request.semantic_data.presentation.text[0]
                .text
                .as_bytes()
                .to_vec(),
        }],
        affordances: vec![],
        correlations: vec![
            GeneratedSemanticCorrelation::Subject {
                index: 0,
                identity: request.semantic_data.presentation.subjects[0]
                    .identity
                    .clone(),
            },
            GeneratedSemanticCorrelation::Text {
                index: 0,
                subject: request.semantic_data.presentation.text[0].subject.clone(),
            },
        ],
        raw_provider_output: None,
        wording_proposal: None,
    };
    manifestation.candidate_identity = manifestation.digest();
    manifestation
}

#[test]
fn finite_wording_replay_uses_the_presenter_policy_and_revalidates_the_face() {
    let face = presentation();
    let mut candidate = retained(face.clone());
    let proposal = GeneratedWordingProposal {
        source_presentation_identity: face.identity.as_str().into(),
        source_presentation_revision: face.revision,
        clauses: vec![GeneratedWordingClause::Text {
            index: 0,
            subject: face.text[0].subject.clone(),
            value: face.text[0].text.clone(),
            style: GeneratedWordingStyle::Direct,
        }],
    };
    candidate.template_contract_revision = FINITE_FACE_WORDING_TEMPLATE_REVISION.into();
    candidate.raw_provider_output = Some(
        serde_json::json!({"proposal": proposal, "suggested_action_identities": []}).to_string(),
    );
    candidate.wording_proposal = Some(proposal);
    candidate.candidate_identity = candidate.digest();
    let shown = execute_retained_manifestation_mask(
        "finite-replay-fixture",
        "finite-replay-fixture",
        face.clone(),
        candidate.clone(),
    )
    .expect("the exact finite wording proposal must reach an acknowledged Show");
    assert_eq!(
        shown.shown.show.show.lifecycle,
        ManifestationLifecycle::Available
    );
    assert_eq!(shown.shown.show.show.presentation_id, face.identity);

    let mut invented = candidate.clone();
    let GeneratedWordingClause::Text { value, .. } =
        &mut invented.wording_proposal.as_mut().unwrap().clauses[0]
    else {
        unreachable!()
    };
    *value = "An invented state.".into();
    invented.candidate_identity = invented.digest();
    assert!(execute_retained_manifestation_mask(
        "invented-replay-fixture",
        "invented-replay-fixture",
        face.clone(),
        invented,
    )
    .is_err());

    candidate.template_contract_revision = "unknown/policy".into();
    candidate.candidate_identity = candidate.digest();
    let refused = execute_retained_manifestation_mask(
        "unknown-replay-fixture",
        "unknown-replay-fixture",
        face,
        candidate,
    )
    .unwrap_err();
    assert!(refused.contains("unsupported spoken Mask template"));
}

#[test]
#[ignore = "requires explicitly installed eSpeak NG, English data and shared library; produces WAV only, no playback"]
fn installed_espeak_mask_retains_acknowledged_wav_through_plan_and_play() {
    let engine = std::fs::canonicalize("/usr/lib/x86_64-linux-gnu/libespeak-ng.so.1").unwrap();
    let discovery = crate::hosted_speech_synthesis::EspeakDiscovery::inspect(
        Path::new("/usr/bin/espeak-ng"),
        Path::new("/usr/lib/x86_64-linux-gnu/espeak-ng-data"),
        "en-us",
        &[engine],
    )
    .unwrap();
    let directory =
        std::env::temp_dir().join(format!("conduit-real-mask-proof-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let destination = directory.join("ready.wav");
    let face = presentation();
    let result = execute_retained_manifestation_mask_with_espeak(
        "real-spoken-proof",
        "real-spoken-proof",
        face.clone(),
        retained(face),
        discovery,
        &destination,
    )
    .unwrap();
    let shown = &result.execution.shown;
    let voices: Vec<_> = result
        .execution
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .filter(|placement| placement.kind_id.as_str() == "speech/synthesize")
        .collect();
    assert_eq!(voices.len(), 1);
    assert_eq!(
        voices[0].implementation_id.as_str(),
        conduit_std_offers::ESPEAK_SPEECH_IMPLEMENTATION
    );
    assert_eq!(shown.show.show.lifecycle, ManifestationLifecycle::Available);
    assert_eq!(shown.artifact.plan_id, result.execution.plan.plan_id);
    assert_eq!(
        result.artifact.artifact_identity,
        shown.artifact.artifact_identity
    );
    assert_eq!(result.artifact.pcm_sha256, shown.artifact.content_sha256);
    let wav = std::fs::read(&destination).unwrap();
    assert_eq!(result.artifact.wav_bytes, wav.len() as u64);
    assert_eq!(
        result.artifact.wav_bytes,
        u64::from(shown.artifact.pcm_bytes) + 44
    );
    assert!(shown.artifact.frames > 0 && shown.artifact.frames <= 48_000 * 3);
    assert!(wav[44..].iter().any(|byte| *byte != 0));
    use sha2::{Digest, Sha256};
    assert_eq!(
        result.artifact.wav_sha256,
        format!("{:x}", Sha256::digest(&wav))
    );
    std::fs::remove_file(destination).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
#[ignore = "requires installed eSpeak NG; validates real streaming Mask audio using an explicitly fixture Presenter candidate"]
fn installed_espeak_streaming_mask_acknowledges_substantive_chapter() {
    let words = "This Body keeps the clock you started. Change the interval, then inspect the connections to see how your action reaches the running work. You can pause the Body without erasing its history. When you return, inspect the current host and the new plan before starting again. If a presentation host disappears, the Body must show what stopped and which admitted route can continue. Your preference chooses among available Masks; it never invents a missing host.";
    let engine = std::fs::canonicalize("/usr/lib/x86_64-linux-gnu/libespeak-ng.so.1").unwrap();
    let discovery = crate::hosted_speech_synthesis::EspeakDiscovery::inspect(
        Path::new("/usr/bin/espeak-ng"),
        Path::new("/usr/lib/x86_64-linux-gnu/espeak-ng-data"),
        "en-us",
        &[engine],
    )
    .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "conduit-streamed-mask-proof-{}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let destination = directory.join("chapter.wav");
    let face = presentation_with_text(words);
    let candidate = retained(face.clone());
    let result = execute_retained_manifestation_mask_with_streaming_espeak(
        "streamed-spoken-proof",
        "streamed-spoken-proof",
        face.clone(),
        candidate.clone(),
        discovery,
        &destination,
    )
    .unwrap();
    let shown = &result.execution.shown;
    assert_eq!(shown.show.show.lifecycle, ManifestationLifecycle::Available);
    assert_eq!(shown.artifact.plan_id, result.execution.plan.plan_id);
    assert!((20 * 48000..=30 * 48000).contains(&shown.artifact.frames));
    assert_eq!(result.artifact.pcm_sha256, shown.artifact.content_sha256);
    let voices: Vec<_> = result
        .execution
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .filter(|p| p.kind_id.as_str() == "speech/synthesize-stream")
        .collect();
    assert_eq!(voices.len(), 1);
    assert_eq!(
        voices[0].implementation_id.as_str(),
        conduit_std_offers::ESPEAK_STREAM_IMPLEMENTATION
    );
    for (name, value) in [
        ("face.json", serde_json::to_value(&face).unwrap()),
        (
            "fixture-candidate.json",
            serde_json::to_value(&candidate).unwrap(),
        ),
        (
            "plan.json",
            serde_json::to_value(&result.execution.plan).unwrap(),
        ),
        ("show.json", serde_json::to_value(shown).unwrap()),
    ] {
        std::fs::write(
            directory.join(name),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
    }
    std::fs::write(directory.join("words.txt"), words).unwrap();
    eprintln!("Actual streamed Mask audio retained at {} (fixture wording; no live model or playback claim)", directory.display());
}
