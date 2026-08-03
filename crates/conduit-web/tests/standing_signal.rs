use conduit_web::{
    patchbay_advance_exact_run, patchbay_attach_exact_watch, patchbay_cancel_exact_run,
    patchbay_detach_exact_watch, patchbay_open_session as patchbay_open_session_with_front,
    patchbay_pump_exact_run, patchbay_start_exact_run,
};
use serde_json::Value;

fn json(value: String) -> Value {
    serde_json::from_str(&value).unwrap_or_else(|error| panic!("{error}: {value}"))
}

fn patchbay_open_session(document_id: String, source: String) -> String {
    patchbay_open_session_with_front(document_id, source, String::new(), String::new())
}

#[test]
fn standing_modulated_audio_waits_resumes_observes_and_stops_explicitly() {
    let session_id = "standing-signal-proof";
    let source = include_str!("../../../examples/standing-signal-lab.panel").to_owned();
    let opened = json(patchbay_open_session(session_id.to_owned(), source));
    assert_eq!(opened["ok"], true, "{opened}");

    let started = json(patchbay_start_exact_run(session_id.to_owned()));
    assert_eq!(started["ok"], true, "{started}");
    assert_eq!(started["state"], "active");
    let run_id = started["run_id"].as_str().unwrap().to_owned();
    let revision = started["source_revision"].as_u64().unwrap();
    let plan_identity = started["plan_identity"].as_str().unwrap().to_owned();

    let waiting = json(patchbay_pump_exact_run(
        session_id.to_owned(),
        run_id.clone(),
        revision,
        plan_identity.clone(),
        64,
    ));
    assert_eq!(waiting["ok"], true, "{waiting}");
    assert_eq!(waiting["state"], "waiting");
    assert_eq!(waiting["terminal"], Value::Null);
    let first_deadline = waiting["next_timer_deadline"].as_u64().unwrap();

    let attached = json(patchbay_attach_exact_watch(
        session_id.to_owned(),
        run_id.clone(),
        revision,
        plan_identity.clone(),
        "operator/browser-patchbay".to_owned(),
        "watch/cord-6".to_owned(),
    ));
    assert_eq!(attached["ok"], true, "{attached}");
    assert_eq!(attached["plan_identity"], plan_identity);

    let resumed = json(patchbay_advance_exact_run(
        session_id.to_owned(),
        run_id.clone(),
        revision,
        plan_identity.clone(),
        first_deadline,
    ));
    assert_eq!(resumed["ok"], true, "{resumed}");
    assert_eq!(resumed["state"], "active");
    let waiting_again = json(patchbay_pump_exact_run(
        session_id.to_owned(),
        run_id.clone(),
        revision,
        plan_identity.clone(),
        64,
    ));
    assert_eq!(waiting_again["ok"], true, "{waiting_again}");
    assert_eq!(waiting_again["state"], "waiting");
    assert_eq!(waiting_again["plan_identity"], plan_identity);
    assert!(
        waiting_again["next_timer_deadline"].as_u64().unwrap() > first_deadline,
        "the same live epoch registers its next exact pulse"
    );

    let detached = json(patchbay_detach_exact_watch(
        session_id.to_owned(),
        run_id.clone(),
        revision,
        plan_identity.clone(),
        "operator/browser-patchbay".to_owned(),
        "watch/cord-6".to_owned(),
    ));
    assert_eq!(detached["ok"], true, "{detached}");
    assert_eq!(detached["plan_identity"], plan_identity);

    let cancelled = json(patchbay_cancel_exact_run(
        session_id.to_owned(),
        run_id,
        revision,
        plan_identity.clone(),
        "abort".to_owned(),
    ));
    assert_eq!(cancelled["ok"], true, "{cancelled}");
    assert_eq!(cancelled["state"], "cancelled");
    assert_eq!(cancelled["terminal"], "cancelled");
    assert_eq!(cancelled["plan_identity"], plan_identity);
}

#[test]
fn virtual_capture_processing_playback_waits_and_stops_explicitly() {
    let session_id = "virtual-audio-loopback-proof";
    let source = include_str!("../../../examples/virtual-audio-loopback.panel").to_owned();
    let opened = json(patchbay_open_session(session_id.to_owned(), source));
    assert_eq!(opened["ok"], true, "{opened}");

    let started = json(patchbay_start_exact_run(session_id.to_owned()));
    assert_eq!(started["ok"], true, "{started}");
    let run_id = started["run_id"].as_str().unwrap().to_owned();
    let revision = started["source_revision"].as_u64().unwrap();
    let plan_identity = started["plan_identity"].as_str().unwrap().to_owned();

    let waiting = json(patchbay_pump_exact_run(
        session_id.to_owned(),
        run_id.clone(),
        revision,
        plan_identity.clone(),
        32,
    ));
    assert_eq!(waiting["ok"], true, "{waiting}");
    assert_eq!(waiting["state"], "waiting");
    assert_eq!(waiting["terminal"], Value::Null);
    let deadline = waiting["next_timer_deadline"].as_u64().unwrap();

    let resumed = json(patchbay_advance_exact_run(
        session_id.to_owned(),
        run_id.clone(),
        revision,
        plan_identity.clone(),
        deadline,
    ));
    assert_eq!(resumed["ok"], true, "{resumed}");
    let waiting_again = json(patchbay_pump_exact_run(
        session_id.to_owned(),
        run_id.clone(),
        revision,
        plan_identity.clone(),
        32,
    ));
    assert_eq!(waiting_again["state"], "waiting");
    assert!(waiting_again["next_timer_deadline"].as_u64().unwrap() > deadline);
    assert_eq!(waiting_again["plan_identity"], plan_identity);

    let cancelled = json(patchbay_cancel_exact_run(
        session_id.to_owned(),
        run_id,
        revision,
        plan_identity.clone(),
        "abort".to_owned(),
    ));
    assert_eq!(cancelled["ok"], true, "{cancelled}");
    assert_eq!(cancelled["state"], "cancelled");
    assert_eq!(cancelled["terminal"], "cancelled");
    assert_eq!(cancelled["plan_identity"], plan_identity);
}
