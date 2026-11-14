//! CPU authority fixtures and real file effects; no remote admission/Start claim.
use super::*;
use conduit_body::{Body, BodyPlan, BodyPlayIdentity, BodyPlotPlan, ResidentPlot, Wake};
use conduit_std_host::body_execution::BodyRunRequest;
fn resident(plan: Plan) -> BodyPlotPlan {
    BodyPlotPlan {
        plot: ResidentPlot::new(
            plan.source_document_id.clone(),
            plan.checked_plot_id.clone(),
        ),
        plan,
    }
}
fn seal(parts: Vec<BodyPlotPlan>) -> (Wake, BodyPlan) {
    let first = &parts[0].plot;
    let mut body = Body::born(
        first.source_document_id.clone(),
        first.checked_plot_id.clone(),
        1,
        "sign/issuer/born".into(),
    )
    .unwrap();
    for (i, p) in parts.iter().enumerate().skip(1) {
        body = body
            .admit_plot(p.plot.clone(), format!("sign/issuer/admit/{i}").into())
            .unwrap();
    }
    let (_, wake) = body.wake(1, "sign/issuer/wake".into()).unwrap();
    let plan = BodyPlan::seal(&wake, parts).unwrap();
    (wake, plan)
}
fn input() -> ExternalForeInput {
    ExternalForeInput {
        front_port_id: port_id("value"),
        track: ConnectionTrack::Payload,
        bytes: b"I was here.".to_vec(),
    }
}
#[test]
fn original_issuer_records_its_partition_without_starting_or_settling_the_body() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let root = tempfile::tempdir().unwrap();
            let remote_root = tempfile::tempdir().unwrap();
            let mut host = fixture::host(root.path(), "boot/issuer", true);
            host.set_initial_kernel_play_sequence(1).unwrap();
            let local = fixture::plan(&host, true);
            let local_id = local.plan_id.clone();
            let remote_host = conduit_std_host::StdHost::new_with_text_file(
                conduit_std_host::StdHostConfig {
                    host_id: "remote/mcu-fixture".into(),
                    boot_id: "boot/remote".into(),
                    offer_generation: OfferGeneration(1),
                },
                std::fs::File::open(remote_root.path()).unwrap(),
                fixture::reference(),
                true,
            )
            .unwrap();
            let remote = fixture::named_plan(&remote_host, "remote/recording");
            let remote_id = remote.plan_id.clone();
            let (wake, plan) = seal(vec![resident(local), resident(remote)]);
            let before_wake = wake.clone();
            let before_plan = plan.clone();
            let issued = host.prospective_body_play(&wake, &plan).unwrap();
            let control = conduit_std_host::RunControl::default();
            let request = || BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            };
            // Existing whole-local entrance must continue refusing foreign partitions.
            assert!(host
                .run_body_plan_with_fore_to_with_start(
                    request(),
                    &[input()],
                    false,
                    &mut Output::default(),
                    &mut Vec::new(),
                    &mut conduit_std_host::ThreadTimer,
                    |_, _| panic!("not all local")
                )
                .is_err());
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
            let mut output = Output::default();
            let mut starts = 0;
            let mut local_start = None;
            let report = host
                .run_issuing_host_plan_with_fore_to_with_start(
                    request(),
                    &issued,
                    &[input()],
                    &mut output,
                    &mut Vec::new(),
                    &mut conduit_std_host::ThreadTimer,
                    |play, sign| {
                        starts += 1;
                        local_start = Some(sign.clone());
                        assert_eq!(play, &issued);
                        assert_eq!(sign.active_play_id.as_ref(), Some(&issued.active_play_id));
                        Ok(())
                    },
                )
                .unwrap();
            assert_eq!(starts, 1);
            assert_eq!(report.play(), &issued);
            assert_eq!(&report.local_execution().wake_at_start, &before_wake);
            for ordinal in 0..=2 {
                let global = bind_sign(
                    &host.advertisement().host_id,
                    &host.advertisement().boot_id,
                    Some(&issued.active_play_id),
                    ordinal,
                );
                assert_ne!(report.local_terminal_sign().sign_id, global.sign_id);
                assert_ne!(local_start.as_ref().unwrap().sign_id, global.sign_id);
            }
            assert_ne!(
                local_start.unwrap().sign_id,
                report.local_terminal_sign().sign_id
            );
            assert_eq!(report.local_terminal(), &TerminalDisposition::Completed);
            assert!(report.failure().is_none() && report.cleanup_failure().is_none());
            assert_eq!(report.local_partitions().len(), 1);
            assert_eq!(report.local_partitions()[0].plan_id, local_id);
            assert_ne!(report.local_partitions()[0].plan_id, remote_id);
            assert_eq!(output.0.len(), 1);
            assert_eq!(report.fore_deliveries().len(), 1);
            assert_eq!(
                conduit_data::DataReference::decode_for(&kind_id("value/text"), &output.0[0].bytes)
                    .unwrap()
                    .reference(),
                &fixture::reference()
            );
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
            assert_eq!(std::fs::read_dir(remote_root.path()).unwrap().count(), 0);
            assert_eq!(wake, before_wake);
            assert_eq!(plan, before_plan);
            assert_eq!(wake.lifecycle, conduit_body::WakeLifecycle::AwaitingPlan);
            assert!(host
                .run_issuing_host_plan_with_fore_to_with_start(
                    request(),
                    &issued,
                    &[input()],
                    &mut Output::default(),
                    &mut Vec::new(),
                    &mut conduit_std_host::ThreadTimer,
                    |_, _| panic!("replay")
                )
                .is_err());
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn all_local_partitions_reserve_atomically_before_any_effect_or_cursor_consumption() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let root = tempfile::tempdir().unwrap();
            let mut host = fixture::host(root.path(), "boot/local", true);
            host.set_initial_kernel_play_sequence(1).unwrap();
            let local = fixture::plan(&host, true);
            let second = fixture::named_plan(&host, "second/local-publication");
            let (wake, plan) = seal(vec![resident(local), resident(second)]);
            let issued = host.prospective_body_play(&wake, &plan).unwrap();
            let control = conduit_std_host::RunControl::default();
            assert!(host
                .run_issuing_host_plan_with_fore_to_with_start(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &plan,
                        control: &control,
                        keyboard: None
                    },
                    &issued,
                    &[input()],
                    &mut Output::default(),
                    &mut Vec::new(),
                    &mut conduit_std_host::ThreadTimer,
                    |_, _| panic!(
                        "one offered file instance cannot fund two concurrent operations"
                    )
                )
                .is_err());
            assert_eq!(host.prospective_body_play(&wake, &plan).unwrap(), issued);
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn foreign_issued_sequence_and_stale_local_boot_refuse_without_effects() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let root = tempfile::tempdir().unwrap();
            let mut host = fixture::host(root.path(), "boot/current", true);
            host.set_initial_kernel_play_sequence(1).unwrap();
            let old = fixture::host(root.path(), "boot/old", true);
            let control = conduit_std_host::RunControl::default();
            for plan in [fixture::plan(&host, true), fixture::plan(&old, true)] {
                let stale = plan.fragments[0].boot_id != host.advertisement().boot_id;
                let (wake, plan) = seal(vec![resident(plan)]);
                let actual = host.prospective_body_play(&wake, &plan).unwrap();
                let offered = if stale {
                    actual.clone()
                } else {
                    BodyPlayIdentity::bind(&plan, actual.play_sequence + 1)
                };
                assert!(host
                    .run_issuing_host_plan_with_fore_to_with_start(
                        BodyRunRequest {
                            wake: &wake,
                            plan: &plan,
                            control: &control,
                            keyboard: None
                        },
                        &offered,
                        &[input()],
                        &mut Output::default(),
                        &mut Vec::new(),
                        &mut conduit_std_host::ThreadTimer,
                        |_, _| panic!("stale")
                    )
                    .is_err());
                assert_eq!(host.prospective_body_play(&wake, &plan).unwrap(), actual);
                assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn failed_local_start_retention_writes_nothing_and_cannot_replay_the_issued_play() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let root = tempfile::tempdir().unwrap();
            let mut host = fixture::host(root.path(), "boot/callback-refusal", true);
            host.set_initial_kernel_play_sequence(1).unwrap();
            let (wake, plan) = seal(vec![resident(fixture::plan(&host, true))]);
            let issued = host.prospective_body_play(&wake, &plan).unwrap();
            let control = conduit_std_host::RunControl::default();
            let request = || BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            };
            let mut calls = 0;
            assert!(host
                .run_issuing_host_plan_with_fore_to_with_start(
                    request(),
                    &issued,
                    &[input()],
                    &mut Output::default(),
                    &mut Vec::new(),
                    &mut conduit_std_host::ThreadTimer,
                    |_, _| {
                        calls += 1;
                        Err("retention outcome uncertain".into())
                    }
                )
                .is_err());
            assert_eq!(calls, 1);
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
            assert_ne!(host.prospective_body_play(&wake, &plan).unwrap(), issued);
            assert!(host
                .run_issuing_host_plan_with_fore_to_with_start(
                    request(),
                    &issued,
                    &[input()],
                    &mut Output::default(),
                    &mut Vec::new(),
                    &mut conduit_std_host::ThreadTimer,
                    |_, _| panic!("must not retry")
                )
                .is_err());
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn published_file_with_refused_fore_delivery_is_not_acknowledged_or_replayed() {
    struct Refuse;
    impl conduit_std_host::body_execution::BodyForeOutputAdapter for Refuse {
        fn deliver(&mut self, _: &ExternalForeDelivery) -> Result<(), String> {
            Err("consumer did not retain result".into())
        }
    }
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let root = tempfile::tempdir().unwrap();
            let mut host = fixture::host(root.path(), "boot/refused-output", true);
            host.set_initial_kernel_play_sequence(1).unwrap();
            let (wake, plan) = seal(vec![resident(fixture::plan(&host, true))]);
            let issued = host.prospective_body_play(&wake, &plan).unwrap();
            let control = conduit_std_host::RunControl::default();
            let request = || BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            };
            let report = host
                .run_issuing_host_plan_with_fore_to_with_start(
                    request(),
                    &issued,
                    &[input()],
                    &mut Refuse,
                    &mut Vec::new(),
                    &mut conduit_std_host::ThreadTimer,
                    |_, _| Ok(()),
                )
                .unwrap();
            assert_eq!(
                std::fs::read_dir(root.path()).unwrap().count(),
                1,
                "the file effect may already be published"
            );
            assert!(report.fore_deliveries().is_empty());
            assert!(report.local_execution().fore_pending_output.is_some());
            assert!(
                report.failure().is_some()
                    || report.local_terminal() != &TerminalDisposition::Completed
            );
            assert_eq!(report.local_execution().wake_at_start, wake);
            assert!(host
                .run_issuing_host_plan_with_fore_to_with_start(
                    request(),
                    &issued,
                    &[input()],
                    &mut Output::default(),
                    &mut Vec::new(),
                    &mut conduit_std_host::ThreadTimer,
                    |_, _| panic!("must not repeat published effect")
                )
                .is_err());
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
        })
        .unwrap()
        .join()
        .unwrap();
}
