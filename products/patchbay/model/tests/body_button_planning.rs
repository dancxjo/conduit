use conduit_body::{BodyWorkset, ResidentForm};
use conduit_core::{resource_offer, SignId, INPUT_RESOURCE_CLASS};
use patchbay_model::{plan_body_workset_on_host, FormCandidate};

#[test]
fn canonical_button_body_planning_respects_both_selected_queue_limits() {
    let source = include_str!("../../../../forms/button-across-room/main.conduit");
    let candidate = FormCandidate::from_source(
        "Button",
        "button.conduit",
        source,
        "canonical Body planning proof",
        SignId::from("sign/button-candidate"),
        1,
    )
    .unwrap();
    let mut workset = BodyWorkset::default();
    workset
        .add(ResidentForm::new(
            candidate.source_document_id.clone(),
            candidate.checked_form_id.clone(),
        ))
        .unwrap();
    let mut host = conduit_std_host::StdHost::new().advertisement().clone();
    host.capabilities = vec![
        conduit_std_offers::button::offer(),
        conduit_std_offers::button::mapper_offer(),
        conduit_std_offers::button::indicator_offer(),
    ];
    host.capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    host.resources.push(resource_offer(
        "proof/body-keyboard",
        INPUT_RESOURCE_CLASS,
        1,
    ));
    host.resources.sort();
    let plans = plan_body_workset_on_host(
        &workset,
        &[candidate],
        &host,
        &["conduit.base/local@1".into()],
    )
    .unwrap();
    assert_eq!(plans.len(), 1);
    let fragment = &plans[0].plan.fragments[0];
    let indicator_cord = fragment
        .connections
        .iter()
        .find(|cord| cord.value_kind.as_str() == conduit_core::BOOL_INFO_ID)
        .unwrap();
    assert_eq!(indicator_cord.item_capacity, 1);
    assert_eq!(indicator_cord.byte_capacity, 1);
}
