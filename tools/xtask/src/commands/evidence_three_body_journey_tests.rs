use super::*;

fn contract() -> JourneyContract {
    JourneyContract {
        schema: CONTRACT_SCHEMA.into(),
        journey_id: "orifina/tutorial@1".into(),
        git_commit: "a".repeat(40),
        steps: vec![ContractStep {
            step_id: "body.born".into(),
            required_assertion: "one-new-body-exists".into(),
            allowed_dispositions: vec!["established".into()],
            required_evidence_classes: vec!["semantic-receipt".into()],
            required_provenance: vec![ProvenanceField::Body, ProvenanceField::Manifestation],
            non_claims: vec!["not-physical-proof".into()],
        }],
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
        steps: vec![TrackStep {
            step_id: "body.born".into(),
            assertion: "one-new-body-exists".into(),
            disposition: "established".into(),
            provenance: StepProvenance {
                body_id: Some(format!("body-{index}")),
                host_id: Some(format!("host-{index}-0")),
                plan_id: (hosts > 1).then(|| format!("plan-{index}")),
                play_id: None,
                presentation_id: Some(format!("presentation-{index}")),
                manifestation_id: Some(format!("manifestation-{index}")),
                line_id: (hosts > 1).then(|| format!("line-{index}")),
                sign_id: Some(format!("sign-{index}")),
            },
            evidence: vec![StepEvidence {
                artifact_id: format!("artifact-{index}"),
                evidence_class: "semantic-receipt".into(),
                path: PathBuf::from(format!("artifact-{index}.json")),
                sha256: format!("sha256:{:064x}", index + 1),
            }],
        }],
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
fn one_body_with_three_skins_refuses() {
    let mut tracks = complete();
    tracks[1].body_id = tracks[0].body_id.clone();
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
