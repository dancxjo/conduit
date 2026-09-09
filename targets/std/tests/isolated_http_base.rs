#![cfg(all(target_os = "linux", feature = "isolated-http-base"))]

use conduit_core::{
    AuthorityGrant, AuthorityGrantId, BaseImplementationId, BaseInstanceId, BootId, HostId,
    OfferGeneration, ResourceGenerationId,
};
use conduit_std_host::isolated_http_base::{
    run_adversarial_http_proof, IsolatedHttpBaseConfig, IsolatedHttpHost,
    ISOLATED_HTTP_IMPLEMENTATION,
};
use conduit_std_host::{StdHostComposition, StdHostConfig};
use conduit_web::{HttpBody, HttpMethod, HttpRequest, HttpTarget, HttpTransactionId};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

#[test]
fn exact_endpoint_plan_executes_without_redirect_or_sibling_authority() {
    let exact = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let exact_address = exact.local_addr().unwrap();
    let sibling = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    sibling.set_nonblocking(true).unwrap();
    let sibling_address = sibling.local_addr().unwrap();
    let alternate_ip = TcpListener::bind((Ipv4Addr::new(127, 0, 0, 2), 0)).unwrap();
    alternate_ip.set_nonblocking(true).unwrap();
    let alternate_ip_address = alternate_ip.local_addr().unwrap();
    let authority = format!("fixture.invalid:{}", exact_address.port());
    let provider = config(exact_address, authority.clone(), 1, "endpoint/generation/1");
    let mut host = IsolatedHttpHost::new(
        StdHostConfig {
            host_id: HostId::from("host/isolated-http"),
            boot_id: BootId::from("boot/isolated-http/current"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        provider.clone(),
    )
    .unwrap();
    let (form, plan) = plan(&host, &authority);
    assert!(form
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == conduit_web::HTTP_CLIENT_KIND));
    let fragment = &plan.fragments[0];
    assert!(fragment
        .placements
        .iter()
        .any(|placement| { placement.implementation_id.as_str() == ISOLATED_HTTP_IMPLEMENTATION }));

    let server = thread::spawn(move || {
        let mut observed_requests = 0_u8;
        for sequence in 0..4 {
            let (mut stream, _) = exact.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            if sequence == 0 {
                let request = read_http_message(&mut stream).unwrap();
                assert!(request.starts_with(b"GET /selected HTTP/1.1\r\n"));
                assert!(request
                    .windows(authority.len() + 6)
                    .any(|window| window == format!("Host: {authority}").as_bytes()));
                observed_requests += 1;
                write!(
                    stream,
                    "HTTP/1.1 302 Found\r\nLocation: http://{sibling_address}/escape\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
            } else {
                let mut bytes = Vec::new();
                stream.read_to_end(&mut bytes).unwrap();
                assert!(bytes.is_empty(), "negative provider emitted HTTP bytes");
            }
        }
        observed_requests
    });

    let exact_request = request(&provider.authority, "/selected");
    let play = host.host_mut().issue_kernel_play(fragment).unwrap();
    let receipt = host.exchange(play, fragment, &exact_request).unwrap();
    assert_eq!(receipt.status, 302);
    assert!(receipt.response_bytes <= conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES);

    let inspection = host.inspection();
    assert_eq!(inspection.endpoint, exact_address);
    assert_eq!(inspection.authority, provider.authority);
    assert_eq!(
        inspection.enforcement_class,
        conduit_core::BaseEnforcementClass::OsCapabilityMediated
    );

    let negative_play = host.host_mut().issue_kernel_play(fragment).unwrap();
    let report = run_adversarial_http_proof(
        &provider,
        fragment,
        &negative_play,
        &exact_request,
        &sibling_address.to_string(),
    )
    .unwrap();
    assert!(report.forged_scope_refused);
    assert!(report.wrong_authority_refused_at_provider);
    assert!(report.stale_generation_refused_after_restart);
    assert!(report.revoked_handle_refused);
    assert_eq!(report.raw_socket_errno, libc::EPERM);
    assert_eq!(report.listener_errno, libc::EPERM);
    assert_eq!(report.process_spawn_errno, libc::EPERM);
    assert_eq!(server.join().unwrap(), 1);

    let outside = request(&sibling_address.to_string(), "/escape");
    let rejected_play = host.host_mut().issue_kernel_play(fragment).unwrap();
    assert!(host.exchange(rejected_play, fragment, &outside).is_err());
    assert!(matches!(
        sibling.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
    let alternate = request(&alternate_ip_address.to_string(), "/alternate-ip");
    let alternate_play = host.host_mut().issue_kernel_play(fragment).unwrap();
    assert!(host.exchange(alternate_play, fragment, &alternate).is_err());
    assert!(matches!(
        alternate_ip.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
}

fn config(
    endpoint: std::net::SocketAddr,
    authority: String,
    generation: u64,
    resource_generation: &str,
) -> IsolatedHttpBaseConfig {
    IsolatedHttpBaseConfig {
        executable: PathBuf::from(env!("CARGO_BIN_EXE_conduit-isolated-http-base")),
        base_instance_id: BaseInstanceId::from("base/http/exact-endpoint"),
        provider_generation: generation,
        authority,
        endpoint,
        resource_generation_id: ResourceGenerationId(resource_generation.into()),
    }
}

fn plan(
    host: &IsolatedHttpHost,
    _authority: &str,
) -> (conduit_form::CheckedForm, conduit_core::Plan) {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut catalog = conduit_form::ProfileCatalog::new();
    conduit_web::install_http_catalogs(&mut startup, &mut catalog).unwrap();
    let form = conduit_form::parse(
        "form proxy {\n server: http/server\n client: http/client\n server.request > client.request\n client.response > server.response\n}\n",
        &catalog,
    )
    .unwrap();
    let grants = host
        .host()
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
                        "grant/{}/{index}",
                        capability.capability_id.as_str()
                    )),
                    contract_id: requirement.contract_id.clone(),
                    host_operation_contract_id: requirement.host_operation_contract_id.clone(),
                    subject_kind: requirement.subject_kind.clone(),
                    host_id: host.host().advertisement().host_id.clone(),
                    boot_id: host.host().advertisement().boot_id.clone(),
                    capability_id: capability.capability_id.clone(),
                })
        })
        .collect::<Vec<_>>();
    let hosts = [host.host().advertisement().clone()];
    let placements = conduit_planner::default_placements(&form, &hosts).unwrap();
    let plan = conduit_planner::plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
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
    (form, plan)
}

fn request(authority: &str, path: &str) -> HttpRequest {
    HttpRequest {
        transaction_id: HttpTransactionId(7),
        method: HttpMethod::Get,
        target: HttpTarget {
            scheme: "http".into(),
            authority: authority.into(),
            path_and_query: path.into(),
        },
        headers: Vec::new(),
        body: HttpBody::inline(Vec::new()),
    }
}

fn read_http_message(stream: &mut impl Read) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0_u8; 1];
        stream
            .read_exact(&mut byte)
            .map_err(|error| error.to_string())?;
        bytes.push(byte[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            return Ok(bytes);
        }
        if bytes.len() > conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES as usize {
            return Err("request exceeded bound".into());
        }
    }
}
