//! Actual installed Body/Host output custody across a separately admitted replacement.
#[path = "replacement_fore_custody/accepted_then_cancelled.rs"]
mod accepted_then_cancelled;
#[path = "common/checkpoint_fixture.rs"]
mod checkpoint_fixture;
#[path = "replacement_fore_custody/provider_loss.rs"]
mod provider_loss;
use checkpoint_fixture::*;
use conduit_body::{
    Body, BodyPlan, BodyPlanningSession, BodyPlanningTransition, BodyPlayIdentity, BodyPlotPlan,
    ResidentPlot,
};
use conduit_core::*;
use conduit_std_host::body_execution::{
    BodyForeDeliveryStage, BodyForeExchange, BodyForeOutputAdapter, BodyRunRequest,
    TodoCheckpointSelection,
};
use conduit_std_host::{ExternalForeDelivery, RunControl, StdHostConfig, ThreadTimer};
use conduit_todo_plot::{TodoCommand, TodoState};

struct EffectThenError {
    received: Vec<ExternalForeDelivery>,
}
impl BodyForeOutputAdapter for EffectThenError {
    fn deliver(&mut self, delivery: &ExternalForeDelivery) -> Result<(), String> {
        // Deliberately retain the effect before reporting loss of acknowledgement.
        // The runner cannot infer whether this adapter performed the effect.
        self.received.push(delivery.clone());
        Err("external acknowledgement lost after effect".into())
    }
}

#[test]
fn uncertain_old_output_stays_with_old_play_and_is_never_replayed_into_replacement() {
    std::thread::Builder::new()
        .name("replacement-fore-custody".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(prove_uncertain_custody)
        .unwrap()
        .join()
        .unwrap();
}

fn prove_uncertain_custody() {
    let (mut old_host, old_plan, old_root) = planned_checkpoint();
    let old_plan_snapshot = old_plan.clone();
    let mut cache = conduit_planner::IncrementalPlanner::new(1).unwrap();
    let (cold_old, _) = compare_checkpoint_cache(&mut cache, &old_host, &old_plan);
    assert_eq!(cold_old, old_plan);
    let (_, unchanged) = compare_checkpoint_cache(&mut cache, &old_host, &old_plan);
    assert_eq!(unchanged.metrics.reused_candidates, 1);
    old_host.set_initial_kernel_play_sequence(1).unwrap();
    let body = Body::born(
        old_plan.source_document_id.clone(),
        old_plan.checked_plot_id.clone(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let mut session = BodyPlanningSession::prepare(
        &body,
        1,
        "sign/wake".into(),
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                old_plan.source_document_id.clone(),
                old_plan.checked_plot_id.clone(),
            ),
            plan: old_plan,
        }],
    )
    .unwrap();
    let old_wake = session.wake().clone();
    let old_body_plan = session.current_plan().clone();
    let claim = session
        .claim_execution(
            &old_body_plan.plan_id,
            &old_host.advertisement().host_id,
            &old_host.advertisement().boot_id,
        )
        .unwrap();
    let mut external = EffectThenError {
        received: Vec::with_capacity(1),
    };
    let old_report = old_host
        .run_body_plan_with_todo_checkpoint_to_with_start(
            BodyRunRequest {
                wake: &old_wake,
                plan: &old_body_plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            BodyForeExchange {
                inputs: &inputs(),
                output: &mut external,
            },
            TodoCheckpointSelection {
                root: &old_root,
                identity: checkpoint_identity(),
            },
            &mut Vec::new(),
            &mut ThreadTimer,
            |play, wake| {
                assert_eq!(play, &claim.play);
                session.report_execution_started(play, wake).unwrap();
                // An in-flight claim cannot be replaced by an availability hint.
                assert!(session.has_outstanding_execution_claim());
                Ok(())
            },
        )
        .unwrap();
    session
        .report_execution_terminal(
            &old_report.play,
            "failed",
            &old_report.terminal_sign.sign_id,
        )
        .unwrap();
    assert!(!session.has_outstanding_execution_claim());
    assert!(matches!(
        old_report.terminal,
        TerminalDisposition::Failed { .. }
    ));
    assert_eq!(
        old_report.failure.as_deref(),
        Some("external acknowledgement lost after effect")
    );
    assert!(
        old_report.cleanup_failure.is_none(),
        "{:?}",
        old_report.cleanup_failure
    );
    assert!(old_report.fore_deliveries.is_empty());
    let pending = old_report
        .fore_pending_output
        .as_ref()
        .expect("unknown output must survive failure");
    assert_eq!(pending.stage, BodyForeDeliveryStage::AdapterOutcomeUnknown);
    assert_eq!(external.received.len(), 1);
    assert_eq!(pending.delivery.sequence, external.received[0].sequence);
    assert_eq!(
        pending.delivery.front_port_id,
        external.received[0].front_port_id
    );
    assert_eq!(pending.delivery.track, external.received[0].track);
    assert_eq!(pending.delivery.value_kind, external.received[0].value_kind);
    assert_eq!(
        pending.delivery.bytes.as_slice(),
        external.received[0].bytes.as_slice()
    );
    assert_eq!(
        pending.delivery.bytes.as_ptr(),
        external.received[0].bytes.as_ptr(),
        "move the prepared slot; do not allocate a replacement payload"
    );
    assert!(!old_report.kernel_events.iter().any(|event| matches!(
        event.kind,
        conduit_kernel::KernelEventKind::RemoteValueAccepted
            | conduit_kernel::KernelEventKind::RemoteValueDelivered
    )));
    assert_eq!(
        old_report.terminal_sign.active_play_id.as_ref(),
        Some(&old_report.play.active_play_id)
    );
    let metadata = serde_json::to_value(pending.metadata()).unwrap();
    assert_eq!(metadata["stage"], "adapter-outcome-unknown");
    assert_eq!(metadata["byte_length"], pending.delivery.bytes.len());
    assert_eq!(metadata["sha256"].as_str().unwrap().len(), 64);
    let old_evidence = format!("{old_report:?}");
    let old_files = files(&old_root);
    assert_eq!(
        old_files.len(),
        2,
        "the checkpoint publication preceded external acknowledgement loss"
    );

    let (mut replacement_host, replacement_plan, replacement_root) =
        planned_checkpoint_with_version(
            StdHostConfig {
                host_id: "host-a".into(),
                boot_id: "replacement-boot".into(),
                offer_generation: OfferGeneration(2),
            },
            [3; 32],
        );
    let (cold_replacement, cache_replacement) =
        compare_checkpoint_cache(&mut cache, &replacement_host, &replacement_plan);
    assert_eq!(cold_replacement, replacement_plan);
    assert_eq!(cache_replacement.metrics.invalidated_candidates, 1);
    assert_eq!(cache_replacement.metrics.reused_candidates, 0);
    assert_ne!(replacement_plan.plan_id, old_plan_snapshot.plan_id);
    assert_eq!(
        replacement_plan.source_document_id,
        old_plan_snapshot.source_document_id
    );
    assert_eq!(
        replacement_plan.checked_plot_id,
        old_plan_snapshot.checked_plot_id
    );
    assert_eq!(
        replacement_plan.expanded_plot_id,
        old_plan_snapshot.expanded_plot_id
    );
    session
        .mark_current_unsatisfied(old_report.terminal_sign.sign_id.clone())
        .unwrap();
    let replacement_wake = session.wake().clone();
    let plots = vec![BodyPlotPlan {
        plot: ResidentPlot::new(
            replacement_plan.source_document_id.clone(),
            replacement_plan.checked_plot_id.clone(),
        ),
        plan: replacement_plan,
    }];
    let candidate = BodyPlan::seal(&replacement_wake, plots.clone()).unwrap();
    let replacement_play = BodyPlayIdentity::bind(&candidate, 2);
    let advertisement = replacement_host.advertisement();
    let sign = |sequence| {
        bind_sign(
            &advertisement.host_id,
            &advertisement.boot_id,
            Some(&replacement_play.active_play_id),
            sequence,
        )
        .sign_id
    };
    session
        .replan(
            plots,
            BodyPlanningTransition {
                unsatisfied_sign_id: None,
                plan_ready_sign_id: sign(0),
                play_sequence: 2,
                play_started_sign_id: sign(1),
            },
        )
        .unwrap();
    let replacement_body_plan = session.current_plan().clone();
    assert_eq!(replacement_body_plan, candidate);
    assert_eq!(session.plan(&old_body_plan.plan_id), Some(&old_body_plan));
    replacement_host
        .set_initial_kernel_play_sequence(2)
        .unwrap();
    let mut captured = CapturedFore::default();
    // A stale Plan must refuse before effects, rather than rebind old authority.
    assert!(replacement_host
        .run_body_plan_with_todo_checkpoint_to(
            BodyRunRequest {
                wake: &old_wake,
                plan: &old_body_plan,
                control: &RunControl::default(),
                keyboard: None
            },
            BodyForeExchange {
                inputs: &inputs(),
                output: &mut captured
            },
            TodoCheckpointSelection {
                root: &replacement_root,
                identity: checkpoint_identity()
            },
            &mut Vec::new(),
            &mut ThreadTimer,
        )
        .is_err());
    assert!(captured.0.is_empty());
    assert!(files(&replacement_root).is_empty());

    // Explicit bounded provider transfer: preserve the published immutable
    // candidate and selector before new work. This is not output replay or an
    // automatic interpretation of the adapter's unknown acknowledgement.
    copy_checkpoint(&old_files, &replacement_root);
    assert_eq!(files(&replacement_root), old_files);

    // Explicit caller reconciliation chooses a new command against the known
    // checkpointed state. The runner never resubmits the old command/output.
    let mut fresh = inputs();
    fresh[0].bytes = pending.delivery.bytes.as_slice().to_vec();
    fresh[1].bytes = TodoCommand::Add {
        text: "Buy bread".into(),
    }
    .encode_info()
    .unwrap();
    let replacement_report = replacement_host
        .run_body_plan_with_todo_checkpoint_to_with_start(
            BodyRunRequest {
                wake: &replacement_wake,
                plan: &replacement_body_plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            BodyForeExchange {
                inputs: &fresh,
                output: &mut captured,
            },
            TodoCheckpointSelection {
                root: &replacement_root,
                identity: checkpoint_identity(),
            },
            &mut Vec::new(),
            &mut ThreadTimer,
            |play, wake| {
                assert_eq!(play, &replacement_play);
                assert_eq!(wake, session.wake());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(
        replacement_report.terminal,
        TerminalDisposition::Completed,
        "{:?}",
        replacement_report.failure
    );
    assert!(replacement_report.failure.is_none());
    assert!(replacement_report.cleanup_failure.is_none());
    assert!(replacement_report.fore_pending_output.is_none());
    assert_eq!(replacement_report.fore_deliveries.len(), 1);
    assert_ne!(
        old_report.play.active_play_id,
        replacement_report.play.active_play_id
    );
    assert_ne!(
        old_report.terminal_sign.sign_id,
        replacement_report.terminal_sign.sign_id
    );
    assert_ne!(old_report.play.plan_id, replacement_report.play.plan_id);
    assert_eq!(external.received.len(), 1);
    assert_eq!(captured.0.len(), 1);
    let state = TodoState::decode_info(&captured.0[0].bytes).unwrap();
    assert_eq!(state.items.len(), 2);
    assert_eq!(state.items[0].text, "Buy milk");
    assert_eq!(state.items[1].text, "Buy bread");
    assert_eq!(old_body_plan.plots[0].plan, old_plan_snapshot);
    assert_eq!(format!("{old_report:?}"), old_evidence);
    assert_eq!(session.plan(&old_body_plan.plan_id), Some(&old_body_plan));
    assert_eq!(session.snapshot().historical_plan_ids.len(), 2);
    let causal = conduit_std_host::body_causal_evidence::BodyRunCausalRecord::from_run(
        &old_body_plan,
        &old_report,
    )
    .unwrap();
    assert!(causal.retained_evidence().next().is_some());
    assert_eq!(files(&old_root), old_files);
    println!(
        "{}",
        serde_json::json!({
            "case": "uncertain-old-Fore-new-Plan-and-Play",
        "old_body_plan": old_body_plan, "replacement_body_plan": replacement_body_plan,
        "old_pending_output": pending.metadata(), "old_output_bytes": pending.delivery.bytes,
        "replacement_items": state.items.iter().map(|item| item.text.as_str()).collect::<Vec<_>>(),
        "old_report": format!("{old_report:?}"), "replacement_report": format!("{replacement_report:?}"),
        "session": session.snapshot(),
        "cache_old_result": old_plan_snapshot.plan_id.as_str(),
        "cache_replacement_result": cache_replacement.selected_result_identity,
        "cache_replacement_invalidations": cache_replacement.metrics.invalidated_candidates,
            "old_plan": old_report.play.plan_id.as_str(), "old_play": old_report.play.active_play_id.as_str(),
            "old_terminal_sign": old_report.terminal_sign.sign_id.as_str(),
            "old_stage": format!("{:?}", pending.stage), "old_output_sequence": pending.delivery.sequence,
            "replacement_plan": replacement_report.play.plan_id.as_str(),
            "replacement_play": replacement_report.play.active_play_id.as_str(),
            "replacement_terminal_sign": replacement_report.terminal_sign.sign_id.as_str(),
            "external_old_effects": external.received.len(), "replacement_outputs": captured.0.len(),
        })
    );
    std::fs::remove_dir_all(old_root).unwrap();
    std::fs::remove_dir_all(replacement_root).unwrap();
}

fn files(root: &std::path::Path) -> Vec<(std::ffi::OsString, Vec<u8>)> {
    let mut entries = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_owned(),
                std::fs::read(&path).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn copy_checkpoint(source: &[(std::ffi::OsString, Vec<u8>)], root: &std::path::Path) {
    assert_eq!(source.len(), 2);
    for (name, bytes) in source {
        assert!(bytes.len() <= conduit_std_host::todo_durable_resource::CHECKPOINT_MAX_BYTES);
        let path = root.join(name);
        std::fs::write(&path, bytes).unwrap();
        std::fs::File::open(path).unwrap().sync_all().unwrap();
    }
    std::fs::File::open(root).unwrap().sync_all().unwrap();
}

fn compare_checkpoint_cache(
    planner: &mut conduit_planner::IncrementalPlanner,
    host: &conduit_std_host::StdHost,
    expected: &Plan,
) -> (Plan, conduit_planner::IncrementalPlan) {
    use conduit_planner::*;
    use sha2::{Digest, Sha256};
    let key = PlanningFactKey::exact(
        FactDomain::Host,
        "checkpoint/current-Host-and-resource-binding",
    );
    let facts = [PlanningFact {
        key: key.clone(),
        generation: 1,
        content_identity: format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(host.advertisement()).unwrap())
        ),
    }];
    let candidates = [CandidateStructure {
        candidate_id: "checkpoint".into(),
        semantic_contract_id: expected.checked_plot_id.as_str().into(),
        implementation_family_id: "installed-Todo-checkpoint".into(),
        placement_id: "checkpoint/local".into(),
        dependencies: vec![key],
    }];
    let mut latest = None;
    let mut evaluate = |_: &CandidateStructure, fresh: &[PlanningFact]| {
        assert_eq!(fresh, &facts);
        // Reparse/check/expand the actual pinned Source and run the ordinary
        // planner against this fresh Host, including exact resource/authority.
        let plan = checkpoint_plan(host);
        assert!(verify_plan(&plan));
        let result_identity = plan.plan_id.as_str().to_owned();
        latest = Some(plan);
        CandidateEvaluation {
            disposition: CandidateEvaluationDisposition::Admitted,
            result_identity,
            total_cost: 1,
            evaluation_work_units: 1,
        }
    };
    let incremental = planner
        .plan(
            &candidates,
            &facts,
            &StabilityPolicy::disabled(),
            &mut evaluate,
        )
        .unwrap();
    let cold = plan_cold(&candidates, &facts, &StabilityPolicy::disabled(), evaluate).unwrap();
    assert_eq!(
        incremental.selected_result_identity,
        cold.selected_result_identity
    );
    assert_eq!(
        incremental.considered[0].evaluation,
        cold.considered[0].evaluation
    );
    let actual = latest.unwrap();
    assert_eq!(&actual, expected);
    assert_eq!(
        incremental.selected_result_identity,
        actual.plan_id.as_str()
    );
    (actual, incremental)
}
