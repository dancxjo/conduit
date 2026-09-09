use conduit_core::*;
use std::fs;
use std::io::{self, Write};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct WifiBroadcastProvider {
    socket: UdpSocket,
    payload: Vec<u8>,
    calls: u32,
    safe: bool,
}

impl ConsequentialEffectProvider for WifiBroadcastProvider {
    fn apply(
        &mut self,
        request: &ConsequentialRequest,
    ) -> Result<PhysicalObservation, ConsequentialProviderFailure> {
        if self.calls != 0
            || request.resource_id != "wifi/bounded-broadcast"
            || request.operation_id != "transmit-one-datagram"
            || request.magnitude != 1
            || request.duration_ticks != 1
            || request.rate != 1
        {
            return Err(ConsequentialProviderFailure::Refused);
        }
        self.calls = 1;
        self.socket
            .send(&self.payload)
            .map_err(|_| ConsequentialProviderFailure::Ambiguous)?;
        Ok(PhysicalObservation::Observed)
    }

    fn safe_disposition(&mut self, disposition: &str) -> Result<(), ConsequentialProviderFailure> {
        if disposition != "no-further-transmission" {
            return Err(ConsequentialProviderFailure::Refused);
        }
        self.safe = true;
        Ok(())
    }
}

fn main() -> Result<(), String> {
    let interface = std::env::var("CONDUIT_PHYSICAL_INTERFACE")
        .map_err(|_| "set CONDUIT_PHYSICAL_INTERFACE to the attended network interface")?;
    let endpoint: SocketAddr = std::env::var("CONDUIT_PHYSICAL_BROADCAST")
        .map_err(|_| {
            "set CONDUIT_PHYSICAL_BROADCAST to the exact attended IPv4 broadcast endpoint"
        })?
        .parse()
        .map_err(|_| "CONDUIT_PHYSICAL_BROADCAST is not an exact socket address")?;
    if !endpoint.ip().is_ipv4() || endpoint.ip().is_loopback() || endpoint.port() == 0 {
        return Err("physical proof requires a non-loopback IPv4 broadcast endpoint".into());
    }
    let operstate = fs::read_to_string(format!("/sys/class/net/{interface}/operstate"))
        .map_err(|error| format!("read interface state: {error}"))?;
    if operstate.trim() != "up" {
        return Err("attended physical interface is not up".into());
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock precedes the proof epoch")?
        .as_nanos();
    let phrase = format!("TRANSMIT ONCE {interface} {endpoint} {nonce:032x}");
    eprintln!("Attended low-energy physical proof");
    eprintln!("This sends one 40-byte UDP broadcast and never retries.");
    eprintln!("Type exactly: {phrase}");
    eprint!("> ");
    io::stderr().flush().map_err(|error| error.to_string())?;
    let mut approval = String::new();
    io::stdin()
        .read_line(&mut approval)
        .map_err(|error| format!("read attended approval: {error}"))?;
    if approval.trim() != phrase {
        return Err("fresh attended approval did not match the exact effect envelope".into());
    }

    let listener = UdpSocket::bind(("0.0.0.0", endpoint.port()))
        .map_err(|error| format!("bind independent observer: {error}"))?;
    listener
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;
    let socket = UdpSocket::bind(("0.0.0.0", 0))
        .map_err(|error| format!("bind effect provider: {error}"))?;
    socket
        .set_broadcast(true)
        .map_err(|error| format!("admit broadcast provider: {error}"))?;
    socket
        .connect(endpoint)
        .map_err(|error| format!("select attended interface route: {error}"))?;
    let provider_address = socket
        .local_addr()
        .map_err(|error| format!("inspect attended interface route: {error}"))?;
    let payload = format!("conduit-{nonce:032x}").into_bytes();
    let mut provider = WifiBroadcastProvider {
        socket,
        payload: payload.clone(),
        calls: 0,
        safe: false,
    };
    let (mut gate, attendance) = prepare_gate(100, 110)?;
    let request = request();
    let safety = readiness();

    if gate.attempt(&mut provider, 101, None, &safety, &request)
        != ConsequentialDisposition::Refused(ConsequentialRefusal::MissingAttendance)
        || provider.calls != 0
    {
        return Err("missing attendance reached the physical provider".into());
    }
    let mut excessive = request.clone();
    excessive.magnitude = 2;
    if gate.attempt(&mut provider, 101, Some(&attendance), &safety, &excessive)
        != ConsequentialDisposition::Refused(ConsequentialRefusal::EnvelopeExceeded)
        || provider.calls != 0
    {
        return Err("excessive request reached the physical provider".into());
    }

    let before = tx_packets(&interface)?;
    if gate.attempt(&mut provider, 101, Some(&attendance), &safety, &request)
        != ConsequentialDisposition::PhysicalEffectObserved
    {
        return Err("exact attended physical effect was not admitted".into());
    }
    let mut observed = [0_u8; 128];
    let (length, source) = listener
        .recv_from(&mut observed)
        .map_err(|error| format!("independent broadcast observer: {error}"))?;
    if observed[..length] != payload {
        return Err(format!(
            "independent socket observed a different datagram: expected {} bytes, received {length}",
            payload.len()
        ));
    }
    if source.ip() != provider_address.ip() {
        return Err(format!(
            "independent socket observed an unexpected source: expected {}, received {}",
            provider_address.ip(),
            source.ip()
        ));
    }
    let after = wait_for_tx_increment(&interface, before)?;

    let (mut revoked, revoked_attendance) = prepare_gate(200, 210)?;
    revoked
        .capability_table
        .revoke(&revoked.capability_handle)
        .map_err(|error| format!("revoke proof capability: {error:?}"))?;
    if revoked.attempt(
        &mut provider,
        201,
        Some(&revoked_attendance),
        &safety,
        &request,
    ) != ConsequentialDisposition::Refused(ConsequentialRefusal::Capability(
        BaseCapabilityRefusal::Revoked,
    )) || provider.calls != 1
    {
        return Err("revoked possession reached the physical provider".into());
    }
    if gate.lose(&mut provider) != ConsequentialDisposition::Safe || !provider.safe {
        return Err("provider loss did not reach its explicit safe disposition".into());
    }
    println!(
        "PROVED attended physical effect interface={interface} endpoint={endpoint} bytes={} tx_delta={} unauthorized_effects=0 retries=0 safe=true",
        payload.len(),
        after - before
    );
    Ok(())
}

fn tx_packets(interface: &str) -> Result<u64, String> {
    fs::read_to_string(format!("/sys/class/net/{interface}/statistics/tx_packets"))
        .map_err(|error| format!("read NIC transmit observer: {error}"))?
        .trim()
        .parse()
        .map_err(|_| "NIC transmit observer is malformed".into())
}

fn wait_for_tx_increment(interface: &str, before: u64) -> Result<u64, String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let after = tx_packets(interface)?;
        if after > before {
            return Ok(after);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "NIC transmit observer did not advance from {before} after the one attempted effect"
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn request() -> ConsequentialRequest {
    ConsequentialRequest {
        resource_id: "wifi/bounded-broadcast".into(),
        resource_generation: 1,
        operation_id: "transmit-one-datagram".into(),
        magnitude: 1,
        duration_ticks: 1,
        rate: 1,
    }
}

fn readiness() -> SafetyReadiness {
    SafetyReadiness {
        resource_id: "wifi/bounded-broadcast".into(),
        resource_generation: 1,
        observation_generation: 1,
        ready: true,
    }
}

fn prepare_gate(
    valid_from: u64,
    valid_until: u64,
) -> Result<(ConsequentialEffectGate, AttendanceHandle), String> {
    let operation = HostOperationContractId::from("conduit.host/physical-datagram@1");
    let scope = BaseCapabilityScope {
        host_id: HostId::from("host/victus"),
        boot_id: BootId::from("boot/attended-proof"),
        base_instance_id: BaseInstanceId::from("base/wifi-physical"),
        base_provider_generation: 1,
        plan_id: PlanId::from("plan/physical-proof"),
        active_play_id: ActivePlayId::from("play/physical-proof"),
        authority_grant_id: AuthorityGrantId::from("grant/attended-physical-proof"),
        authority_contract_id: AuthorityContractId::from("authority/attended-physical@1"),
        capability_id: CapabilityId::from("network/transmit-one"),
        implementation_id: ImplementationId::from("std/wifi-broadcast-base@1"),
        operation_contract_id: operation.clone(),
        subject_kind: KindId::from("network/datagram"),
        resource_pool_id: ResourcePoolId::from("wifi/bounded-broadcast"),
        resource_generation_id: ResourceGenerationId("wifi/generation-1".into()),
        envelope_id: CapabilityEnvelopeId::from("datagram/one/max-64"),
        maximum_parameter_bytes: 24,
        maximum_result_bytes: 1,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 1,
    };
    let issue = CapabilityIssueRequest {
        authority: BaseCapabilityAuthority {
            grant: AuthorityGrant {
                grant_id: scope.authority_grant_id.clone(),
                contract_id: scope.authority_contract_id.clone(),
                host_operation_contract_id: operation.clone(),
                subject_kind: scope.subject_kind.clone(),
                host_id: scope.host_id.clone(),
                boot_id: scope.boot_id.clone(),
                capability_id: scope.capability_id.clone(),
            },
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: 1,
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            operation_contract_id: operation,
            envelope_id: scope.envelope_id.clone(),
            maximum_parameter_bytes: 24,
            maximum_result_bytes: 1,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 1,
        },
        scope,
    };
    let scope = &issue.scope;
    let claim = BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: 1,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: 24,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        1,
        [11; 32],
        1,
    )
    .map_err(|error| format!("capability table: {error:?}"))?;
    let capability_handle = table
        .issue(issue)
        .map_err(|error| format!("issue capability: {error:?}"))?;
    let (attendance, handle) = AttendanceGrant::issue(
        [12; 32],
        valid_from,
        format!("attendance/{valid_from}"),
        "wifi/bounded-broadcast".into(),
        1,
        "transmit-one-datagram".into(),
        valid_from,
        valid_until,
    )
    .map_err(|error| format!("issue attendance: {error:?}"))?;
    let gate = ConsequentialEffectGate::new(
        ConsequentialProfile {
            profile_id: "consequence/attended-low-energy-network@1".into(),
            requires_attendance: true,
            envelope: ConsequentialEnvelope {
                maximum_magnitude: 1,
                maximum_duration_ticks: 1,
                maximum_rate: 1,
            },
            safe_disposition: "no-further-transmission".into(),
        },
        table,
        capability_handle,
        claim,
        Some(attendance),
        "wifi/bounded-broadcast".into(),
        1,
        1,
    )
    .map_err(|error| format!("prepare physical gate: {error:?}"))?;
    Ok((gate, handle))
}
