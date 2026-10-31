//! Running-owner refusals required before introducing a continuing successor.
use super::{fixture, BodyLifecycleSessionError, ResidentPlot};

// The continuing worker bridge must gain a real successor transaction. Ordinary
// proposal APIs cannot be used to replace the owner underneath a running Play.
#[test]
fn started_sealed_owner_refuses_workset_extension_without_retiring_its_play() {
    use conduit_body::{BodyPlayIdentity, SealedBodyLifecycleSession, VerifiedBodyPlotPlan};
    let (rich, partition, host, boot) = fixture();
    let mut owner =
        SealedBodyLifecycleSession::open_admitted(rich.evidence().clone(), &host, &boot).unwrap();
    let verified =
        conduit_core::verify_plan_metadata_with_configurations(partition.plan.clone(), |_| {
            Ok::<_, ()>(vec![])
        })
        .unwrap();
    let input = [VerifiedBodyPlotPlan {
        plot: partition.plot.clone(),
        plan: &verified,
    }];
    owner
        .propose_verified(&input, &[], None, &host, &boot)
        .unwrap();
    let realization = owner.realization().unwrap();
    let play = BodyPlayIdentity::bind(&realization.plan, 1);
    let started = realization
        .wake
        .body_plan_ready(
            &realization.plan,
            conduit_core::bind_sign(&host, &boot, Some(&play.active_play_id), 0).sign_id,
        )
        .unwrap()
        .body_play_started(
            &realization.plan,
            &play,
            conduit_core::bind_sign(&host, &boot, Some(&play.active_play_id), 1).sign_id,
        )
        .unwrap();
    owner
        .started(&host, &boot, play.clone(), started.clone())
        .unwrap();
    let evidence = owner.evidence().clone();
    let realization = owner.realization().cloned();
    let archives = owner.pending_archives().to_vec();
    let serialized = postcard::to_allocvec(owner.evidence()).unwrap();
    let extra = ResidentPlot::new("source/worker".into(), "checked/worker".into());
    assert_eq!(
        owner.admit_plot(evidence.body.workload_revision, extra, &host, &boot),
        Err(BodyLifecycleSessionError::NotLulled)
    );
    assert!(matches!(
        owner.prepare_verified_proposal_context(&host, &boot),
        Err(BodyLifecycleSessionError::NotLulled)
    ));
    assert_eq!(
        owner.propose_verified(&input, &[], None, &host, &boot),
        Err(BodyLifecycleSessionError::NotLulled)
    );
    assert_eq!(
        owner.started(&host, &boot, play.clone(), started),
        Err(BodyLifecycleSessionError::AlreadyPlaying)
    );
    assert_eq!(owner.evidence(), &evidence);
    assert_eq!(postcard::to_allocvec(owner.evidence()).unwrap(), serialized);
    assert_eq!(owner.realization(), realization.as_ref());
    assert_eq!(owner.pending_archives(), archives);
    assert!(owner
        .evidence()
        .startup_for_play(&owner.realization().unwrap().plan, &play)
        .is_ok());
}
