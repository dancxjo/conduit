//! One retained Body/Wake crosses an actual checked checkpoint offer transition.
use super::*;
use conduit_body::{WakeLifecycleEvent, WakePlanState};

fn checkpoint_files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .map(|path| {
            let bytes = std::fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect();
    files.sort();
    files
}

fn write_inputs(state: Vec<u8>, text: &str) -> [ExternalForeInput; 2] {
    [
        ExternalForeInput {
            front_port_id: port_id("current"),
            track: ConnectionTrack::Payload,
            bytes: state,
        },
        ExternalForeInput {
            front_port_id: port_id("command"),
            track: ConnectionTrack::Payload,
            bytes: TodoCommand::Add { text: text.into() }
                .encode_info()
                .unwrap(),
        },
    ]
}

#[test]
fn retained_body_replans_only_through_fresh_checkpoint_authority() {
    on_body_stack(|| {
        let root = root();
        let (mut host, first_plan) = planned(Mode::Write, &root, 2);
        let original_host = host.advertisement().host_id.clone();
        let original_boot = host.advertisement().boot_id.clone();
        let original_generation = host.advertisement().offer_generation;
        let old_placement = first_plan.fragments[0]
            .placements
            .iter()
            .find(|placement| {
                placement.implementation_id.as_str()
                    == conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION
            })
            .unwrap()
            .clone();
        let resident = ResidentPlot::new(
            first_plan.source_document_id.clone(),
            first_plan.checked_plot_id.clone(),
        );
        // Born and Wake are each issued exactly once; no deterministic-ID recreation.
        let body = Body::born(
            first_plan.source_document_id.clone(),
            first_plan.checked_plot_id.clone(),
            1,
            "sign/retained-body-born".into(),
        )
        .unwrap();
        let (awake, wake) = body.wake(1, "sign/retained-body-woke".into()).unwrap();
        let body_before = awake.clone();
        let first_body_plan = BodyPlan::seal(
            &wake,
            vec![BodyPlotPlan {
                plot: resident.clone(),
                plan: first_plan.clone(),
            }],
        )
        .unwrap();
        let inputs = write_inputs(
            TodoState::new("Groceries".into())
                .unwrap()
                .encode_info()
                .unwrap(),
            "Buy milk",
        );
        let mut first_fore = CapturedFore::default();
        let mut first_starts = 0;
        let first = host
            .run_body_plan_with_todo_checkpoint_to_with_start(
                BodyRunRequest {
                    wake: &wake,
                    plan: &first_body_plan,
                    control: &RunControl::default(),
                    keyboard: None,
                },
                BodyForeExchange {
                    inputs: &inputs,
                    output: &mut first_fore,
                },
                TodoCheckpointSelection {
                    root: &root,
                    identity: identity(),
                },
                &mut Vec::new(),
                &mut ThreadTimer,
                |_, _| {
                    first_starts += 1;
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(first_starts, 1);
        assert_eq!(first.terminal, TerminalDisposition::Completed);
        assert!(first.cleanup_failure.is_none());
        assert_eq!(first_fore.0.len(), 1);
        let retained_state = first_fore.0[0].bytes.to_vec();
        let prior_report = format!("{first:?}");
        let prior_files = checkpoint_files(&root);
        assert!(!prior_files.is_empty());
        host.transition_todo_checkpoint_offer(&root, content(Mode::Write, 3))
            .unwrap();
        assert_eq!(host.advertisement().host_id, original_host);
        assert_eq!(host.advertisement().boot_id, original_boot);
        assert!(host.advertisement().offer_generation > original_generation);
        // A's start Wake is historical; terminal+cleanup and the actual idle owner
        // transition above establish retirement before this explicit lifecycle step.
        let continuing = first
            .wake_at_start
            .became_unsatisfied(
                &first_body_plan.plan_id,
                "sign/checkpoint-offer-retired".into(),
            )
            .unwrap();
        assert_eq!(continuing.body_id, awake.body_id);
        assert_eq!(continuing.wake_id, wake.wake_id);
        assert_eq!(continuing.workset, wake.workset);
        let stale_inputs = write_inputs(retained_state.clone(), "Stale attempt");
        let mut stale_fore = CapturedFore::default();
        let mut stale_started = false;
        let stale_error = host
            .run_body_plan_with_todo_checkpoint_to_with_start(
                BodyRunRequest {
                    wake: &continuing,
                    plan: &first_body_plan,
                    control: &RunControl::default(),
                    keyboard: None,
                },
                BodyForeExchange {
                    inputs: &stale_inputs,
                    output: &mut stale_fore,
                },
                TodoCheckpointSelection {
                    root: &root,
                    identity: identity(),
                },
                &mut Vec::new(),
                &mut ThreadTimer,
                |_, _| {
                    stale_started = true;
                    Ok(())
                },
            )
            .unwrap_err();
        assert!(
            stale_error.contains("does not target the current host boot and offer"),
            "{stale_error}"
        );
        assert!(!stale_started);
        assert!(stale_fore.0.is_empty());
        assert_eq!(checkpoint_files(&root), prior_files);
        let stale_grant = plan_on_host(
            Mode::Write,
            &host,
            3,
            Some(old_placement.capability_id.clone()),
        )
        .unwrap_err();
        assert!(
            stale_grant.starts_with("AuthorityGrantMissing("),
            "{stale_grant}"
        );
        let replacement_plan = plan_on_host(Mode::Write, &host, 3, None).unwrap();
        assert_eq!(
            replacement_plan.source_document_id,
            first_plan.source_document_id
        );
        assert_eq!(replacement_plan.checked_plot_id, first_plan.checked_plot_id);
        assert_ne!(replacement_plan.plan_id, first_plan.plan_id);
        let new_placement = replacement_plan.fragments[0]
            .placements
            .iter()
            .find(|placement| placement.implementation_id == old_placement.implementation_id)
            .unwrap();
        assert_ne!(new_placement.capability_id, old_placement.capability_id);
        assert_ne!(new_placement.placement_id, old_placement.placement_id);
        assert_eq!(
            new_placement.offer_generation,
            host.advertisement().offer_generation
        );
        assert_eq!(
            new_placement.resources[0]
                .content
                .as_ref()
                .unwrap()
                .contract
                .version,
            ResourceVersionIdentity::from_digest([3; 32])
        );
        assert_eq!(new_placement.authority.len(), 1);
        let replacement_body_plan = BodyPlan::seal(
            &continuing,
            vec![BodyPlotPlan {
                plot: resident,
                plan: replacement_plan.clone(),
            }],
        )
        .unwrap();
        let inputs = write_inputs(retained_state, "Buy bread");
        let mut fresh_fore = CapturedFore::default();
        let mut fresh_starts = 0;
        let replacement = host
            .run_body_plan_with_todo_checkpoint_to_with_start(
                BodyRunRequest {
                    wake: &continuing,
                    plan: &replacement_body_plan,
                    control: &RunControl::default(),
                    keyboard: None,
                },
                BodyForeExchange {
                    inputs: &inputs,
                    output: &mut fresh_fore,
                },
                TodoCheckpointSelection {
                    root: &root,
                    identity: identity(),
                },
                &mut Vec::new(),
                &mut ThreadTimer,
                |_, _| {
                    fresh_starts += 1;
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(fresh_starts, 1);
        assert_eq!(replacement.terminal, TerminalDisposition::Completed);
        assert!(replacement.cleanup_failure.is_none());
        assert_eq!(fresh_fore.0.len(), 1);
        let state = TodoState::decode_info(&fresh_fore.0[0].bytes).unwrap();
        assert_eq!(
            state
                .items
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>(),
            ["Buy milk", "Buy bread"]
        );
        assert_eq!(awake, body_before);
        assert_eq!(replacement.play.body_id, first.play.body_id);
        assert_eq!(replacement.play.wake_id, first.play.wake_id);
        assert_ne!(replacement.play.active_play_id, first.play.active_play_id);
        assert_eq!(replacement.play.play_sequence, first.play.play_sequence + 1);
        let replanned = &replacement.wake_at_start;
        assert_eq!(
            replanned.workload_revision,
            first.wake_at_start.workload_revision
        );
        assert_eq!(replanned.workset, first.wake_at_start.workset);
        assert_eq!(replanned.plans.len(), 2);
        assert_eq!(replanned.plans[0].state, WakePlanState::Superseded);
        assert_eq!(replanned.plans[1].state, WakePlanState::Playing);
        assert!(replanned.events.iter().any(|event| matches!(event, WakeLifecycleEvent::Replanned { prior_plan_id, replacement_plan_id, .. } if prior_plan_id == &first_body_plan.plan_id && replacement_plan_id == &replacement_body_plan.plan_id)));
        assert_eq!(format!("{first:?}"), prior_report);
        println!(
            "{}",
            serde_json::json!({"schema":"conduit.body-replan-authority/v1","body":awake.body_id,"wake":wake.wake_id,"host":original_host,"boot":original_boot,"old_play":first.play,"new_play":replacement.play,"old_physical_plan":first_plan.plan_id,"new_physical_plan":replacement_plan.plan_id,"old_capability":old_placement.capability_id,"new_capability":new_placement.capability_id,"old_offer":original_generation,"new_offer":host.advertisement().offer_generation,"old_plan_refusal":stale_error,"old_grant_refusal":stale_grant,"retained_items":state.items.iter().map(|item|item.text.as_str()).collect::<Vec<_>>(),"not_established":["externally replayable lease or physical-device Base handle","DMA quiescence","hostile confinement","all-mechanism6409 acceptance"]})
        );
        std::fs::remove_dir_all(root).unwrap();
    });
}
