//! Same-Body revision adoption through the ordinary pure transition Plot.
use conduit_body::{Body, BodyPlan, BodyPlanningSession, BodyPlotPlan, ResidentPlot};
use conduit_core::*;
use conduit_semantic_catalog::{
    TextDocument, TextEdit, MAXIMUM_TEXT_DOCUMENT_BYTES, MAXIMUM_TEXT_EDIT_BYTES,
};
use conduit_std_host::body_execution::{BodyForeOutputAdapter, BodyRunReport, BodyRunRequest};
use conduit_std_host::{ExternalForeDelivery, ExternalForeInput, RunControl, StdHost, ThreadTimer};
use std::collections::BTreeMap;

fn plan(host: &StdHost) -> Plan {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_state_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(include_str!("../../../plots/notebook/main.conduit")),
        &startup,
    )
    .unwrap();
    let authored = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "notebook/transition",
        &profile,
    )
    .unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let mut boundaries = BTreeMap::new();
    for (direction, name, bytes) in [
        (PortDirection::Input, "document", 264),
        (PortDirection::Input, "edit", 272),
        (PortDirection::Output, "current", 264),
    ] {
        boundaries.insert(
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port_id(name),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: bytes,
            },
        );
    }
    conduit_planner::plan_expanded_authoring_with_activations(
        &checked,
        &authored,
        &profile,
        &conduit_plot::CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 272,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap()
}
fn resident(plan: &Plan) -> BodyPlotPlan {
    BodyPlotPlan {
        plot: ResidentPlot::new(
            plan.source_document_id.clone(),
            plan.checked_plot_id.clone(),
        ),
        plan: plan.clone(),
    }
}
fn inputs(document: &[u8], revision: u64, insertion: &[u8]) -> Vec<ExternalForeInput> {
    let current = TextDocument::decode(document).unwrap();
    let end = core::str::from_utf8(current.text).unwrap().chars().count() as u32;
    let mut edit = [0; MAXIMUM_TEXT_EDIT_BYTES as usize];
    let length = TextEdit {
        base_revision: revision,
        start: end,
        end,
        insert_text: insertion,
    }
    .encode_into(&mut edit)
    .unwrap();
    vec![
        ExternalForeInput {
            front_port_id: port_id("document"),
            track: ConnectionTrack::Payload,
            bytes: document.to_vec(),
        },
        ExternalForeInput {
            front_port_id: port_id("edit"),
            track: ConnectionTrack::Payload,
            bytes: edit[..length].to_vec(),
        },
    ]
}
struct Output(Vec<ExternalForeDelivery>);
impl BodyForeOutputAdapter for Output {
    fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value.clone());
        Ok(())
    }
}
fn play(
    host: &mut StdHost,
    session: &mut BodyPlanningSession,
    inputs: &[ExternalForeInput],
) -> (BodyRunReport, Vec<ExternalForeDelivery>) {
    let wake = session.wake().clone();
    let body_plan: BodyPlan = session.current_plan().clone();
    let claim = session
        .claim_execution(
            &body_plan.plan_id,
            &host.advertisement().host_id,
            &host.advertisement().boot_id,
        )
        .unwrap();
    let mut output = Output(Vec::with_capacity(1));
    let report = host
        .run_body_plan_with_fore_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            inputs,
            false,
            &mut output,
            &mut Vec::new(),
            &mut ThreadTimer,
            |play, wake| {
                assert_eq!(play, &claim.play);
                session
                    .report_execution_started(play, wake)
                    .map_err(|error| format!("{error:?}"))
            },
        )
        .unwrap();
    assert!(
        report.cleanup_failure.is_none(),
        "{:?}",
        report.cleanup_failure
    );
    let disposition = match report.terminal {
        TerminalDisposition::Completed => "completed",
        TerminalDisposition::Cancelled { .. } => "cancelled",
        TerminalDisposition::Failed { .. } => "failed",
    };
    session
        .report_execution_terminal(&report.play, disposition, &report.terminal_sign.sign_id)
        .unwrap();
    let sequence = report.play.play_sequence;
    session
        .lull(
            format!("sign/notebook/lull/{sequence}").into(),
            format!("sign/notebook/retained/{sequence}").into(),
        )
        .unwrap();
    (report, output.0)
}

#[test]
fn provided_revision_advances_and_stale_action_preserves_the_same_body_document() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut host = StdHost::new();
            host.set_initial_kernel_play_sequence(1).unwrap();
            let plan = plan(&host);
            let body = Body::born(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
                1,
                "sign/notebook/born".into(),
            )
            .unwrap();
            let id = body.body_id.clone();
            let mut session = BodyPlanningSession::prepare(
                &body,
                1,
                "sign/notebook/wake/1".into(),
                vec![resident(&plan)],
            )
            .unwrap();
            let mut document = [0; MAXIMUM_TEXT_DOCUMENT_BYTES as usize];
            let length = TextDocument {
                revision: 41,
                text: b"I was here.",
            }
            .encode_into(&mut document)
            .unwrap();
            let initial = document;
            let (first, output) = play(
                &mut host,
                &mut session,
                &inputs(&document[..length], 41, b" Again."),
            );
            assert_eq!(first.terminal, TerminalDisposition::Completed);
            assert_eq!(first.play.body_id, id);
            assert_eq!(document, initial);
            assert_eq!(output.len(), 1);
            let current = output[0].bytes.to_vec();
            assert_eq!(
                TextDocument::decode(&current).unwrap(),
                TextDocument {
                    revision: 42,
                    text: b"I was here. Again."
                }
            );
            session
                .prepare_next_wake(
                    &session.body().clone(),
                    2,
                    "sign/notebook/wake/2".into(),
                    vec![resident(&plan)],
                )
                .unwrap();
            let (stale, refused) = play(&mut host, &mut session, &inputs(&current, 41, b" stale"));
            assert!(matches!(stale.terminal, TerminalDisposition::Failed { .. }));
            assert_eq!(
                stale.kernel_failure,
                Some(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::InvalidInput,
                    detail: 2
                })
            );
            assert!(refused.is_empty());
            assert_eq!(stale.play.body_id, id);
            session
                .prepare_next_wake(
                    &session.body().clone(),
                    3,
                    "sign/notebook/wake/3".into(),
                    vec![resident(&plan)],
                )
                .unwrap();
            let (next, output) = play(
                &mut host,
                &mut session,
                &inputs(&current, 42, b" Continued."),
            );
            assert_eq!(next.terminal, TerminalDisposition::Completed);
            assert_eq!(next.play.body_id, id);
            assert_eq!(
                TextDocument::decode(&output[0].bytes).unwrap(),
                TextDocument {
                    revision: 43,
                    text: b"I was here. Again. Continued."
                }
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn waiting_document_step_admits_one_proposal_after_the_actual_body_start() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            use conduit_std_host::{BodyLiveForeAdmission, BodyLiveForeQueue};
            let mut host = StdHost::new();
            let plan = plan(&host);
            let body = Body::born(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
                1,
                "sign/notebook/waiting-born".into(),
            )
            .unwrap();
            let wake = body.wake(1, "sign/notebook/waiting-wake".into()).unwrap().1;
            let body_plan = BodyPlan::seal(&wake, vec![resident(&plan)]).unwrap();
            let mut document = [0; MAXIMUM_TEXT_DOCUMENT_BYTES as usize];
            let length = TextDocument {
                revision: 7,
                text: b"I was here.",
            }
            .encode_into(&mut document)
            .unwrap();
            let proposed = inputs(&document[..length], 7, b" Again.").pop().unwrap();
            let control = RunControl::default();
            let queue = BodyLiveForeQueue::for_document_step_plan(
                &plan,
                control.clone(),
                document[..length].to_vec(),
                std::time::Duration::from_secs(2),
            )
            .unwrap();
            let mut output = Output(Vec::with_capacity(1));
            let mut starts = 0;
            let before_start = proposed.bytes.clone();
            let producer_queue = queue.clone();
            let producer = std::thread::spawn(move || {
                producer_queue.wait_until_play_started().unwrap();
                assert!(matches!(
                    producer_queue.submit(&proposed.bytes).unwrap(),
                    BodyLiveForeAdmission::Accepted { sequence: 0 }
                ));
                assert!(producer_queue.submit(&proposed.bytes).is_err());
            });
            let report = host
                .run_body_plan_with_live_fore_to_with_start(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &body_plan,
                        control: &control,
                        keyboard: None,
                    },
                    &queue,
                    &mut output,
                    &mut Vec::new(),
                    &mut ThreadTimer,
                    |play, actual_wake| {
                        starts += 1;
                        assert_eq!(play.body_id, body.body_id);
                        assert_eq!(actual_wake.wake_id, wake.wake_id);
                        // Admission is intentionally still closed while the owner
                        // retains this genuine start. It opens after callback success.
                        assert!(queue
                            .submit(&before_start)
                            .unwrap_err()
                            .contains("started Body Play"));
                        Ok(())
                    },
                )
                .unwrap();
            assert_eq!(starts, 1);
            producer.join().unwrap();
            assert_eq!(report.terminal, TerminalDisposition::Completed);
            assert!(report.failure.is_none(), "{:?}", report.failure);
            assert!(
                report.cleanup_failure.is_none(),
                "{:?}",
                report.cleanup_failure
            );
            assert_eq!(output.0.len(), 1);
            assert_eq!(
                TextDocument::decode(&output.0[0].bytes).unwrap(),
                TextDocument {
                    revision: 8,
                    text: b"I was here. Again."
                }
            );
            assert_eq!(queue.status().queue_accepted, 1);
        })
        .unwrap()
        .join()
        .unwrap();
}
