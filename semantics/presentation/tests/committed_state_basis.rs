use conduit_body::Body;
use conduit_core::{
    ActivePlayId, CheckedPlotId, PlanId, ResourceSemanticIdentity, ResourceVersionIdentity, SignId,
    SourceDocumentId,
};
use conduit_presentation::{
    CommittedStateBasisRefusal as Refusal, CommittedStateContributionBasis,
    CommittedStateOperation, CommittedStateSelection,
};

fn fixture() -> (Body, CommittedStateContributionBasis) {
    let body = Body::born(
        SourceDocumentId::from("source/todo"),
        CheckedPlotId::from("checked/todo"),
        1,
        SignId::from("sign/birth"),
    )
    .unwrap();
    let basis = CommittedStateContributionBasis {
        body_id: body.body_id.clone(),
        checked_plot_id: CheckedPlotId::from("checked/todo"),
        selection: CommittedStateSelection {
            resource: ResourceSemanticIdentity::from_digest([1; 32]),
            selected_version: ResourceVersionIdentity::from_digest([2; 32]),
            published_version: ResourceVersionIdentity::from_digest([3; 32]),
        },
        write: CommittedStateOperation {
            plan_id: PlanId::from("plan/write"),
            play_id: ActivePlayId::from("play/write"),
            terminal_sign_id: SignId::from("sign/write"),
        },
        read: CommittedStateOperation {
            plan_id: PlanId::from("plan/read"),
            play_id: ActivePlayId::from("play/read"),
            terminal_sign_id: SignId::from("sign/read"),
        },
        state_digest: [4; 32],
    };
    (body, basis)
}

#[test]
fn exact_lulled_shape_is_distinct_from_current_play_and_not_face_admission() {
    let (body, basis) = fixture();
    assert_eq!(
        basis.validate_shape_against(&body, &basis.selection),
        Ok(())
    );
}

#[test]
fn changed_residence_and_generation_have_distinct_refusals() {
    let (body, basis) = fixture();
    let other_body = Body::born(
        SourceDocumentId::from("source/other"),
        CheckedPlotId::from("checked/other"),
        1,
        SignId::from("sign/other-birth"),
    )
    .unwrap();
    assert_eq!(
        basis.validate_shape_against(&other_body, &basis.selection),
        Err(Refusal::BodyChanged)
    );
    let mut stale_plot = basis.clone();
    stale_plot.checked_plot_id = CheckedPlotId::from("checked/old");
    assert_eq!(
        stale_plot.validate_shape_against(&body, &stale_plot.selection),
        Err(Refusal::PlotNotResident)
    );
    let mut selection = basis.selection.clone();
    selection.resource = ResourceSemanticIdentity::from_digest([5; 32]);
    assert_eq!(
        basis.validate_shape_against(&body, &selection),
        Err(Refusal::CheckpointSelectionChanged)
    );
    let mut selection = basis.selection.clone();
    selection.published_version = ResourceVersionIdentity::from_digest([6; 32]);
    assert_eq!(
        basis.validate_shape_against(&body, &selection),
        Err(Refusal::PublishedVersionChanged)
    );
}

#[test]
fn zero_digest_and_reused_read_are_refused() {
    let (body, basis) = fixture();
    let mut zero = basis.clone();
    zero.state_digest = [0; 32];
    assert_eq!(
        zero.validate_shape_against(&body, &zero.selection),
        Err(Refusal::MissingStateDigest)
    );
    let mut reused = basis.clone();
    reused.read = reused.write.clone();
    assert_eq!(
        reused.validate_shape_against(&body, &reused.selection),
        Err(Refusal::ReusedOperation)
    );
    let mut missing_sign = basis.clone();
    missing_sign.read.terminal_sign_id = SignId::from("");
    assert_eq!(
        missing_sign.validate_shape_against(&body, &missing_sign.selection),
        Err(Refusal::InvalidOperationIdentity)
    );
    let mut oversized_id = basis.clone();
    oversized_id.read.plan_id = PlanId::from("x".repeat(129));
    assert_eq!(
        oversized_id.validate_shape_against(&body, &oversized_id.selection),
        Err(Refusal::InvalidOperationIdentity)
    );
}
