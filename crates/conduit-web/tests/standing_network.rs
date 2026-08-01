use conduit_web::{
    patchbay_advance_exact_run, patchbay_attach_exact_watch, patchbay_cancel_exact_run,
    patchbay_detach_exact_watch, patchbay_open_session, patchbay_pump_exact_run,
    patchbay_start_exact_run,
};
use serde_json::Value;

fn json(value: String) -> Value {
    serde_json::from_str(&value).unwrap_or_else(|error| panic!("{error}: {value}"))
}

#[test]
fn standing_packet_path_waits_observes_resumes_and_stops_explicitly() {
    let session_id = "standing-network-packet-proof";
    let source = include_str!("../../../examples/standing-network-packet-path.panel").to_owned();
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
        "watch/cord-2".to_owned(),
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
    let waiting_again = json(patchbay_pump_exact_run(
        session_id.to_owned(),
        run_id.clone(),
        revision,
        plan_identity.clone(),
        64,
    ));
    assert_eq!(waiting_again["state"], "waiting", "{waiting_again}");
    assert!(waiting_again["next_timer_deadline"].as_u64().unwrap() > first_deadline);
    assert_eq!(waiting_again["plan_identity"], plan_identity);

    let detached = json(patchbay_detach_exact_watch(
        session_id.to_owned(),
        run_id.clone(),
        revision,
        plan_identity.clone(),
        "operator/browser-patchbay".to_owned(),
        "watch/cord-2".to_owned(),
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
fn one_listener_serves_repeated_sessions_without_topology_growth() {
    let session_id = "standing-network-listener-proof";
    let source = include_str!("../../../examples/standing-network-listener.panel").to_owned();
    let opened = json(patchbay_open_session(session_id.to_owned(), source));
    assert_eq!(opened["ok"], true, "{opened}");
    assert_eq!(
        opened["view"]["topology"]["logical_nodes"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        opened["view"]["topology"]["cords"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    let started = json(patchbay_start_exact_run(session_id.to_owned()));
    assert_eq!(started["ok"], true, "{started}");
    let run_id = started["run_id"].as_str().unwrap().to_owned();
    let revision = started["source_revision"].as_u64().unwrap();
    let plan_identity = started["plan_identity"].as_str().unwrap().to_owned();

    let first_wait = json(patchbay_pump_exact_run(
        session_id.to_owned(),
        run_id.clone(),
        revision,
        plan_identity.clone(),
        64,
    ));
    assert_eq!(first_wait["state"], "waiting", "{first_wait}");
    let first_deadline = first_wait["next_timer_deadline"].as_u64().unwrap();

    let resumed = json(patchbay_advance_exact_run(
        session_id.to_owned(),
        run_id.clone(),
        revision,
        plan_identity.clone(),
        first_deadline,
    ));
    assert_eq!(resumed["ok"], true, "{resumed}");
    let second_wait = json(patchbay_pump_exact_run(
        session_id.to_owned(),
        run_id.clone(),
        revision,
        plan_identity.clone(),
        64,
    ));
    assert_eq!(second_wait["state"], "waiting", "{second_wait}");
    assert_eq!(second_wait["plan_identity"], plan_identity);
    assert!(second_wait["next_timer_deadline"].as_u64().unwrap() > first_deadline);

    let cancelled = json(patchbay_cancel_exact_run(
        session_id.to_owned(),
        run_id,
        revision,
        plan_identity.clone(),
        "abort".to_owned(),
    ));
    assert_eq!(cancelled["ok"], true, "{cancelled}");
    assert_eq!(cancelled["state"], "cancelled");
    assert_eq!(cancelled["plan_identity"], plan_identity);
}

#[test]
fn frame_datagram_and_stream_paths_remain_distinct_in_one_standing_run() {
    let session_id = "standing-network-value-families";
    let source = include_str!("../../../examples/standing-network-values.panel").to_owned();
    let opened = json(patchbay_open_session(session_id.to_owned(), source));
    assert_eq!(opened["ok"], true, "{opened}");
    let nodes = opened["view"]["topology"]["logical_nodes"]
        .as_array()
        .unwrap();
    assert_eq!(nodes.len(), 7);
    let port_types = nodes
        .iter()
        .flat_map(|node| {
            node["inputs"]
                .as_array()
                .into_iter()
                .flatten()
                .chain(node["outputs"].as_array().into_iter().flatten())
        })
        .filter_map(|port| port["type_id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for expected in [
        "conduit.net/frame",
        "conduit.net/datagram",
        "conduit.net/byte-stream",
        "conduit.net/retained-state",
    ] {
        assert!(
            port_types.contains(expected),
            "missing {expected}: {opened}"
        );
    }

    let started = json(patchbay_start_exact_run(session_id.to_owned()));
    assert_eq!(started["ok"], true, "{started}");
    let waiting = json(patchbay_pump_exact_run(
        session_id.to_owned(),
        started["run_id"].as_str().unwrap().to_owned(),
        started["source_revision"].as_u64().unwrap(),
        started["plan_identity"].as_str().unwrap().to_owned(),
        48,
    ));
    assert_eq!(waiting["ok"], true, "{waiting}");
    assert_eq!(waiting["state"], "waiting", "{waiting}");
    assert!(waiting["next_timer_deadline"].as_u64().is_some());
}

#[test]
fn isolated_local_services_keep_readiness_layers_distinct_and_standing() {
    let session_id = "standing-isolated-local-network";
    let source = include_str!("../../../examples/pico-network-providers.panel").to_owned();
    let opened = json(patchbay_open_session(session_id.to_owned(), source));
    assert_eq!(opened["ok"], true, "{opened}");
    assert_eq!(
        opened["view"]["topology"]["logical_nodes"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    let cord_types = opened["view"]["topology"]["logical_nodes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|node| {
            node["inputs"]
                .as_array()
                .into_iter()
                .flatten()
                .chain(node["outputs"].as_array().into_iter().flatten())
        })
        .filter_map(|port| port["type_id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        cord_types,
        std::collections::BTreeSet::from([
            "conduit.net/link-observation",
            "conduit.net/address-state",
            "conduit.net/control-event",
            "conduit.net/dhcp-lease",
            "conduit.net/retained-state",
            "conduit.net/service-registration",
            "conduit.net/reachability-observation",
        ])
    );

    let started = json(patchbay_start_exact_run(session_id.to_owned()));
    assert_eq!(started["ok"], true, "{started}");
    let waiting = json(patchbay_pump_exact_run(
        session_id.to_owned(),
        started["run_id"].as_str().unwrap().to_owned(),
        started["source_revision"].as_u64().unwrap(),
        started["plan_identity"].as_str().unwrap().to_owned(),
        32,
    ));
    assert_eq!(waiting["ok"], true, "{waiting}");
    assert_eq!(waiting["state"], "waiting", "{waiting}");
    assert_eq!(waiting["terminal"], Value::Null);
    assert!(waiting["next_timer_deadline"].as_u64().is_some());
}

#[test]
fn complete_tour_network_projects_two_endpoints_and_every_value_family() {
    let session_id = "standing-network-tour-project";
    let source = include_str!("../../../examples/standing-network-tour.panel").to_owned();
    let opened = json(patchbay_open_session(session_id.to_owned(), source));
    assert_eq!(opened["ok"], true, "{opened}");
    assert_eq!(
        opened["view"]["topology"]["logical_nodes"]
            .as_array()
            .unwrap()
            .len(),
        16
    );
    assert_eq!(
        opened["view"]["topology"]["cords"]
            .as_array()
            .unwrap()
            .len(),
        12
    );
    let port_types = opened["view"]["topology"]["logical_nodes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|node| {
            node["inputs"]
                .as_array()
                .into_iter()
                .flatten()
                .chain(node["outputs"].as_array().into_iter().flatten())
        })
        .filter_map(|port| port["type_id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for expected in [
        "conduit.net/link-observation",
        "conduit.net/frame",
        "conduit.net/packet",
        "conduit.net/datagram",
        "conduit.net/byte-stream",
        "conduit.net/session",
        "conduit.net/control-event",
        "conduit.net/retained-state",
        "conduit.net/address-state",
        "conduit.net/dhcp-lease",
        "conduit.net/service-registration",
        "conduit.net/reachability-observation",
    ] {
        assert!(
            port_types.contains(expected),
            "missing {expected}: {opened}"
        );
    }

    assert!(opened["view"].get("plan").is_some(), "{opened}");
}

#[test]
fn zero_delay_packet_loop_remains_renderable_with_an_exact_diagnostic() {
    let source = include_str!("../../../examples/invalid-zero-delay-network-loop.panel").to_owned();
    let opened = json(patchbay_open_session(
        "invalid-network-loop".to_owned(),
        source,
    ));
    assert_eq!(opened["ok"], true, "{opened}");
    assert_eq!(
        opened["view"]["topology"]["logical_nodes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        opened["view"]["topology"]["cords"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(opened["view"].get("plan").is_none(), "{opened}");
    let diagnostics = opened["view"]["diagnostics"].as_array().unwrap();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["code"] == "CND-CMP-001"
            || diagnostic["explanation"]
                .as_str()
                .is_some_and(|value| value.contains("cycle"))
    }));
}
