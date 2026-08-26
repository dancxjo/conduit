use conduit_core::{AuthorityGrant, AuthorityGrantId, BootId, HostId, OfferGeneration};
use conduit_std_host::{
    RunControl, RunControlRequestId, StdHost, StdHostComposition, StdHostConfig, ThreadTimer,
};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;

struct ChannelWriter {
    sender: mpsc::Sender<String>,
}

impl Write for ChannelWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let _ = self
            .sender
            .send(String::from_utf8_lossy(bytes).into_owned());
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn production_http_gears_execute_four_real_correlated_exchanges() {
    let fixture = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let fixture_address = fixture.local_addr().unwrap();
    let fixture_thread = thread::spawn(move || {
        for sequence in 0..conduit_web::HTTP_MAXIMUM_IN_FLIGHT {
            let (mut stream, _) = fixture.accept().unwrap();
            let request = read_http_message(&mut stream).unwrap();
            if sequence == 0 {
                assert!(request.starts_with(b"GET /fixture HTTP/1.1\r\n"));
            } else {
                assert!(request.starts_with(b"POST /fixture HTTP/1.1\r\n"));
                assert!(request.ends_with(format!("payload-{sequence}").as_bytes()));
            }
            let status = match sequence {
                1 => 404,
                2 => 503,
                _ => 200,
            };
            let body = format!("response-{sequence}");
            write!(
                stream,
                "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });

    let config = StdHostConfig {
        host_id: HostId::from("http-host"),
        boot_id: BootId::from("http-boot"),
        offer_generation: OfferGeneration(1),
    };
    let mut host = StdHost::new_with_composition(config, StdHostComposition::minimal().with_http());
    let mut startup = conduit_form::StartupCatalog::new();
    let mut catalog = conduit_form::ProfileCatalog::new();
    conduit_web::install_http_catalogs(&mut startup, &mut catalog).unwrap();
    let form = conduit_form::parse(
        "form proxy {\n server: http/server\n client: http/client\n server.request > client.request\n client.response > server.response\n}\n",
        &catalog,
    )
    .unwrap();
    let host_id = host.advertisement().host_id.clone();
    let boot_id = host.advertisement().boot_id.clone();
    let grants = host
        .advertisement()
        .capabilities
        .iter()
        .flat_map(|capability| {
            capability
                .authority_requirements
                .iter()
                .enumerate()
                .map(|(index, requirement)| AuthorityGrant {
                    grant_id: AuthorityGrantId::from(format!(
                        "{}-{index}",
                        capability.capability_id.as_str()
                    )),
                    contract_id: requirement.contract_id.clone(),
                    host_operation_contract_id: requirement.host_operation_contract_id.clone(),
                    subject_kind: requirement.subject_kind.clone(),
                    host_id: host_id.clone(),
                    boot_id: boot_id.clone(),
                    capability_id: capability.capability_id.clone(),
                })
        })
        .collect::<Vec<_>>();
    assert!(host.plan_local(&form, None).is_err());
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_placements(&form, &hosts).unwrap();
    let plan = conduit_planner::plan_with_options(
        &form,
        &hosts,
        &placements,
        &[conduit_core::ConnectionBase::Local],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES
                .max(conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES),
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let fragment = plan.fragments[0].clone();
    assert_eq!(fragment.placements.len(), 2);
    let server = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_web::HTTP_SERVER_KIND)
        .unwrap();
    let client = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_web::HTTP_CLIENT_KIND)
        .unwrap();
    assert_eq!(server.authority.len(), 2);
    assert_eq!(client.authority.len(), 1);

    let (sender, receiver) = mpsc::channel();
    let (result_sender, result_receiver) = mpsc::channel();
    let run = thread::spawn(move || {
        let result =
            host.run_fragment_to(fragment, &mut ChannelWriter { sender }, &mut ThreadTimer);
        let _ = result_sender.send(result.clone());
        result
    });
    let mut transcript = String::new();
    let proxy_address = loop {
        let chunk = receiver.recv().unwrap();
        transcript.push_str(&chunk);
        if let Some(address) = transcript.lines().find_map(|line| {
            line.strip_prefix("http-server-ready address=")
                .and_then(|value| value.parse::<std::net::SocketAddr>().ok())
        }) {
            break address;
        }
    };

    for sequence in 0..conduit_web::HTTP_MAXIMUM_IN_FLIGHT {
        let mut stream = TcpStream::connect(proxy_address).unwrap();
        let (method, body) = if sequence == 0 {
            ("GET", String::new())
        } else {
            ("POST", format!("payload-{sequence}"))
        };
        write!(
            stream,
            "{method} /fixture HTTP/1.1\r\nHost: {fixture_address}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let response = read_http_message(&mut stream).unwrap_or_else(|error| {
            panic!(
                "proxy response failed: {error}; run={:?}",
                result_receiver.recv_timeout(std::time::Duration::from_secs(1))
            )
        });
        let expected = match sequence {
            1 => 404,
            2 => 503,
            _ => 200,
        };
        assert!(response.starts_with(format!("HTTP/1.1 {expected} ").as_bytes()));
        assert!(response.ends_with(format!("response-{sequence}").as_bytes()));
    }

    fixture_thread.join().unwrap();
    let report = run.join().unwrap().unwrap();
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.identity.lengths().0, 12);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

#[test]
fn operator_cancellation_releases_the_admitted_listener_before_accept() {
    let config = StdHostConfig {
        host_id: HostId::from("cancel-http-host"),
        boot_id: BootId::from("cancel-http-boot"),
        offer_generation: OfferGeneration(1),
    };
    let mut host = StdHost::new_with_composition(config, StdHostComposition::minimal().with_http());
    let mut startup = conduit_form::StartupCatalog::new();
    let mut catalog = conduit_form::ProfileCatalog::new();
    conduit_web::install_http_catalogs(&mut startup, &mut catalog).unwrap();
    let form = conduit_form::parse(
        "form proxy {\n server: http/server\n client: http/client\n server.request > client.request\n client.response > server.response\n}\n",
        &catalog,
    )
    .unwrap();
    let grants = host
        .advertisement()
        .capabilities
        .iter()
        .flat_map(|capability| {
            capability
                .authority_requirements
                .iter()
                .enumerate()
                .map(|(index, requirement)| AuthorityGrant {
                    grant_id: AuthorityGrantId::from(format!(
                        "cancel-{}-{index}",
                        capability.capability_id.as_str()
                    )),
                    contract_id: requirement.contract_id.clone(),
                    host_operation_contract_id: requirement.host_operation_contract_id.clone(),
                    subject_kind: requirement.subject_kind.clone(),
                    host_id: host.advertisement().host_id.clone(),
                    boot_id: host.advertisement().boot_id.clone(),
                    capability_id: capability.capability_id.clone(),
                })
        })
        .collect::<Vec<_>>();
    let plan = host
        .plan_local_with_authority(&form, None, &grants)
        .unwrap();
    let control = RunControl::default();
    control
        .request_stop(RunControlRequestId::new("cancel-http").unwrap())
        .unwrap();
    let report = host
        .run_fragment_controlled_to(
            plan.fragments[0].clone(),
            &mut Vec::new(),
            &mut ThreadTimer,
            &control,
        )
        .unwrap();
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: conduit_core::TerminalDisposition::Cancelled { .. }
        })
    ));
}

fn read_http_message(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let head_end = loop {
        let mut byte = [0; 1];
        stream
            .read_exact(&mut byte)
            .map_err(|error| error.to_string())?;
        bytes.push(byte[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            break bytes.len();
        }
    };
    let head = std::str::from_utf8(&bytes[..head_end]).map_err(|error| error.to_string())?;
    let content_length = head
        .split("\r\n")
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")
                .and_then(|value| value.parse::<usize>().ok())
        })
        .ok_or_else(|| "response has no content length".to_string())?;
    let body_start = bytes.len();
    bytes.resize(body_start + content_length, 0);
    stream
        .read_exact(&mut bytes[body_start..])
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}
