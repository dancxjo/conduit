use super::*;

fn contract() -> JourneyContract {
    JourneyContract {
        schema: CONTRACT_SCHEMA.into(),
        journey_id: "orifina/tutorial@1".into(),
        git_commit: "a".repeat(40),
        steps: REQUIRED_MILESTONES
            .iter()
            .enumerate()
            .map(|(index, milestone)| ContractStep {
                step_id: format!("journey.step-{index}"),
                milestone: Some(*milestone),
                title: format!("Journey milestone {index}"),
                what_happened: "The Body advanced through the shared tutorial.".into(),
                what_conduit_established: "The exact semantic milestone was retained.".into(),
                concepts: vec!["Body".into(), "biography".into()],
                required_assertion: format!("milestone-{index}-established"),
                required_assertion_rung: EvidenceRung::BodyBiography,
                allowed_dispositions: vec!["established".into()],
                required_evidence_classes: vec!["semantic-receipt".into()],
                required_provenance: vec![ProvenanceField::Body, ProvenanceField::Manifestation],
                non_claims: vec!["not-physical-proof".into()],
            })
            .collect(),
    }
}

fn track(index: usize, hosts: usize) -> BodyTrack {
    BodyTrack {
        schema: TRACK_SCHEMA.into(),
        journey_id: "orifina/tutorial@1".into(),
        git_commit: "a".repeat(40),
        track_id: format!("track-{index}"),
        embodiment: format!("embodiment-{index}"),
        body_id: format!("body-{index}"),
        presenter_id: format!("presenter-{index}"),
        hosts: (0..hosts)
            .map(|host| HostIdentity {
                host_id: format!("host-{index}-{host}"),
                boot_id: format!("boot-{index}-{host}"),
            })
            .collect(),
        line_ids: if hosts > 1 {
            vec![format!("line-{index}")]
        } else {
            Vec::new()
        },
        distributed_plan_ids: if hosts > 1 {
            vec![format!("plan-{index}")]
        } else {
            Vec::new()
        },
        steps: REQUIRED_MILESTONES
            .iter()
            .enumerate()
            .map(|(step, _)| TrackStep {
                step_id: format!("journey.step-{step}"),
                assertion: format!("milestone-{step}-established"),
                disposition: "established".into(),
                provenance: StepProvenance {
                    body_id: Some(format!("body-{index}")),
                    host_id: Some(format!("host-{index}-0")),
                    plan_id: (hosts > 1).then(|| format!("plan-{index}")),
                    play_id: None,
                    presentation_id: Some(format!("presentation-{index}-{step}")),
                    manifestation_id: Some(format!("manifestation-{index}-{step}")),
                    line_id: (hosts > 1).then(|| format!("line-{index}")),
                    sign_id: Some(format!("sign-{index}-{step}")),
                },
                evidence: vec![StepEvidence {
                    artifact_id: format!("artifact-{index}-{step}"),
                    evidence_class: "semantic-receipt".into(),
                    assertion_rung: EvidenceRung::BodyBiography,
                    documentary_description: "The exact accepted milestone receipt.".into(),
                    path: PathBuf::from(format!("artifact-{index}-{step}.json")),
                    sha256: format!("sha256:{:064x}", index * 100 + step + 1),
                }],
            })
            .collect(),
    }
}

fn complete() -> Vec<BodyTrack> {
    vec![track(0, 1), track(1, 1), track(2, 2)]
}

#[test]
fn three_distinct_bodies_share_semantics_without_collapsing_identities() {
    validate(&contract(), &complete()).unwrap();
}

#[test]
fn incomplete_or_reordered_lifecycle_refuses() {
    let mut incomplete = contract();
    incomplete
        .steps
        .retain(|step| step.milestone != Some(JourneyMilestone::HostAdded));
    assert_eq!(
        validate(&incomplete, &complete()).unwrap_err(),
        "semantic Journey milestone FaultObserved is duplicated or out of order"
    );

    let mut reordered = contract();
    reordered.steps.swap(7, 8);
    assert_eq!(
        validate(&reordered, &complete()).unwrap_err(),
        "semantic Journey milestone FaultObserved is duplicated or out of order"
    );
}

#[test]
fn one_body_with_three_skins_refuses() {
    let mut tracks = complete();
    tracks[1].body_id = tracks[0].body_id.clone();
    assert!(validate(&contract(), &tracks).is_err());
}

#[test]
fn three_tracks_must_not_reuse_one_embodiment_or_presenter() {
    let mut tracks = complete();
    tracks[1].embodiment = tracks[0].embodiment.clone();
    assert!(validate(&contract(), &tracks).is_err());

    tracks = complete();
    tracks[1].presenter_id = tracks[0].presenter_id.clone();
    assert!(validate(&contract(), &tracks).is_err());
}

#[test]
fn independently_born_hosts_must_not_reuse_boot_identity() {
    let mut tracks = complete();
    tracks[1].hosts[0].boot_id = tracks[0].hosts[0].boot_id.clone();
    assert!(validate(&contract(), &tracks).is_err());
}

#[test]
fn three_single_host_tracks_do_not_prove_a_distributed_body() {
    let tracks = vec![track(0, 1), track(1, 1), track(2, 1)];
    assert_eq!(
        validate(&contract(), &tracks).unwrap_err(),
        "at least one Body must retain multi-Host, Line, and distributed Plan truth"
    );
}

#[test]
fn reordered_semantics_and_cross_body_manifestations_refuse() {
    let mut tracks = complete();
    tracks[1].steps[0].step_id = "body.awake".into();
    assert!(validate(&contract(), &tracks).is_err());
    tracks = complete();
    tracks[1].steps[0].provenance.manifestation_id =
        tracks[0].steps[0].provenance.manifestation_id.clone();
    assert_eq!(
        validate(&contract(), &tracks).unwrap_err(),
        "Body tracks collapsed exact runtime or presentation identity"
    );
}

#[test]
fn steps_cannot_cite_another_tracks_host_or_an_unretained_line() {
    let mut tracks = complete();
    tracks[1].steps[0].provenance.host_id = Some("host-0-0".into());
    assert!(validate(&contract(), &tracks).is_err());
    tracks = complete();
    tracks[2].steps[0].provenance.line_id = Some("line-invented".into());
    assert!(validate(&contract(), &tracks).is_err());
}

#[test]
fn documentary_artifacts_are_digest_bound_and_root_confined() {
    let root =
        std::env::temp_dir().join(format!("conduit-three-body-journey-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("track.json");
    let mut track = track(0, 1);
    for step in &mut track.steps {
        let artifact = root.join(&step.evidence[0].path);
        std::fs::write(&artifact, b"semantic receipt").unwrap();
        step.evidence[0].sha256 = format!("sha256:{:x}", Sha256::digest(b"semantic receipt"));
    }
    verify_artifacts(&track, &source).unwrap();
    let artifact = root.join("artifact-0-0.json");
    std::fs::write(&artifact, b"changed receipt").unwrap();
    assert!(verify_artifacts(&track, &source).is_err());

    #[cfg(unix)]
    {
        let outside = root.with_extension("outside");
        std::fs::write(&outside, b"semantic receipt").unwrap();
        std::fs::remove_file(&artifact).unwrap();
        std::os::unix::fs::symlink(&outside, &artifact).unwrap();
        assert!(verify_artifacts(&track, &source).is_err());
        std::fs::remove_file(outside).unwrap();
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn published_index_preserves_semantic_assertions_and_non_claims() {
    let contract = contract();
    let index = ThreeBodyJourneyIndex {
        schema: INDEX_SCHEMA,
        disposition: "complete",
        journey_id: contract.journey_id,
        git_commit: contract.git_commit,
        semantic_steps: contract.steps,
        tracks: complete(),
    };
    let page = page::render(&index);
    let value = serde_json::to_value(index).unwrap();
    assert_eq!(
        value["semantic_steps"][0]["required_assertion"],
        "milestone-0-established"
    );
    assert_eq!(
        value["semantic_steps"][0]["required_assertion_rung"],
        "body-biography"
    );
    assert_eq!(
        value["semantic_steps"][0]["non_claims"][0],
        "not-physical-proof"
    );
    assert_eq!(value["tracks"].as_array().unwrap().len(), REQUIRED_TRACKS);
    assert!(page.contains("Follow one Body"));
    assert!(page.contains("Compare one semantic step"));
    assert!(page.contains("not-physical-proof"));
}

#[test]
fn downstream_manifestation_cannot_prove_upstream_semantic_truth() {
    let mut tracks = complete();
    for track in &mut tracks {
        track.steps[0].evidence[0].assertion_rung = EvidenceRung::GeneratedManifestation;
    }
    assert_eq!(
        validate(&contract(), &tracks).unwrap_err(),
        "track-0 lacks authoritative BodyBiography evidence at journey.step-0"
    );
}

#[test]
fn publication_writes_both_views_and_refuses_overwrite() {
    let contract = contract();
    let index = ThreeBodyJourneyIndex {
        schema: INDEX_SCHEMA,
        disposition: "complete",
        journey_id: contract.journey_id,
        git_commit: contract.git_commit,
        semantic_steps: contract.steps,
        tracks: complete(),
    };
    let root = std::env::temp_dir().join(format!(
        "conduit-three-body-publication-{}",
        std::process::id()
    ));
    let output = root.join("index.json");
    std::fs::create_dir_all(&root).unwrap();
    publish(&index, &output).unwrap();
    assert!(output.is_file());
    assert!(output.with_extension("html").is_file());
    assert_eq!(
        publish(&index, &output).unwrap_err(),
        "three-Body Journey publication refuses overwrite"
    );
    std::fs::remove_dir_all(root).unwrap();
}
