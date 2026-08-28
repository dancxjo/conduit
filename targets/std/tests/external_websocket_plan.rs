use conduit_core::{
    BaseImplementationId, BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    PlannerCapabilityOffer, PlannerProfileId, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use tungstenite::protocol::Message;

const SOURCE: &str = include_str!("../../../examples/webchat.conduit");

fn browser() -> HostAdvertisement {
    let family = conduit_net::browser_external_websocket_family();
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("browser-chat"),
        boot_id: BootId::from("browser-chat-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser"),
        resources: vec![family.resource],
        planner_capabilities: vec![PlannerCapabilityOffer {
            profile_id: PlannerProfileId::from(conduit_planner::FULL_PLANNER_PROFILE),
            limits: conduit_planner::FULL_PLANNER_LIMITS,
        }],
        capabilities: vec![family.capability],
    }
}

#[test]
fn canonical_transport_forms_plan_to_exact_opt_in_browser_and_std_families() {
    let syntax = parse_syntax_document(SOURCE);
    assert_eq!(syntax.round_trip(), SOURCE);
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_net::install_external_websocket_catalogs(&mut startup, &mut profile).unwrap();
    conduit_chat::install_browser_chat_catalogs(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "webchat-transport-demo", &profile).unwrap();

    let std = StdHost::new_with_composition(
        StdHostConfig {
            host_id: HostId::from("std-chat"),
            boot_id: BootId::from("std-chat-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal().with_external_websocket(),
    );
    let hosts = [browser(), std.advertisement().clone()];
    let placements = conduit_planner::PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|operation| {
                let (host_id, capability_id) =
                    if operation.kind_id.as_str() == conduit_net::EXTERNAL_WEBSOCKET_CLIENT_KIND {
                        (&hosts[0].host_id, &hosts[0].capabilities[0].capability_id)
                    } else {
                        (&hosts[1].host_id, &hosts[1].capabilities[0].capability_id)
                    };
                (
                    operation.gear_id.clone(),
                    conduit_planner::PlacementChoice {
                        host_id: host_id.clone(),
                        capability_id: capability_id.clone(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
    };
    let plan = conduit_planner::plan_expanded_canonical(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();

    assert_eq!(plan.fragments.len(), 2);
    let client = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.kind_id.as_str() == conduit_net::EXTERNAL_WEBSOCKET_CLIENT_KIND)
        .unwrap();
    let listener = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| {
            placement.kind_id.as_str() == conduit_net::EXTERNAL_WEBSOCKET_LISTENER_KIND
        })
        .unwrap();
    assert_eq!(client.host_id.as_str(), "browser-chat");
    assert_eq!(listener.host_id.as_str(), "std-chat");
    assert_eq!(client.inputs, hosts[0].capabilities[0].inputs);
    assert_eq!(client.outputs, hosts[0].capabilities[0].outputs);
    assert_eq!(listener.inputs, hosts[1].capabilities[0].inputs);
    assert_eq!(listener.outputs, hosts[1].capabilities[0].outputs);
    assert_ne!(client.gear_id, listener.gear_id);
}

struct ReadyWriter {
    bytes: Vec<u8>,
    ready: Option<mpsc::Sender<()>>,
}

impl Write for ReadyWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        if self.ready.is_some()
            && self
                .bytes
                .windows("external-websocket-ready".len())
                .any(|window| window == b"external-websocket-ready")
        {
            self.ready.take().unwrap().send(()).unwrap();
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn planned_listener_executes_through_kernel_host_operations_for_two_clients() {
    let port = TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let source = SOURCE.replace("127.0.0.1:4178", &format!("127.0.0.1:{port}"));
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_net::install_external_websocket_catalogs(&mut startup, &mut profile).unwrap();
    conduit_chat::install_browser_chat_catalogs(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "webchat-server-demo", &profile).unwrap();
    let mut host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: HostId::from("std-chat-kernel"),
            boot_id: BootId::from("std-chat-kernel-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal().with_external_websocket(),
    );
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let plan_id = plan.plan_id.clone();
    let fragment = plan.fragments[0].clone();
    let (ready_tx, ready_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let mut output = ReadyWriter {
            bytes: Vec::with_capacity(4096),
            ready: Some(ready_tx),
        };
        let mut timer = conduit_std_host::ThreadTimer;
        let report = host
            .run_fragment_to(fragment, &mut output, &mut timer)
            .unwrap();
        (report, output.bytes)
    });
    ready_rx.recv().unwrap();
    let url = format!("ws://127.0.0.1:{port}");
    let connect = |url: String| {
        thread::spawn(move || {
            let stream = TcpStream::connect(url.trim_start_matches("ws://")).unwrap();
            tungstenite::client(url, stream).unwrap().0
        })
    };
    let client_a = connect(url.clone());
    let mut client_a = client_a.join().unwrap();
    let client_b = connect(url);
    let mut client_b = client_b.join().unwrap();

    client_a
        .send(Message::Binary(b"hello from A".to_vec().into()))
        .unwrap();
    assert_eq!(
        client_a.read().unwrap().into_data().as_ref(),
        b"hello from A"
    );
    assert_eq!(
        client_b.read().unwrap().into_data().as_ref(),
        b"hello from A"
    );
    client_b
        .send(Message::Binary(b"hello from B".to_vec().into()))
        .unwrap();
    assert_eq!(
        client_a.read().unwrap().into_data().as_ref(),
        b"hello from B"
    );
    assert_eq!(
        client_b.read().unwrap().into_data().as_ref(),
        b"hello from B"
    );

    client_a.close(None).unwrap();
    client_b
        .send(Message::Binary(b"still connected".to_vec().into()))
        .unwrap();
    assert_eq!(
        client_b.read().unwrap().into_data().as_ref(),
        b"still connected"
    );
    client_b.close(None).unwrap();

    let (report, output) = server.join().unwrap();
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.identity.plan_id, plan_id);
    assert!(!kernel.active_play_id.as_str().is_empty());
    assert!(kernel.kernel_events > 0);
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("external-websocket-ready"));
    assert!(!output.contains("hello from"));
}
