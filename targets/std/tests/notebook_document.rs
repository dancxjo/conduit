//! The ordinary editor, exposed as a resident Plot Fore. No storage claim.
use conduit_core::*;
use conduit_semantic_catalog::{TextDocument, TextEdit, MAXIMUM_TEXT_EDIT_BYTES};
use conduit_std_host::{BodyLiveForeQueue, RunControl, StdHost};
use std::collections::BTreeMap;

fn planned() -> (StdHost, Plan) {
    let host = StdHost::new();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_state_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(include_str!(
            "../../../plots/notebook/document.conduit"
        )),
        &startup,
    )
    .unwrap();
    let authored =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "notebook/document", &profile)
            .unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let limits = BTreeMap::from([
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("edit"),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 272,
            },
        ),
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("current"),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 264,
            },
        ),
    ]);
    let plan = conduit_planner::plan_expanded_authoring_with_activations(
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
        &limits,
    )
    .unwrap();
    (host, plan)
}

#[test]
fn notebook_exposes_the_existing_editor_and_admits_only_bounded_proposals() {
    let (_, plan) = planned();
    let queue =
        BodyLiveForeQueue::for_text_document_plan(&plan, RunControl::default(), 1, 2).unwrap();
    assert!(queue.submit(&[0; 15]).is_err());
    let mut bytes = [0; MAXIMUM_TEXT_EDIT_BYTES as usize];
    let length = TextEdit {
        base_revision: 0,
        start: 0,
        end: 0,
        insert_text: b"I was here.",
    }
    .encode_into(&mut bytes)
    .unwrap();
    assert!(matches!(
        queue.submit(&bytes[..length]).unwrap(),
        conduit_std_host::BodyLiveForeAdmission::Accepted { sequence: 0 }
    ));
    assert!(matches!(
        queue.submit(&bytes[..length]).unwrap(),
        conduit_std_host::BodyLiveForeAdmission::Full
    ));
    assert_eq!(queue.status().queue_accepted, 1);
    assert!(BodyLiveForeQueue::for_text_document_plan(&plan, RunControl::default(), 1, 0).is_err());
    assert!(
        BodyLiveForeQueue::for_text_document_plan(&plan, RunControl::default(), 1, 65).is_err()
    );
}

#[test]
fn notebook_runs_in_the_body_kernel_and_emits_authoritative_documents() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
            use conduit_std_host::body_execution::{BodyForeOutputAdapter, BodyRunRequest};
            let (mut host, plan) = planned();
            let body = Body::born(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
                1,
                SignId::from("sign/notebook-born"),
            )
            .unwrap();
            let wake = body.wake(1, SignId::from("sign/notebook-wake")).unwrap().1;
            let resident = ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            );
            let control = RunControl::default();
            let queue =
                BodyLiveForeQueue::for_text_document_plan(&plan, control.clone(), 1, 1).unwrap();
            let body_plan = BodyPlan::seal(
                &wake,
                vec![BodyPlotPlan {
                    plot: resident,
                    plan,
                }],
            )
            .unwrap();
            struct Output {
                queue: BodyLiveForeQueue,
                control: RunControl,
                documents: Vec<(u64, Vec<u8>)>,
            }
            impl BodyForeOutputAdapter for Output {
                fn deliver(
                    &mut self,
                    value: &conduit_std_host::ExternalForeDelivery,
                ) -> Result<(), String> {
                    let document = TextDocument::decode(&value.bytes).unwrap();
                    self.documents
                        .push((document.revision, document.text.to_vec()));
                    if document.revision == 0 {
                        let mut bytes = [0; MAXIMUM_TEXT_EDIT_BYTES as usize];
                        let length = TextEdit {
                            base_revision: 0,
                            start: 0,
                            end: 0,
                            insert_text: b"I was here.",
                        }
                        .encode_into(&mut bytes)
                        .unwrap();
                        self.queue.submit(&bytes[..length])?;
                    } else {
                        assert!(self.queue.close().is_err());
                        self.control
                            .request_stop(
                                conduit_std_host::RunControlRequestId::new(
                                    "notebook-session-ended",
                                )
                                .unwrap(),
                            )
                            .unwrap();
                    }
                    Ok(())
                }
            }
            let mut output = Output {
                queue: queue.clone(),
                control: control.clone(),
                documents: Vec::new(),
            };
            let report = host
                .run_body_plan_with_live_fore_to(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &body_plan,
                        control: &control,
                        keyboard: None,
                    },
                    &queue,
                    &mut output,
                    &mut Vec::new(),
                    &mut conduit_std_host::ThreadTimer,
                )
                .unwrap();
            assert!(report.failure.is_none(), "{:?}", report.failure);
            assert!(
                report.cleanup_failure.is_none(),
                "{:?}",
                report.cleanup_failure
            );
            assert!(matches!(
                report.terminal,
                TerminalDisposition::Cancelled { .. }
            ));
            assert_eq!(
                output.documents,
                vec![(0, vec![]), (1, b"I was here.".to_vec())]
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
