//! Durable Text effects use the same Body Play and lifecycle admission as edits.
use super::*;
use conduit_body::{Body, BodyPlanningSession, BodyPlotPlan, ResidentPlot};
use conduit_std_host::body_execution::{BodyForeOutputAdapter, BodyRunRequest};

impl BodyForeOutputAdapter for Output {
    fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value.clone());
        Ok(())
    }
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
fn run(
    host: &mut conduit_std_host::StdHost,
    session: &mut BodyPlanningSession,
    input: ExternalForeInput,
) -> Output {
    let wake = session.wake().clone();
    let plan = session.current_plan().clone();
    let claim = session
        .claim_execution(
            &plan.plan_id,
            &host.advertisement().host_id,
            &host.advertisement().boot_id,
        )
        .unwrap();
    let mut output = Output::default();
    let report = host
        .run_body_plan_with_fore_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &conduit_std_host::RunControl::default(),
                keyboard: None,
            },
            &[input],
            false,
            &mut output,
            &mut Vec::new(),
            &mut conduit_std_host::ThreadTimer,
            |play, wake| {
                assert_eq!(play, &claim.play);
                session
                    .report_execution_started(play, wake)
                    .map_err(|error| format!("{error:?}"))
            },
        )
        .unwrap();
    assert_eq!(
        report.terminal,
        TerminalDisposition::Completed,
        "{:?}",
        report.failure
    );
    assert!(
        report.cleanup_failure.is_none(),
        "{:?}",
        report.cleanup_failure
    );
    session
        .report_execution_terminal(&report.play, "completed", &report.terminal_sign.sign_id)
        .unwrap();
    let sequence = report.play.play_sequence;
    session
        .lull(
            format!("sign/file/lull/{sequence}").into(),
            format!("sign/file/retained/{sequence}").into(),
        )
        .unwrap();
    assert_eq!(report.play.body_id, session.body().body_id);
    output
}

#[test]
fn one_body_publishes_then_reads_the_selected_text_under_fresh_boot_authority() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let root = tempfile::tempdir().unwrap();
            let mut writer = fixture::host(root.path(), "boot/one", true);
            writer.set_initial_kernel_play_sequence(1).unwrap();
            let write = fixture::plan(&writer, true);
            let body = Body::born(
                write.source_document_id.clone(),
                write.checked_plot_id.clone(),
                1,
                "sign/file/born".into(),
            )
            .unwrap();
            let id = body.body_id.clone();
            let mut session = BodyPlanningSession::prepare(
                &body,
                1,
                "sign/file/wake/1".into(),
                vec![resident(&write)],
            )
            .unwrap();
            let saved = run(
                &mut writer,
                &mut session,
                ExternalForeInput {
                    front_port_id: port_id("value"),
                    track: ConnectionTrack::Payload,
                    bytes: b"I was here.".to_vec(),
                },
            );
            assert_eq!(saved.0.len(), 1);
            assert_eq!(saved.0[0].front_port_id.as_str(), "saved");
            let reference = saved.0[0].bytes.to_vec();
            drop(writer);
            let mut reader = fixture::host(root.path(), "boot/two", false);
            reader.set_initial_kernel_play_sequence(2).unwrap();
            let read = fixture::plan(&reader, false);
            let changed = session
                .body()
                .admit_plot(resident(&read).plot, "sign/file/admit-read".into())
                .unwrap()
                .remove_plot(&resident(&write).plot, "sign/file/remove-write".into())
                .unwrap();
            session
                .prepare_next_wake(
                    &changed,
                    2,
                    "sign/file/wake/2".into(),
                    vec![resident(&read)],
                )
                .unwrap();
            let loaded = run(
                &mut reader,
                &mut session,
                ExternalForeInput {
                    front_port_id: port_id("selected"),
                    track: ConnectionTrack::Payload,
                    bytes: reference,
                },
            );
            assert_eq!(loaded.0.len(), 1);
            assert_eq!(loaded.0[0].front_port_id.as_str(), "value");
            assert_eq!(loaded.0[0].bytes.as_slice(), b"I was here.");
            assert_eq!(session.body().body_id, id);
        })
        .unwrap()
        .join()
        .unwrap();
}
