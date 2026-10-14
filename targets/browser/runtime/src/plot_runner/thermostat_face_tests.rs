//! Kernel state, ordinary Mask Show and typed action agreement.
use super::super::*;
use crate::workspace_mask::{BrowserMaskAcknowledgement, BrowserMaskInteraction};
use conduit_face::MaskShow;

fn acknowledge(session: &mut TourSession, snapshot: &serde_json::Value) {
    let show: MaskShow = serde_json::from_value(snapshot["mask_show"].clone()).unwrap();
    let ack = BrowserMaskAcknowledgement {
        show_id: show.show_id.as_str().into(),
        manifestation_id: show.show.manifestation_id.as_str().into(),
        mask_plan_id: show.planned_mask.plan.plan_id.clone(),
        active_play_id: show.show.active_play_id.clone(),
        placement_id: show.show.placement_id.clone(),
        face_id: show.face_id.as_str().into(),
        face_revision: show.face_revision,
    };
    let mut stale = ack.clone();
    stale.face_revision = stale.face_revision.saturating_add(1);
    assert!(session.acknowledge_thermostat_show(stale).is_err());
    session.acknowledge_thermostat_show(ack.clone()).unwrap();
    session.acknowledge_thermostat_show(ack).unwrap();
}
fn interaction(snapshot: &serde_json::Value, action: &str) -> BrowserMaskInteraction {
    let show: MaskShow = serde_json::from_value(snapshot["mask_show"].clone()).unwrap();
    let face: conduit_face::Face = serde_json::from_value(snapshot["face"].clone()).unwrap();
    BrowserMaskInteraction {
        show_id: show.show_id.as_str().into(),
        face_id: show.face_id.as_str().into(),
        face_revision: show.face_revision,
        action_id: action.into(),
        target: face
            .actions
            .iter()
            .find(|entry| entry.identity == action)
            .unwrap()
            .target
            .clone(),
        arguments: vec![],
        sequence: 1,
    }
}
#[test]
fn thermostat_actions_require_ack_and_commit_current_scan_output() {
    let request = body_start::tests::request_from_sources(&[include_str!(
        "../../../../../plots/thermostat/main.conduit"
    )]);
    let mut next_request: body_start::BodyStartRequest =
        serde_json::from_value(serde_json::to_value(&request).unwrap()).unwrap();
    next_request.play_sequence = request.play_sequence + 1;
    let (mut session, _) = body_start::prepare(request).unwrap();
    let initial = session.thermostat_snapshot().unwrap();
    assert_eq!(initial["face"]["revision"], 0);
    assert!(session
        .interact_thermostat(interaction(&initial, "thermostat.raise"))
        .is_err());
    acknowledge(&mut session, &initial);
    let mut forged = interaction(&initial, "thermostat.raise");
    forged.target = "forged".into();
    assert!(session.interact_thermostat(forged).is_err());
    let stale = interaction(&initial, "thermostat.raise");
    let changed = session.interact_thermostat(stale.clone()).unwrap();
    assert_eq!(changed["face"]["face"]["revision"], 1);
    assert!(session.interact_thermostat(stale).is_err());
    assert_eq!(session.scheduler.thermostat_state().unwrap().target, 215);
    assert_eq!(session.scheduler.thermostat_state().unwrap().measured, None);
    acknowledge(&mut session, &changed["face"]);
    let heat = session
        .interact_thermostat(interaction(&changed["face"], "thermostat.mode.heat"))
        .unwrap();
    acknowledge(&mut session, &heat["face"]);
    let repeated = session
        .interact_thermostat(interaction(&heat["face"], "thermostat.mode.heat"))
        .unwrap();
    assert_eq!(session.scheduler.thermostat_state().unwrap().revision, 2);
    assert_eq!(repeated["face"]["face"]["revision"], 3);
    assert_ne!(
        repeated["face"]["mask_show"]["show_id"],
        heat["face"]["mask_show"]["show_id"]
    );
    acknowledge(&mut session, &repeated["face"]);
    assert!(session
        .interact_thermostat(interaction(&heat["face"], "thermostat.mode.heat"))
        .is_err());
    session.cancel().unwrap();
    let (mut next_session, _) = body_start::prepare(next_request).unwrap();
    let next_face = next_session.thermostat_snapshot().unwrap();
    assert_ne!(
        next_face["effect"]["mask_play"]["active_play_id"],
        initial["effect"]["mask_play"]["active_play_id"]
    );
    assert_ne!(
        changed["face"]["effect"]["mask_play"]["active_play_id"],
        initial["effect"]["mask_play"]["active_play_id"]
    );
    assert_eq!(next_face["face"]["revision"], 0);
    assert_eq!(
        next_session.scheduler.thermostat_state().unwrap().target,
        210
    );
    assert!(next_session
        .interact_thermostat(interaction(&initial, "thermostat.raise"))
        .is_err());
    next_session.cancel().unwrap();
}

#[test]
fn resident_thermostat_actions_survive_another_startup_foreground() {
    let mut request = body_start::tests::request_from_sources(&[
        include_str!("../../../../../plots/patchbay/main.conduit"),
        include_str!("../../../../../plots/thermostat/main.conduit"),
    ]);
    request.source = serde_json::json!({
        "schema": "conduit.creche/reviewed-plot-bundle@2",
        "plots": [
            { "slug": "patchbay", "entry": "patchbay", "source": include_str!("../../../../../plots/patchbay/main.conduit") },
            { "slug": "thermostat", "entry": "thermostat/main", "source": include_str!("../../../../../plots/thermostat/main.conduit") },
        ],
    }).to_string();
    assert_ne!(
        request.foreground_checked_plot_id,
        request.plan.plots[1].plot.checked_plot_id.as_str()
    );
    let (mut session, started) = body_start::prepare(request).unwrap();
    let face = session.thermostat_snapshot().unwrap();
    acknowledge(&mut session, &face);
    let next = session
        .interact_thermostat(interaction(&face, "thermostat.raise"))
        .unwrap();
    assert_eq!(
        next["face"]["active_play_id"],
        started.play.active_play_id.as_str()
    );
    assert_eq!(next["face"]["face"]["revision"], 1);
    assert_eq!(session.scheduler.thermostat_state().unwrap().target, 215);
    session.cancel().unwrap();
}
