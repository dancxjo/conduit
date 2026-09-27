use super::*;

fn receipt(track: usize, step_id: &str, assertion: &str) -> TrackStep {
    TrackStep {
        step_id: step_id.into(),
        assertion: assertion.into(),
        disposition: "established".into(),
        provenance: StepProvenance {
            body_id: Some(format!("body-{track}")),
            host_id: None,
            boot_id: None,
            plan_id: Some(format!("plan-{track}")),
            play_id: Some(format!("play-{track}")),
            presentation_id: Some(format!("presentation-{track}-{step_id}")),
            manifestation_id: Some(format!("manifestation-{track}-{step_id}")),
            line_id: (track == 2).then(|| "line-2".into()),
            sign_id: Some(format!("sign-{track}-{step_id}")),
        },
        evidence: vec![StepEvidence {
            artifact_id: format!("artifact-{track}-{step_id}"),
            evidence_class: "semantic-receipt".into(),
            assertion_rung: EvidenceRung::RuntimeReceipt,
            documentary_description: format!("Producer receipt for {step_id}."),
            path: PathBuf::from(format!("artifact-{track}-{step_id}.json")),
            sha256: format!("sha256:{:064x}", track + step_id.len()),
        }],
    }
}

fn track(index: usize) -> BodyTrack {
    let details = [
        ("body.absent", "body-absent"),
        ("bootstrap.started", "bootstrap-started"),
        ("body.born", "body-born"),
        ("body.awake", "body-awake"),
        ("form.used", "standing-form-used"),
        ("body.inspected", "body-inspected"),
        ("fault.observed", "fault-observed"),
        ("body.repaired", "body-repaired"),
        ("body.lulled", "body-lulled"),
        ("body.fulfilled", "body-fulfilled"),
    ];
    BodyTrack {
        schema: TRACK_SCHEMA.into(),
        journey_id: "orifina/tutorial@1".into(),
        git_commit: "a".repeat(40),
        track_id: format!("track-{index}"),
        embodiment: format!("embodiment-{index}"),
        body_id: format!("body-{index}"),
        presenter_id: format!("presenter-{index}"),
        hosts: if index == 2 {
            vec![
                HostIdentity {
                    host_id: "host-2-a".into(),
                    boot_id: "boot-2-a".into(),
                },
                HostIdentity {
                    host_id: "host-2-b".into(),
                    boot_id: "boot-2-b".into(),
                },
            ]
        } else {
            vec![HostIdentity {
                host_id: format!("host-{index}"),
                boot_id: format!("boot-{index}"),
            }]
        },
        line_ids: if index == 2 {
            vec!["line-2".into()]
        } else {
            Vec::new()
        },
        distributed_plan_ids: if index == 2 {
            vec!["plan-2".into()]
        } else {
            Vec::new()
        },
        receipts: details
            .iter()
            .map(|(id, assertion)| receipt(index, id, assertion))
            .collect(),
        actions: vec![
            TrackActionObservation {
                action_id: "journey.bootstrap".into(),
                concrete_event: format!(
                    "Embodiment {index} began without a Body and started bootstrap."
                ),
                receipt_ids: vec!["body.absent".into(), "bootstrap.started".into()],
            },
            TrackActionObservation {
                action_id: "journey.birth".into(),
                concrete_event: format!("Embodiment {index} created and woke its Body."),
                receipt_ids: vec!["body.born".into(), "body.awake".into()],
            },
            TrackActionObservation {
                action_id: "journey.useful-work".into(),
                concrete_event: format!("Embodiment {index} performed its concrete useful work."),
                receipt_ids: vec!["form.used".into()],
            },
            TrackActionObservation {
                action_id: "journey.break-recover".into(),
                concrete_event: format!(
                    "Embodiment {index} retained a fault and its recovery outcome."
                ),
                receipt_ids: vec!["fault.observed".into(), "body.repaired".into()],
            },
            TrackActionObservation {
                action_id: "journey.rest-finish".into(),
                concrete_event: format!("Embodiment {index} lulled and fulfilled its Body."),
                receipt_ids: vec!["body.lulled".into(), "body.fulfilled".into()],
            },
        ],
    }
}

fn complete() -> Vec<BodyTrack> {
    vec![track(0), track(1), track(2)]
}

#[test]
fn exactly_three_materially_distinct_tracks_share_five_ordered_actions() {
    let contract = contract::canonical(&"a".repeat(40));
    validate(&contract, &complete(), &contract.git_commit).unwrap();
    let index = assemble_index(contract, complete(), None).unwrap();
    assert_eq!(index.actions.len(), 5);
    assert!(index.actions.iter().all(|action| action.bodies.len() == 3));
    assert_eq!(index.actions[2].action.action_id, "journey.useful-work");
}

#[test]
fn detailed_receipts_are_richer_than_public_actions() {
    let tracks = complete();
    assert!(tracks
        .iter()
        .all(|track| track.receipts.len() > track.actions.len()));
    assert!(tracks.iter().all(|track| track
        .receipts
        .iter()
        .any(|receipt| receipt.step_id == "body.inspected")));
}

#[test]
fn action_order_and_required_semantics_fail_closed() {
    let contract = contract::canonical(&"a".repeat(40));
    let mut tracks = complete();
    tracks[0].actions.swap(0, 1);
    assert!(validate(&contract, &tracks, &contract.git_commit)
        .unwrap_err()
        .contains("shuffled"));
    let mut tracks = complete();
    tracks[1].actions[3].receipt_ids = vec!["body.repaired".into()];
    assert!(validate(&contract, &tracks, &contract.git_commit)
        .unwrap_err()
        .contains("required semantic receipts"));
}

#[test]
fn extra_detailed_receipts_do_not_become_public_actions() {
    let contract = contract::canonical(&"a".repeat(40));
    let mut tracks = complete();
    tracks[0]
        .receipts
        .push(receipt(0, "mask.replanned", "mask-replanned"));
    validate(&contract, &tracks, &contract.git_commit).unwrap();
    let index = assemble_index(contract, tracks, None).unwrap();
    assert_eq!(index.actions.len(), 5);
    assert!(index.actions.iter().all(|action| action.bodies[0]
        .receipts
        .iter()
        .all(|receipt| receipt.step_id != "mask.replanned")));
}

#[test]
fn identity_collapsing_and_stale_commits_refuse() {
    let contract = contract::canonical(&"a".repeat(40));
    let mut tracks = complete();
    tracks[1].body_id = tracks[0].body_id.clone();
    assert!(validate(&contract, &tracks, &contract.git_commit).is_err());
    assert!(validate(&contract, &complete(), &"b".repeat(40)).is_err());
}

#[test]
fn action_major_page_names_producer_events_without_fixed_manifestation_labels() {
    let contract = contract::canonical(&"a".repeat(40));
    let index = assemble_index(contract, complete(), None).unwrap();
    let html = page::render(&index);
    assert!(html.contains("The same moment, three ways"));
    assert!(html.contains("Embodiment 2 performed its concrete useful work."));
    let value = serde_json::to_value(index).unwrap();
    assert_eq!(value["actions"][0]["bodies"].as_array().unwrap().len(), 3);
    assert!(value.get("tracks").is_none());
}
