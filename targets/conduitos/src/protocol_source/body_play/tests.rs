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
