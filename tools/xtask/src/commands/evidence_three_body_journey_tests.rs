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
            face_id: Some(format!("face-{track}-{step_id}")),
            show_id: Some(format!("show-{track}-{step_id}")),
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

fn mask_actions(track: usize) -> Vec<MaskActionObservation> {
    let face_id = format!("mask-face-{track}");
    let first_plan = format!("mask-plan-{track}-initial");
    let replacement_plan = format!("mask-plan-{track}-replacement");
    let primary_mask = format!("mask-{track}-primary");
    let alternate_mask = format!("mask-{track}-alternate");
    conduit_presentation::MASK_JOURNEY_ACTIONS
        .iter()
        .enumerate()
        .map(|(position, action)| {
            let replacement = position >= 6;
            let unavailable = (3..=6).contains(&position);
            let restored = position >= 8;
            MaskActionObservation {
                action_id: action.id().into(),
                concrete_event: format!(
                    "Embodiment {track} enacted shared Mask action {}.",
                    action.id()
                ),
                face_id: face_id.clone(),
                selected_mask_form_id: Some(if position < 2 || restored {
                    primary_mask.clone()
                } else {
                    alternate_mask.clone()
                }),
                plan_id: if replacement {
                    replacement_plan.clone()
                } else {
                    first_plan.clone()
                },
                selected_route_id: (!unavailable).then(|| {
                    if restored {
                        format!("mask-route-{track}-replacement-primary")
                    } else if position >= 2 {
                        format!("mask-route-{track}-alternate")
                    } else {
                        format!("mask-route-{track}-primary")
                    }
                }),
                show_id: (!unavailable).then(|| format!("mask-show-{track}-{position}")),
                receipt_ids: vec![format!("mask.action-{position}")],
            }
        })
        .collect()
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
    let mut receipts = details
        .iter()
        .map(|(id, assertion)| receipt(index, id, assertion))
        .collect::<Vec<_>>();
    receipts.extend(
        conduit_presentation::MASK_JOURNEY_ACTIONS
            .iter()
            .enumerate()
            .map(|(position, action)| {
                receipt(
                    index,
                    &format!("mask.action-{position}"),
                    &format!("mask-{}", action.id()),
                )
            }),
    );
    let mask_actions = mask_actions(index);
    let mut actions = vec![
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
    ];
    actions.splice(
        3..3,
        mask_actions.iter().map(|action| TrackActionObservation {
            action_id: action.action_id.clone(),
            concrete_event: action.concrete_event.clone(),
            receipt_ids: action.receipt_ids.clone(),
        }),
    );
    BodyTrack {
        schema: TRACK_SCHEMA.into(),
        journey_id: "orifina/tutorial@1".into(),
        git_commit: "a".repeat(40),
        track_id: format!("track-{index}"),
        embodiment: format!("embodiment-{index}"),
        body_id: format!("body-{index}"),
        mask_form_id: format!("mask-form-{index}"),
        construction: if index < 2 {
            vec![ConstructionTruth {
                host_id: format!("host-{index}"),
                profile: ConstructionStage::Exact {
                    identity: format!("profile-{index}"),
                },
                build: ConstructionStage::Exact {
                    identity: format!("build-{index}"),
                },
                image: ConstructionStage::Exact {
                    identity: format!("image-{index}"),
                },
            }]
        } else {
            vec![ConstructionTruth {
                host_id: "host-2-a".into(),
                profile: ConstructionStage::Omitted {
                    reason: "This hosted journey uses an already-running Host and performs no profile make stage.".into(),
                },
                build: ConstructionStage::Omitted {
                    reason: "This hosted journey starts admitted implementations and produces no standalone build artifact.".into(),
                },
                image: ConstructionStage::Omitted {
                    reason: "This hosted journey is not booted from an image, so no image identity exists.".into(),
                },
            }, ConstructionTruth {
                host_id: "host-2-b".into(),
                profile: ConstructionStage::Omitted {
                    reason: "This peer hosted journey uses an already-running Host and performs no profile make stage.".into(),
                },
                build: ConstructionStage::Omitted {
                    reason: "This peer hosted journey starts admitted implementations and produces no standalone build artifact.".into(),
                },
                image: ConstructionStage::Omitted {
                    reason: "This peer hosted journey is not booted from an image, so no image identity exists.".into(),
                },
            }]
        },
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
        receipts,
        actions,
        mask_actions,
    }
}

fn complete() -> Vec<BodyTrack> {
    vec![track(0), track(1), track(2)]
}

#[test]
fn exactly_three_materially_distinct_tracks_share_one_ordered_action_journey() {
    let contract = contract::canonical(&"a".repeat(40));
    validate(&contract, &complete(), &contract.git_commit).unwrap();
    let index = assemble_index(contract, complete()).unwrap();
    assert_eq!(index.actions.len(), 15);
    assert!(index.actions.iter().all(|action| action.bodies.len() == 3));
    assert_eq!(index.actions[2].action.action_id, "journey.useful-work");
    assert_eq!(
        index.actions[3].action.action_id,
        "mask.inspect-initial-show"
    );
    assert_eq!(
        index.actions[12].action.action_id,
        "mask.inspect-restored-show"
    );
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
    tracks[1].actions[13].receipt_ids = vec!["body.repaired".into()];
    assert!(validate(&contract, &tracks, &contract.git_commit)
        .unwrap_err()
        .contains("required semantic receipts"));
}

#[test]
fn mask_journey_refuses_staged_or_invented_transition_truth() {
    let contract = contract::canonical(&"a".repeat(40));

    let mut tracks = complete();
    tracks[0].mask_actions.swap(2, 3);
    assert!(validate(&contract, &tracks, &contract.git_commit)
        .unwrap_err()
        .contains("reordered Mask action"));

    let mut tracks = complete();
    tracks[0].mask_actions[4].show_id = Some("invented-show".into());
    assert!(validate(&contract, &tracks, &contract.git_commit)
        .unwrap_err()
        .contains("invents a Show"));

    let mut tracks = complete();
    tracks[1].mask_actions[2].plan_id = "different-plan-too-early".into();
    assert!(validate(&contract, &tracks, &contract.git_commit)
        .unwrap_err()
        .contains("sealed same-Plan Mask selection"));

    let mut tracks = complete();
    tracks[1].mask_actions[6].plan_id = tracks[1].mask_actions[0].plan_id.clone();
    tracks[1].mask_actions[7].plan_id = tracks[1].mask_actions[0].plan_id.clone();
    assert!(validate(&contract, &tracks, &contract.git_commit)
        .unwrap_err()
        .contains("genuine replacement Plan"));

    let mut tracks = complete();
    tracks[2].mask_actions[9].selected_mask_form_id = Some("wrong-mask".into());
    assert!(validate(&contract, &tracks, &contract.git_commit)
        .unwrap_err()
        .contains("restore its original worn Mask"));
}

#[test]
fn mask_journey_refuses_retained_show_identity_as_current_truth() {
    let contract = contract::canonical(&"a".repeat(40));
    let mut tracks = complete();
    tracks[0].mask_actions[9].show_id = tracks[0].mask_actions[0].show_id.clone();

    let error = validate(&contract, &tracks, &contract.git_commit).unwrap_err();

    assert!(error.contains("reuses a retained Show as current Face truth"));
}

#[test]
fn extra_detailed_receipts_do_not_become_public_actions() {
    let contract = contract::canonical(&"a".repeat(40));
    let mut tracks = complete();
    tracks[0]
        .receipts
        .push(receipt(0, "mask.replanned", "mask-replanned"));
    validate(&contract, &tracks, &contract.git_commit).unwrap();
    let index = assemble_index(contract, tracks).unwrap();
    assert_eq!(index.actions.len(), 15);
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
fn construction_truth_covers_each_host_without_inventing_missing_stages() {
    let contract = contract::canonical(&"a".repeat(40));

    let mut tracks = complete();
    tracks[0].construction.clear();
    assert!(validate(&contract, &tracks, &contract.git_commit)
        .unwrap_err()
        .contains("invalid Body track"));

    let mut tracks = complete();
    tracks[2].construction[1].host_id = "host-not-in-body".into();
    assert!(validate(&contract, &tracks, &contract.git_commit)
        .unwrap_err()
        .contains("does not cover every Host"));

    let mut tracks = complete();
    tracks[2].construction[0].build = ConstructionStage::Exact {
        identity: "invented-build".into(),
    };
    assert!(validate(&contract, &tracks, &contract.git_commit)
        .unwrap_err()
        .contains("invalid Body track"));
}

#[test]
fn action_major_page_names_producer_events_without_fixed_manifestation_labels() {
    let contract = contract::canonical(&"a".repeat(40));
    let index = assemble_index(contract, complete()).unwrap();
    let html = page::render(&index);
    assert!(html.contains("The same moment, three ways"));
    assert!(html.contains("Embodiment 2 performed its concrete useful work."));
    assert!(html.contains("Host host-2-a construction: profile omitted:"));
    let value = serde_json::to_value(index).unwrap();
    assert_eq!(value["actions"][0]["bodies"].as_array().unwrap().len(), 3);
    assert!(value.get("tracks").is_none());
}
