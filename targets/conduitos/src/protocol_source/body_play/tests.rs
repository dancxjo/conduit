use super::*;
use conduit_body::BodyPlotPlan;
use conduit_core::{PlotIdentity, seal_plan};

#[test]
fn protocol_profile_requires_the_exact_complete_current_proposal() {
    let host = HostId::from("fixture/protocol");
    let boot = BootId::from("fixture/protocol-boot");
    let identity = PlotIdentity {
        source_document_id: "fixture/source".into(),
        checked_plot_id: "fixture/checked".into(),
        expanded_plot_id: "fixture/expanded".into(),
    };
    let partition = BodyPlotPlan {
        plot: conduit_body::ResidentPlot::new(
            identity.source_document_id.clone(),
            identity.checked_plot_id.clone(),
        ),
        plan: seal_plan(identity, alloc::vec![]),
    };
    let session =
        crate::protocol_test_support::protocol_body_session(partition.clone(), &host, &boot);
    validate_workset(&session, &partition).unwrap();
    let mut foreign = partition.clone();
    foreign.plot.checked_plot_id = "foreign/checked".into();
    assert!(matches!(
        validate_workset(&session, &foreign),
        Err(ProtocolBodyRefusal::UnsupportedWorkset)
    ));
    let mut no_proposal = session.clone();
    no_proposal.lull(&host, &boot, None).unwrap();
    assert!(matches!(
        validate_workset(&no_proposal, &partition),
        Err(ProtocolBodyRefusal::Lifecycle(
            BodyLifecycleSessionError::NoProposal
        ))
    ));
    let other = conduit_body::ResidentPlot::new(
        "fixture/other-source".into(),
        "fixture/other-checked".into(),
    );
    no_proposal
        .admit_plot(
            no_proposal.evidence().body.workload_revision,
            other.clone(),
            &host,
            &boot,
        )
        .unwrap();
    let other_plan = seal_plan(
        PlotIdentity {
            source_document_id: other.source_document_id.clone(),
            checked_plot_id: other.checked_plot_id.clone(),
            expanded_plot_id: "fixture/other-expanded".into(),
        },
        alloc::vec![],
    );
    no_proposal
        .propose(
            alloc::vec![
                partition.clone(),
                BodyPlotPlan {
                    plot: other,
                    plan: other_plan
                }
            ],
            &host,
            &boot,
        )
        .unwrap();
    assert!(matches!(
        validate_workset(&no_proposal, &partition),
        Err(ProtocolBodyRefusal::UnsupportedWorkset)
    ));
    // A canonical active Play is not a fresh preparation entrance.
    let mut active = session.clone();
    let current = active.realization().unwrap();
    let play = BodyPlayIdentity::bind(&current.plan, 0);
    let sign = |sequence| bind_sign(&host, &boot, Some(&play.active_play_id), sequence).sign_id;
    let wake = current
        .wake
        .body_plan_ready(&current.plan, sign(0))
        .unwrap()
        .body_play_started(&current.plan, &play, sign(1))
        .unwrap();
    active.started(&host, &boot, play, wake).unwrap();
    assert!(matches!(
        validate_workset(&active, &partition),
        Err(ProtocolBodyRefusal::Lifecycle(
            BodyLifecycleSessionError::AlreadyPlaying
        ))
    ));
}

#[test]
fn acknowledged_cancellation_binds_original_body_and_artifact_and_is_one_shot() {
    use crate::protocol_operations::tests::automatic_events::{InertBus, InertClock, prepare};
    let (mut play, _, _) = prepare(InertBus, InertClock);
    play.start().unwrap();
    let original = play.retirement_identity.as_ref().unwrap().clone();
    let receipt = play.cancel_acknowledged().unwrap();
    assert_eq!(receipt.artifact(), &original.0);
    assert_eq!(receipt.play(), &original.1);
    assert!(receipt.pending().iter().all(Option::is_none));
    assert!(play.kernel().is_none());
    assert!(play.session().realization().is_none());
    assert!(play.cancel_acknowledged().is_err());
}

#[test]
fn stale_play_identity_refuses_before_owner_revoke_or_kernel_cancel() {
    use crate::protocol_operations::tests::automatic_events::{InertBus, InertClock, prepare};
    let (mut play, _, _) = prepare(InertBus, InertClock);
    play.start().unwrap();
    let original = play.retirement_identity.as_ref().unwrap().clone();
    play.retirement_identity.as_mut().unwrap().1.active_play_id = "foreign/play".into();
    let refused = play.cancel_acknowledged().unwrap_err();
    assert_eq!(refused.stage, CancellationStage::Identity);
    assert!(play.kernel().is_some());
    assert!(play.session().realization().unwrap().play.is_some());
    // Same actual owners remain usable for valid cancellation after the stale
    // proposed identity is removed; no cancellation authority was consumed.
    play.retirement_identity = Some(original);
    assert!(play.cancel_acknowledged().is_ok());
}

#[test]
fn lull_refusal_does_not_mint_or_discard_retained_cancelled_kernel() {
    use crate::protocol_operations::tests::automatic_events::{InertBus, InertClock, prepare};
    let (mut play, _, _) = prepare(InertBus, InertClock);
    play.start().unwrap();
    let original_host = play.host.clone();
    play.host = "foreign/host".into();
    let refused = play.cancel_acknowledged().unwrap_err();
    assert_eq!(refused.stage, CancellationStage::BodyLull);
    assert!(play.kernel().is_some());
    assert!(play.retirement_identity.is_some());
    play.host = original_host;
    assert!(play.cancel_acknowledged().is_ok());
}
