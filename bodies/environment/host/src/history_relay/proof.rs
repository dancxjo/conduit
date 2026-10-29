//! Explicit process proof roles. All identities and measurements belong to a
//! separate test construction; these roles never load physical Body credentials.
use super::*;
use crate::history_relay_kernel::HistoryKernel;
use conduit_core::*;
use conduit_environment_runtime::history_exchange::{
    self as h, codec, HistoryProvider, HistoryProviderRefusal,
};
use conduit_protected_line::{
    self as p, establish_protected_session, EndpointBinding, ProtectedCarrier,
    ProtectedSessionPolicy, Role, SessionLimits,
};
use conduit_std_host::relay_client::{HostedRelayCarrier, RelayClientDescriptor};
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    address: SocketAddr,
    url: String,
    server_identity: String,
    certificate_binding: [u8; 32],
    route: String,
    negotiation: String,
    session: String,
    source_capability: [u8; 32],
    collector_capability: [u8; 32],
    psk: [u8; 32],
    test_signing_seed: [u8; 32],
    body: String,
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn signed(key: &SigningKey, scope: [u8; 96], forms: [u8; 32], after: u32) -> Vec<u8> {
    let records = if after == 0 {
        conduit_environment_runtime::observed_record::ObservedRecord {
            sequence: 1,
            observed_at_boot_ms: 1234,
            disposition:
                conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
            quantities: None,
        }
        .encode()
        .unwrap()
        .to_vec()
    } else {
        vec![]
    };
    let signature = key
        .sign(&crate::observed_history::wire::transcript(
            &scope, &forms, after, 1, &records,
        ))
        .to_bytes();
    serde_json::to_vec(&serde_json::json!({"profile":"observed-history@1","scope":hex(&scope),"forms":hex(&forms),"after":after,"committed":1,"records":hex(&records),"signature":hex(&signature)})).unwrap()
}
#[derive(Clone)]
struct Archive {
    zero: Vec<u8>,
    end: Vec<u8>,
}
impl HistoryProvider for Archive {
    fn read_signed_batch(
        &mut self,
        cursor: u32,
        out: &mut [u8; 2048],
    ) -> std::result::Result<usize, HistoryProviderRefusal> {
        let bytes = match cursor {
            0 => &self.zero,
            1 => &self.end,
            _ => return Err(HistoryProviderRefusal::InvalidCursor),
        };
        out[..bytes.len()].copy_from_slice(bytes);
        Ok(bytes.len())
    }
    fn revoke(&mut self) {}
}
fn line_offer(source: &str, sink: &str, label: &str, observed: bool) -> LineOffer {
    let id = LineId::from(format!("proof/history/{label}"));
    let binding = LinkBindingId::from(format!("proof/history/{label}/binding"));
    LineOffer {
        line_id: id.clone(),
        binding: LinkBinding {
            binding_id: binding.clone(),
            source: LinkEndpoint {
                host_id: format!("host/{source}").into(),
                boot_id: format!("boot/{source}").into(),
                endpoint_id: format!("endpoint/{source}/{label}").into(),
            },
            sink: LinkEndpoint {
                host_id: format!("host/{sink}").into(),
                boot_id: format!("boot/{sink}").into(),
                endpoint_id: format!("endpoint/{sink}/{label}").into(),
            },
            base: p::IMPLEMENTATION_ID.into(),
            base_instance_id: "proof/history/protected-relay".into(),
            credential: LinkCredentialReference::Opaque("proof/history/relay-credential".into()),
            authority: LinkAuthorityReference::ProcessOwned,
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 2304,
                maximum_buffered_bytes: 2304,
                maximum_frame_bytes: 4096,
            },
        },
        contract: LineContract {
            scope: LineScope::RoutedNetwork,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::AuthenticatedEncrypted,
        },
        availability: LineAvailabilitySign {
            line_id: id,
            binding_id: binding,
            availability: if observed {
                LineAvailability::Ready
            } else {
                LineAvailability::Unavailable
            },
            sign_id: format!("proof/history/{label}/attachment-observed").into(),
        },
    }
}
fn connect(
    config: &Configuration,
    collector: bool,
) -> Result<ProtectedCarrier<HostedRelayCarrier>> {
    let mut carrier = checked(HostedRelayCarrier::connect(RelayClientDescriptor {
        address: config.address,
        public_url: config.url.clone(),
        server_identity: config.server_identity.clone(),
        certificate_binding_sha256: config.certificate_binding,
        route_id: config.route.clone(),
        role: if collector {
            p::RelayEndpointRole::First
        } else {
            p::RelayEndpointRole::Second
        },
        endpoint_binding: if collector {
            "host/collector/boot/collector"
        } else {
            "host/source/boot/source"
        }
        .into(),
        capability: if collector {
            config.collector_capability
        } else {
            config.source_capability
        },
        maximum_protected_frame_bytes: 4130,
        timeout_millis: 10000,
    }))?;
    let binding = p::SessionBinding {
        initiator: EndpointBinding {
            host_id: "host/collector".into(),
            boot_id: "boot/collector".into(),
        },
        responder: EndpointBinding {
            host_id: "host/source".into(),
            boot_id: "boot/source".into(),
        },
        negotiation_id: config.negotiation.clone(),
        line_session_id: config.session.clone(),
        candidate_binding: config.route.clone(),
        transport_binding: format!("relay/wss/{}", hex(&config.certificate_binding)),
    };
    let policy = ProtectedSessionPolicy {
        traffic: SessionLimits {
            maximum_payload_bytes: 4096,
            maximum_frames_per_direction: 4096,
            maximum_bytes_per_direction: 16 * 1024 * 1024,
        },
        maximum_simultaneous_sessions: 1,
        maximum_pending_frames_per_session: 1,
        handshake_work_units: 2,
        handshake_timeout_millis: 10000,
        idle_timeout_millis: 10000,
    };
    let session = checked(establish_protected_session(
        &mut carrier,
        if collector {
            Role::Initiator
        } else {
            Role::Responder
        },
        &binding,
        policy,
        config.psk,
        crate::admission::random()?,
    ))?;
    checked(ProtectedCarrier::new(carrier, session, policy))
}
fn hold_boundary(directory: &Path, boundary: &str) -> Result<()> {
    if std::env::var("CONDUIT_HISTORY_RELAY_PROOF_HOLD")
        .ok()
        .as_deref()
        != Some(boundary)
    {
        return Ok(());
    }
    std::fs::write(directory.join(format!("held-{boundary}")), boundary)?;
    println!("history-relay-held boundary={boundary}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        if directory.join(format!("release-{boundary}")).exists() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Err("proof hold exceeded explicit thirty-second deadline".into())
}
fn run_role(directory: &Path, role: &str) -> Result<()> {
    let configuration: Configuration =
        serde_json::from_slice(&std::fs::read(directory.join("configuration.json"))?)?;
    let collector_role = role == "collector";
    let line = connect(&configuration, collector_role)?;
    // Ready is observed only after both actual outbound attachments and their
    // protected handshake have succeeded; the planner cannot invent the route.
    let mut lines = [
        line_offer("collector", "source", "request", true),
        line_offer("source", "collector", "response", true),
    ];
    for line in &mut lines {
        line.binding.base_instance_id = format!("proof/history/{}", configuration.session).into();
    }
    let key = SigningKey::from_bytes(&configuration.test_signing_seed);
    let mut scope = [0; 96];
    scope[..32].copy_from_slice(&Sha256::digest(configuration.body.as_bytes()));
    scope[32..64].copy_from_slice(&Sha256::digest(b"boot/source"));
    scope[64..].copy_from_slice(&Sha256::digest(b"proof/source/observation-play"));
    let contract = h::HistoryReadContract::prepare().map_err(|e| format!("{e:?}"))?;
    let mut collector = crate::observed_history::Collector::from_authenticated_scope(
        scope,
        &key.verifying_key().to_bytes(),
    )?;
    let forms = Sha256::digest(
        conduit_environment_runtime::observed_record::descriptor(
            &conduit_environment_runtime::observation::ObservationContract::prepare()
                .map_err(|e| format!("{e:?}"))?,
        )
        .map_err(|e| format!("{e:?}"))?,
    )
    .into();
    let archive = Archive {
        zero: signed(&key, scope, forms, 0),
        end: signed(&key, scope, forms, 1),
    };
    let prepared = crate::history_relay_plan::prepare_proof_plan(archive, &lines)?;
    let connections: Vec<_> = prepared
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.connections)
        .collect();
    let request = connections
        .iter()
        .find(|c| c.sink_placement_id == prepared.provider_placement)
        .ok_or("history request Cord")?
        .connection_id
        .clone();
    let response = connections
        .iter()
        .find(|c| c.source_placement_id == prepared.provider_placement)
        .ok_or("history response Cord")?
        .connection_id
        .clone();
    let mut relay =
        HistoryRelay::from_plan(line, &prepared.plan, &request, &response, collector_role)?;
    relay.activate()?;
    println!("history-relay-{role}-active");
    if collector_role {
        let history = directory.join("history");
        collector.collect_from(&history, 0, |cursor| {
            println!("history-relay-request cursor={cursor}");
            if cursor == 1 {
                hold_boundary(directory, "after-commit")?;
            }
            let request = checked(codec::encode_request(&contract, cursor))?;
            let canonical = relay.read(&request)?;
            hold_boundary(directory, "before-commit")?;
            Ok(checked(codec::decode_result(&contract, &canonical))?.to_vec())
        })?;
        println!("history-relay-durable-prefix after={}", collector.after);
        relay.close()?;
    } else {
        let fragment = prepared
            .plan
            .fragments
            .iter()
            .find(|f| {
                f.placements
                    .iter()
                    .any(|g| g.placement_id == prepared.provider_placement)
            })
            .ok_or("provider fragment")?;
        let (lowered, _) = checked(
            conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
                &prepared.plan,
                &fragment.fragment_id,
            ),
        )?;
        let active = bind_active_play(
            &prepared.plan.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            0,
        );
        let mut table = checked(BaseCapabilityTable::new(
            prepared.scope.host_id.clone(),
            prepared.scope.boot_id.clone(),
            prepared.scope.base_instance_id.clone(),
            prepared.scope.base_provider_generation,
            crate::admission::random()?,
            1,
        ))?;
        let handle = checked(table.issue(CapabilityIssueRequest {
            scope: prepared.scope,
            authority: prepared.authority,
        }))?;
        let execution = checked(h::SelectedHistoryExecution::prepare(
            prepared.base,
            table,
            handle,
            prepared.claim,
            h::HistoryCallSelection {
                contract: &contract,
                fragment,
                lowered: &lowered,
                active: &active,
                placement: &prepared.provider_placement,
            },
        ))?;
        let (back, owner) = execution.into_parts();
        let mut kernel = HistoryKernel::prepare(&lowered, back, owner)?;
        let mut completed = 0;
        for _ in 0..240 {
            match relay.serve_one(|request| kernel.dispatch(request)) {
                Ok(()) => {
                    kernel.delivered()?;
                    completed += 1;
                }
                Err(error) => {
                    println!("history-relay-source-ended {error}");
                    if completed < 2 {
                        kernel.cancel()?;
                        return Err(error);
                    }
                    break;
                }
            }
        }
        kernel.cancel()?;
    }
    Ok(())
}
/// Process entrance used by the Docker/NAT proof runner, never an instrument
/// provisioning path. Configuration is an explicitly separate private fixture.
#[test]
#[ignore = "requires an explicitly provisioned two-endpoint relay fixture"]
fn outbound_process_role() {
    let directory = PathBuf::from(
        std::env::var_os("CONDUIT_HISTORY_RELAY_PROOF_DIRECTORY").expect("private proof directory"),
    );
    let role = std::env::var("CONDUIT_HISTORY_RELAY_PROOF_ROLE").expect("source or collector");
    assert!(matches!(role.as_str(), "source" | "collector"));
    run_role(&directory, &role).unwrap();
}

#[test]
fn checked_history_plan_selects_two_exact_remote_cords() {
    let lines = [
        line_offer("collector", "source", "request", true),
        line_offer("source", "collector", "response", true),
    ];
    let prepared = crate::history_relay_plan::prepare_proof_plan(
        Archive {
            zero: vec![],
            end: vec![],
        },
        &lines,
    )
    .unwrap();
    assert!(conduit_core::verify_plan(&prepared.plan));
    assert_eq!(prepared.plan.fragments.len(), 2);
    let source = prepared
        .plan
        .fragments
        .iter()
        .find(|f| f.host_id.as_str() == "host/source")
        .unwrap();
    assert_eq!(source.placements.len(), 1);
    assert_eq!(source.connections.len(), 2);
}

#[path = "local_binding.rs"]
mod local_binding;

#[path = "queue_pressure.rs"]
mod queue_pressure;
