//! Body-session CPU fixtures; no DOM, browser acquisition or Host Show evidence.
use super::super::*;
use conduit_face::Face;

fn request() -> body_start::BodyStartRequest {
    let source = include_str!("../../../../../plots/patchbay/main.conduit");
    let mut request = body_start::tests::request_from_sources(&[source]);
    request.source = source.into();
    request
}

#[test]
fn observation_reads_current_body_instance_without_event_or_show() {
    let request = request();
    let body = request.plan.body_id.clone();
    let (session, started) = body_start::prepare(request).unwrap();
    let pending = session.pending.len();
    let initial = session.patchbay_observation().unwrap();
    let observed: Face = serde_json::from_value(initial["face"].clone()).unwrap();
    observed.validate().unwrap();
    assert_eq!(observed.basis.body_id.as_ref(), Some(&body));
    assert_eq!(
        initial["active_play_id"],
        serde_json::to_value(&started.play.active_play_id).unwrap()
    );
    assert!(initial["mask_show"].is_null());
    assert_eq!(initial["interaction_admitted"], false);
    let select = observed
        .actions
        .iter()
        .find(|action| action.identity == "patchbay.plot.0")
        .unwrap();
    assert!(!select.availability.is_available());
    assert_eq!(session.patchbay_observation().unwrap(), initial);
    assert_eq!(session.pending.len(), pending);
    assert_eq!(session.active_play_id, started.play.active_play_id);
    session.cancel().unwrap();
}

#[test]
fn missing_optional_biography_does_not_break_legacy_patchbay_startup() {
    let mut request = request();
    request.body_evidence = None;
    let (session, _) = body_start::prepare(request).unwrap();
    assert!(session
        .patchbay_observation()
        .unwrap_err()
        .contains("no admitted Patchbay observation context"));
    session.cancel().unwrap();
}

#[test]
fn exact_owner_is_selected_when_two_patchbay_residents_are_prepared() {
    let first = include_str!("../../../../../plots/patchbay/main.conduit");
    let second = first.replacen("plot patchbay", "plot another_patchbay", 1);
    let mut request = body_start::tests::request_from_sources(&[first, &second]);
    request.source = serde_json::json!({
        "schema": "conduit.creche/reviewed-plot-bundle@2",
        "plots": [
            { "slug": "first", "entry": "patchbay", "source": first },
            { "slug": "second", "entry": "another_patchbay", "source": second }
        ]
    })
    .to_string();
    request.foreground_checked_plot_id = request.plan.plots[1].plot.checked_plot_id.as_str().into();
    let selected = request.plan.plots[1].plot.checked_plot_id.clone();
    let (session, _) = body_start::prepare(request).unwrap();
    let observation = session.patchbay_observation().unwrap();
    let observed: Face = serde_json::from_value(observation["face"].clone()).unwrap();
    observed.validate().unwrap();
    assert!(observed
        .subjects
        .iter()
        .any(|subject| subject.identity == format!("patchbay/document/{}", selected.as_str())));
    session.cancel().unwrap();
}

#[test]
fn wrong_bound_node_refuses_instead_of_searching_for_another_patchbay() {
    let (mut session, _) = body_start::prepare(request()).unwrap();
    session.patchbay_face.as_mut().unwrap().owner_node = conduit_kernel::NodeId(u16::MAX);
    assert!(session
        .patchbay_observation()
        .unwrap_err()
        .contains("exact resident Patchbay owner is not prepared"));
    session.cancel().unwrap();
}
