//! Bounded network attachment, lease, reachability, and DNS-SD semantics.
//!
//! TCP/UDP transport belongs to `conduit-socket`; HTTP, SSE, and WebSocket
//! application semantics belong to `conduit-http`. This package neither opens
//! sockets nor grants enrollment, possession, service, safety, or motion
//! authority. A Pico W implementation is one optional witness of these
//! contracts, never their semantic identity.

mod runtime_nodes;
mod standing;
mod types;

pub use runtime_nodes::{
    DHCP_SERVER_CONTRACT, DNS_SD_CONTRACT, NETWORK_CONTRACTS, REACHABILITY_CONTRACT,
    WIFI_AP_CONTRACT, register_deterministic_network_providers, register_network_contracts,
};
pub use standing::{
    EXECUTABLE_STANDING_NETWORK_CONTRACTS, LINK_OBSERVE_CONTRACT,
    NATIVE_USERSPACE_ROUTE_ARTIFACT_ID, NATIVE_USERSPACE_ROUTE_IMPLEMENTATION_ID,
    NETWORK_EFFECT_CONTRACTS, NETWORK_METER_CONTRACT, PACKET_CLASSIFY_CONTRACT,
    PACKET_ROUTE_CONTRACT, PACKET_SINK_CONTRACT, PACKET_SOURCE_CONTRACT, SERVICE_OBSERVE_CONTRACT,
    SESSION_LISTEN_CONTRACT, STANDING_NETWORK_CONTRACTS,
    install_native_userspace_route_implementation, native_userspace_route_capability_requirement,
    register_deterministic_standing_network_providers, register_standing_network_contracts,
};
pub use types::*;

pub const MAXIMUM_CLIENTS: usize = 8;
pub const MAXIMUM_NAME_BYTES: usize = 63;
pub const MAXIMUM_RECORDS: usize = 8;
pub const MAXIMUM_PORTS: usize = 8;
pub const MAXIMUM_PACKET_BYTES: usize = 1_500;
pub const MAXIMUM_EVIDENCE_EVENTS: usize = 64;
pub const DHCP_LEASE_TICKS: u64 = 3_600_000;
pub const ICMP_WINDOW_TICKS: u64 = 1_000;
pub const ICMP_PACKETS_PER_WINDOW: u8 = 4;

pub const AP_ADDRESS: Ipv4Address = Ipv4Address([192, 168, 4, 1]);
pub const DHCP_FIRST_ADDRESS: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ipv4Address(pub [u8; 4]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkReason {
    ApUnavailable,
    Cyw43InitializationFailed,
    ProviderLost,
    ObservationStale,
    PoolExhausted,
    MalformedPacket,
    PacketTooLarge,
    RateLimited,
    NameConflict,
    NameTooLong,
    RecordTableFull,
    PortConflict,
    PortTableFull,
    WrongDevice,
    WrongBoot,
    LeaseMissing,
    LeaseExpired,
    LeaseGenerationMismatch,
    RegistrationStale,
    Cancelled,
    EvidenceFull,
    RoutingForbidden,
    NoRoute,
    HopExhausted,
    MtuExceeded,
    ForwardingDenied,
    RouteTableFull,
    RouteInvalid,
    SessionTableFull,
    SessionMissing,
    StaleGeneration,
    Bounds,
    Unsupported,
    InvalidTopology,
}

impl NetworkReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ApUnavailable | Self::Cyw43InitializationFailed => "CND-NET-001",
            Self::ProviderLost | Self::ObservationStale => "CND-NET-002",
            Self::PoolExhausted => "CND-NET-003",
            Self::MalformedPacket | Self::PacketTooLarge => "CND-NET-004",
            Self::RateLimited => "CND-NET-005",
            Self::NameConflict | Self::NameTooLong | Self::RecordTableFull => "CND-NET-006",
            Self::PortConflict | Self::PortTableFull => "CND-NET-007",
            Self::WrongDevice
            | Self::WrongBoot
            | Self::LeaseMissing
            | Self::LeaseExpired
            | Self::LeaseGenerationMismatch
            | Self::RegistrationStale => "CND-NET-008",
            Self::Cancelled => "CND-NET-009",
            Self::EvidenceFull => "CND-NET-010",
            Self::RoutingForbidden => "CND-NET-011",
            Self::NoRoute => "CND-NET-012",
            Self::HopExhausted => "CND-NET-013",
            Self::MtuExceeded => "CND-NET-014",
            Self::ForwardingDenied => "CND-NET-015",
            Self::RouteTableFull | Self::RouteInvalid => "CND-NET-016",
            Self::SessionTableFull | Self::SessionMissing | Self::StaleGeneration => "CND-NET-017",
            Self::Bounds => "CND-NET-018",
            Self::Unsupported => "CND-NET-019",
            Self::InvalidTopology => "CND-NET-020",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientIdentity {
    bytes: [u8; 32],
    length: u8,
}

impl ClientIdentity {
    pub fn new(value: &[u8]) -> Result<Self, NetworkReason> {
        if value.is_empty() || value.len() > 32 {
            return Err(NetworkReason::MalformedPacket);
        }
        let mut bytes = [0; 32];
        bytes[..value.len()].copy_from_slice(value);
        Ok(Self {
            bytes,
            length: value.len() as u8,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DhcpLease {
    pub client: ClientIdentity,
    pub address: Ipv4Address,
    pub generation: u32,
    pub expires_at_tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DhcpMessage {
    Discover,
    Renew,
    Release,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DhcpOutcome {
    Offered(DhcpLease),
    Acknowledged(DhcpLease),
    Released,
}

/// Fixed eight-slot lease state. Time is caller supplied and no allocation or
/// ambient clock is used.
pub struct DhcpLeaseTable {
    leases: [Option<DhcpLease>; MAXIMUM_CLIENTS],
    next_generation: u32,
}

impl Default for DhcpLeaseTable {
    fn default() -> Self {
        Self::new()
    }
}

impl DhcpLeaseTable {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            leases: [None; MAXIMUM_CLIENTS],
            next_generation: 1,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.leases.iter().flatten().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn handle(
        &mut self,
        message: DhcpMessage,
        client: ClientIdentity,
        packet_bytes: usize,
        now_tick: u64,
    ) -> Result<DhcpOutcome, NetworkReason> {
        if packet_bytes == 0 {
            return Err(NetworkReason::MalformedPacket);
        }
        if packet_bytes > MAXIMUM_PACKET_BYTES {
            return Err(NetworkReason::PacketTooLarge);
        }
        self.expire(now_tick);
        if message == DhcpMessage::Release {
            let slot = self
                .leases
                .iter_mut()
                .find(|lease| lease.is_some_and(|lease| lease.client == client))
                .ok_or(NetworkReason::LeaseMissing)?;
            *slot = None;
            return Ok(DhcpOutcome::Released);
        }
        if let Some(lease) = self
            .leases
            .iter_mut()
            .flatten()
            .find(|lease| lease.client == client)
        {
            lease.expires_at_tick = now_tick
                .checked_add(DHCP_LEASE_TICKS)
                .ok_or(NetworkReason::Bounds)?;
            return Ok(match message {
                DhcpMessage::Discover => DhcpOutcome::Offered(*lease),
                DhcpMessage::Renew => DhcpOutcome::Acknowledged(*lease),
                DhcpMessage::Release => unreachable!(),
            });
        }
        let slot_index = self
            .leases
            .iter()
            .position(Option::is_none)
            .ok_or(NetworkReason::PoolExhausted)?;
        let generation = self.next_generation;
        let next_generation = generation.checked_add(1).ok_or(NetworkReason::Bounds)?;
        let expires_at_tick = now_tick
            .checked_add(DHCP_LEASE_TICKS)
            .ok_or(NetworkReason::Bounds)?;
        let lease = DhcpLease {
            client,
            address: Ipv4Address([192, 168, 4, DHCP_FIRST_ADDRESS + slot_index as u8]),
            generation,
            expires_at_tick,
        };
        self.next_generation = next_generation;
        self.leases[slot_index] = Some(lease);
        Ok(match message {
            DhcpMessage::Discover => DhcpOutcome::Offered(lease),
            DhcpMessage::Renew => DhcpOutcome::Acknowledged(lease),
            DhcpMessage::Release => unreachable!(),
        })
    }

    pub fn lease_for(&mut self, client: ClientIdentity, now_tick: u64) -> Option<DhcpLease> {
        self.expire(now_tick);
        self.leases
            .iter()
            .flatten()
            .copied()
            .find(|lease| lease.client == client)
    }

    pub fn reboot(&mut self) -> Result<(), NetworkReason> {
        let next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(NetworkReason::Bounds)?;
        self.leases = [None; MAXIMUM_CLIENTS];
        self.next_generation = next_generation;
        Ok(())
    }

    fn expire(&mut self, now_tick: u64) {
        for lease in &mut self.leases {
            if lease.is_some_and(|lease| now_tick >= lease.expires_at_tick) {
                *lease = None;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IcmpRateLimiter {
    window_started_at: u64,
    packets: u8,
}

impl IcmpRateLimiter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            window_started_at: 0,
            packets: 0,
        }
    }

    pub fn admit(&mut self, packet_bytes: usize, now_tick: u64) -> Result<(), NetworkReason> {
        if packet_bytes == 0 || packet_bytes > MAXIMUM_PACKET_BYTES {
            return Err(if packet_bytes == 0 {
                NetworkReason::MalformedPacket
            } else {
                NetworkReason::PacketTooLarge
            });
        }
        let elapsed = now_tick
            .checked_sub(self.window_started_at)
            .ok_or(NetworkReason::Bounds)?;
        if elapsed >= ICMP_WINDOW_TICKS {
            self.window_started_at = now_tick;
            self.packets = 0;
        }
        if self.packets >= ICMP_PACKETS_PER_WINDOW {
            return Err(NetworkReason::RateLimited);
        }
        self.packets += 1;
        Ok(())
    }
}

impl Default for IcmpRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceName {
    bytes: [u8; MAXIMUM_NAME_BYTES],
    length: u8,
}

impl ServiceName {
    pub fn new(value: &str) -> Result<Self, NetworkReason> {
        if value.is_empty() || value.len() > MAXIMUM_NAME_BYTES || !value.is_ascii() {
            return Err(NetworkReason::NameTooLong);
        }
        let mut bytes = [0; MAXIMUM_NAME_BYTES];
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        Ok(Self {
            bytes,
            length: value.len() as u8,
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.length as usize]).expect("ASCII name")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DnsSdRecord {
    pub name: ServiceName,
    pub address: Ipv4Address,
    pub generation: u32,
    pub expires_at_tick: u64,
}

pub struct DnsSdTable {
    records: [Option<DnsSdRecord>; MAXIMUM_RECORDS],
    next_generation: u32,
}

impl Default for DnsSdTable {
    fn default() -> Self {
        Self::new()
    }
}

impl DnsSdTable {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: [None; MAXIMUM_RECORDS],
            next_generation: 1,
        }
    }

    pub fn publish(
        &mut self,
        name: ServiceName,
        address: Ipv4Address,
        ttl_ticks: u64,
        now_tick: u64,
    ) -> Result<DnsSdRecord, NetworkReason> {
        self.expire(now_tick);
        if ttl_ticks == 0 {
            return Err(NetworkReason::RegistrationStale);
        }
        if let Some(existing) = self
            .records
            .iter()
            .flatten()
            .find(|record| record.name == name)
        {
            if existing.address != address {
                return Err(NetworkReason::NameConflict);
            }
            return Ok(*existing);
        }
        let slot = self
            .records
            .iter_mut()
            .find(|record| record.is_none())
            .ok_or(NetworkReason::RecordTableFull)?;
        let expires_at_tick = now_tick
            .checked_add(ttl_ticks)
            .ok_or(NetworkReason::Bounds)?;
        let generation = self.next_generation;
        let next_generation = generation.checked_add(1).ok_or(NetworkReason::Bounds)?;
        let record = DnsSdRecord {
            name,
            address,
            generation,
            expires_at_tick,
        };
        self.next_generation = next_generation;
        *slot = Some(record);
        Ok(record)
    }

    pub fn resolve(&mut self, name: ServiceName, now_tick: u64) -> Option<DnsSdRecord> {
        self.expire(now_tick);
        self.records
            .iter()
            .flatten()
            .copied()
            .find(|record| record.name == name)
    }

    pub fn reboot(&mut self) -> Result<(), NetworkReason> {
        let next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(NetworkReason::Bounds)?;
        self.records = [None; MAXIMUM_RECORDS];
        self.next_generation = next_generation;
        Ok(())
    }

    fn expire(&mut self, now_tick: u64) {
        for record in &mut self.records {
            if record.is_some_and(|record| now_tick >= record.expires_at_tick) {
                *record = None;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportProtocol {
    Tcp,
    Udp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PortBinding {
    protocol: TransportProtocol,
    port: u16,
}

pub struct PortTable {
    bindings: [Option<PortBinding>; MAXIMUM_PORTS],
}

impl Default for PortTable {
    fn default() -> Self {
        Self::new()
    }
}

impl PortTable {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bindings: [None; MAXIMUM_PORTS],
        }
    }

    pub fn bind(&mut self, protocol: TransportProtocol, port: u16) -> Result<(), NetworkReason> {
        if port == 0
            || self
                .bindings
                .iter()
                .flatten()
                .any(|binding| binding.protocol == protocol && binding.port == port)
        {
            return Err(NetworkReason::PortConflict);
        }
        let slot = self
            .bindings
            .iter_mut()
            .find(|binding| binding.is_none())
            .ok_or(NetworkReason::PortTableFull)?;
        *slot = Some(PortBinding { protocol, port });
        Ok(())
    }
}

/// Netherwick-owned composition fact. It can be used for discovery, never as
/// enrollment, possession, service, safety, or motor authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotherbrainRegistration {
    pub lease: DhcpLease,
    pub device_identity: u64,
    pub boot_identity: u64,
    pub registration_generation: u32,
    pub expires_at_tick: u64,
}

pub struct MotherbrainRegistry {
    expected_device_identity: u64,
    expected_boot_identity: u64,
    current: Option<MotherbrainRegistration>,
    next_generation: u32,
}

impl MotherbrainRegistry {
    #[must_use]
    pub const fn new(expected_device_identity: u64, expected_boot_identity: u64) -> Self {
        Self {
            expected_device_identity,
            expected_boot_identity,
            current: None,
            next_generation: 1,
        }
    }

    pub fn register(
        &mut self,
        lease: DhcpLease,
        device_identity: u64,
        boot_identity: u64,
        lease_generation: u32,
        requested_ttl_ticks: u64,
        now_tick: u64,
    ) -> Result<MotherbrainRegistration, NetworkReason> {
        if device_identity != self.expected_device_identity {
            return Err(NetworkReason::WrongDevice);
        }
        if boot_identity != self.expected_boot_identity {
            return Err(NetworkReason::WrongBoot);
        }
        if lease.generation != lease_generation {
            return Err(NetworkReason::LeaseGenerationMismatch);
        }
        if now_tick >= lease.expires_at_tick {
            return Err(NetworkReason::LeaseExpired);
        }
        if requested_ttl_ticks == 0 {
            return Err(NetworkReason::RegistrationStale);
        }
        let requested_expiry = now_tick
            .checked_add(requested_ttl_ticks)
            .ok_or(NetworkReason::Bounds)?;
        let expires_at_tick = lease.expires_at_tick.min(requested_expiry);
        let same = self.current.is_some_and(|current| {
            current.lease.client == lease.client
                && current.lease.generation == lease.generation
                && current.device_identity == device_identity
                && current.boot_identity == boot_identity
                && now_tick < current.expires_at_tick
        });
        let generation = if same {
            self.current.expect("checked").registration_generation
        } else {
            let generation = self.next_generation;
            self.next_generation = generation.checked_add(1).ok_or(NetworkReason::Bounds)?;
            generation
        };
        let registration = MotherbrainRegistration {
            lease,
            device_identity,
            boot_identity,
            registration_generation: generation,
            expires_at_tick,
        };
        self.current = Some(registration);
        Ok(registration)
    }

    #[must_use]
    pub fn resolve(&self, now_tick: u64) -> Option<MotherbrainRegistration> {
        self.current
            .filter(|registration| now_tick < registration.expires_at_tick)
    }

    pub fn reboot(&mut self, boot_identity: u64) -> Result<(), NetworkReason> {
        let next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(NetworkReason::Bounds)?;
        self.expected_boot_identity = boot_identity;
        self.current = None;
        self.next_generation = next_generation;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceKind {
    Accepted,
    Rejected,
    Pressure,
    Cancelled,
    ProviderLost,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkEvidence {
    pub sequence: u32,
    pub tick: u64,
    pub kind: EvidenceKind,
    pub reason: Option<NetworkReason>,
}

pub struct EvidenceLog {
    events: [Option<NetworkEvidence>; MAXIMUM_EVIDENCE_EVENTS],
    length: usize,
    head: usize,
    next_sequence: u32,
}

impl Default for EvidenceLog {
    fn default() -> Self {
        Self::new()
    }
}

impl EvidenceLog {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            events: [None; MAXIMUM_EVIDENCE_EVENTS],
            length: 0,
            head: 0,
            next_sequence: 1,
        }
    }

    pub fn push(
        &mut self,
        tick: u64,
        kind: EvidenceKind,
        reason: Option<NetworkReason>,
    ) -> Result<NetworkEvidence, NetworkReason> {
        let following_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(NetworkReason::EvidenceFull)?;
        let index = if self.length < MAXIMUM_EVIDENCE_EVENTS {
            let index = (self.head + self.length) % MAXIMUM_EVIDENCE_EVENTS;
            self.length += 1;
            index
        } else {
            let index = self.head;
            self.head = (self.head + 1) % MAXIMUM_EVIDENCE_EVENTS;
            index
        };
        let event = NetworkEvidence {
            sequence: self.next_sequence,
            tick,
            kind,
            reason,
        };
        self.events[index] = Some(event);
        self.next_sequence = following_sequence;
        Ok(event)
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.length
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    #[must_use]
    pub fn oldest(&self) -> Option<NetworkEvidence> {
        (self.length != 0).then(|| self.events[self.head]).flatten()
    }

    #[must_use]
    pub fn latest(&self) -> Option<NetworkEvidence> {
        (self.length != 0)
            .then(|| {
                let index = (self.head + self.length - 1) % MAXIMUM_EVIDENCE_EVENTS;
                self.events[index]
            })
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(value: u8) -> ClientIdentity {
        ClientIdentity::new(&[value]).unwrap()
    }

    #[test]
    fn dhcp_zero_eight_ninth_renew_expiry_identity_change_and_malformed_are_bounded() {
        let mut table = DhcpLeaseTable::new();
        assert_eq!(table.lease_for(client(0), 0), None);
        let mut leases = [None; MAXIMUM_CLIENTS];
        for (index, slot) in leases.iter_mut().enumerate() {
            let outcome = table
                .handle(DhcpMessage::Discover, client(index as u8), 300, 0)
                .unwrap();
            let DhcpOutcome::Offered(lease) = outcome else {
                panic!("discover did not offer")
            };
            assert_eq!(lease.address.0[3], DHCP_FIRST_ADDRESS + index as u8);
            *slot = Some(lease);
        }
        assert_eq!(
            table.handle(DhcpMessage::Discover, client(9), 300, 0),
            Err(NetworkReason::PoolExhausted)
        );
        let first = leases[0].unwrap();
        let renewed = table.handle(DhcpMessage::Renew, client(0), 300, 1).unwrap();
        assert!(
            matches!(renewed, DhcpOutcome::Acknowledged(lease) if lease.address == first.address)
        );
        assert_eq!(
            table.handle(DhcpMessage::Discover, client(10), 0, 1),
            Err(NetworkReason::MalformedPacket)
        );
        assert_eq!(table.lease_for(client(0), DHCP_LEASE_TICKS + 1), None);
        assert!(
            table
                .handle(DhcpMessage::Discover, client(10), 300, DHCP_LEASE_TICKS + 1)
                .is_ok()
        );
    }

    #[test]
    fn icmp_rate_and_packet_bounds_are_deterministic() {
        let mut limiter = IcmpRateLimiter::new();
        for _ in 0..ICMP_PACKETS_PER_WINDOW {
            limiter.admit(64, 10).unwrap();
        }
        assert_eq!(limiter.admit(64, 10), Err(NetworkReason::RateLimited));
        assert!(limiter.admit(64, ICMP_WINDOW_TICKS).is_ok());
        assert_eq!(limiter.admit(0, 2_000), Err(NetworkReason::MalformedPacket));
    }

    #[test]
    fn dns_sd_conflict_expiry_and_reboot_do_not_preserve_discovery() {
        let mut table = DnsSdTable::new();
        let name = ServiceName::new("pete.local").unwrap();
        table.publish(name, AP_ADDRESS, 10, 0).unwrap();
        assert_eq!(
            table.publish(name, Ipv4Address([192, 168, 4, 2]), 10, 0),
            Err(NetworkReason::NameConflict)
        );
        assert!(table.resolve(name, 9).is_some());
        assert_eq!(table.resolve(name, 10), None);
        table.publish(name, AP_ADDRESS, 10, 10).unwrap();
        table.reboot().expect("generation remains bounded");
        assert_eq!(table.resolve(name, 11), None);
    }

    #[test]
    fn tcp_udp_port_conflicts_are_protocol_scoped_and_finite() {
        let mut ports = PortTable::new();
        ports.bind(TransportProtocol::Tcp, 80).unwrap();
        assert_eq!(
            ports.bind(TransportProtocol::Tcp, 80),
            Err(NetworkReason::PortConflict)
        );
        ports.bind(TransportProtocol::Udp, 80).unwrap();
    }

    #[test]
    fn retained_generations_time_and_expiry_fail_closed_without_wrap_or_saturation() {
        let mut leases = DhcpLeaseTable::new();
        leases.next_generation = u32::MAX;
        assert_eq!(
            leases.handle(DhcpMessage::Discover, client(1), 64, 0),
            Err(NetworkReason::Bounds)
        );
        assert!(leases.is_empty());

        let mut leases = DhcpLeaseTable::new();
        let near_max = u64::MAX - DHCP_LEASE_TICKS;
        leases
            .handle(DhcpMessage::Discover, client(1), 64, near_max)
            .unwrap();
        let original = leases.lease_for(client(1), near_max).unwrap();
        assert_eq!(
            leases.handle(DhcpMessage::Renew, client(1), 64, near_max + 1),
            Err(NetworkReason::Bounds)
        );
        assert_eq!(leases.lease_for(client(1), near_max + 1), Some(original));
        leases.next_generation = u32::MAX;
        assert_eq!(leases.reboot(), Err(NetworkReason::Bounds));
        assert_eq!(leases.lease_for(client(1), near_max + 1), Some(original));

        let mut limiter = IcmpRateLimiter::new();
        limiter.admit(64, ICMP_WINDOW_TICKS).unwrap();
        assert_eq!(
            limiter.admit(64, ICMP_WINDOW_TICKS - 1),
            Err(NetworkReason::Bounds)
        );

        let mut records = DnsSdTable::new();
        let name = ServiceName::new("pete.local").unwrap();
        assert_eq!(
            records.publish(name, AP_ADDRESS, 2, u64::MAX - 1),
            Err(NetworkReason::Bounds)
        );
        records.next_generation = u32::MAX;
        assert_eq!(
            records.publish(name, AP_ADDRESS, 1, 0),
            Err(NetworkReason::Bounds)
        );
        assert_eq!(records.resolve(name, 0), None);

        let DhcpOutcome::Offered(lease) = DhcpLeaseTable::new()
            .handle(DhcpMessage::Discover, client(2), 64, 0)
            .unwrap()
        else {
            panic!("discover did not offer");
        };
        let mut registry = MotherbrainRegistry::new(10, 20);
        assert_eq!(
            registry.register(lease, 10, 20, lease.generation, u64::MAX, 1,),
            Err(NetworkReason::Bounds)
        );
        assert_eq!(registry.resolve(1), None);
    }

    #[test]
    fn motherbrain_registration_binds_device_boot_lease_generation_and_ttl() {
        let mut leases = DhcpLeaseTable::new();
        let DhcpOutcome::Acknowledged(lease) = leases
            .handle(DhcpMessage::Renew, client(1), 300, 0)
            .unwrap()
        else {
            panic!("renew did not acknowledge")
        };
        let mut registry = MotherbrainRegistry::new(10, 20);
        assert_eq!(
            registry.register(lease, 11, 20, lease.generation, 100, 0),
            Err(NetworkReason::WrongDevice)
        );
        assert_eq!(
            registry.register(lease, 10, 21, lease.generation, 100, 0),
            Err(NetworkReason::WrongBoot)
        );
        assert_eq!(
            registry.register(lease, 10, 20, lease.generation + 1, 100, 0),
            Err(NetworkReason::LeaseGenerationMismatch)
        );
        let registration = registry
            .register(lease, 10, 20, lease.generation, 100, 0)
            .unwrap();
        assert_eq!(registry.resolve(99), Some(registration));
        assert_eq!(registry.resolve(100), None);
        registry.reboot(21).expect("generation remains bounded");
        assert_eq!(registry.resolve(1), None);
    }

    #[test]
    fn cancellation_provider_loss_pressure_and_terminal_evidence_are_distinct() {
        let mut evidence = EvidenceLog::new();
        assert_eq!(
            evidence
                .push(1, EvidenceKind::Accepted, None)
                .unwrap()
                .sequence,
            1
        );
        assert_eq!(
            evidence
                .push(
                    2,
                    EvidenceKind::Pressure,
                    Some(NetworkReason::PoolExhausted)
                )
                .unwrap()
                .sequence,
            2
        );
        evidence
            .push(3, EvidenceKind::Cancelled, Some(NetworkReason::Cancelled))
            .unwrap();
        evidence
            .push(
                4,
                EvidenceKind::ProviderLost,
                Some(NetworkReason::ProviderLost),
            )
            .unwrap();
        evidence.push(5, EvidenceKind::Terminal, None).unwrap();
    }

    #[test]
    fn standing_evidence_retains_one_exact_bounded_window() {
        let mut evidence = EvidenceLog::new();
        for tick in 0..(MAXIMUM_EVIDENCE_EVENTS as u64 + 3) {
            evidence.push(tick, EvidenceKind::Accepted, None).unwrap();
        }
        assert_eq!(evidence.len(), MAXIMUM_EVIDENCE_EVENTS);
        assert_eq!(evidence.oldest().unwrap().sequence, 4);
        assert_eq!(
            evidence.latest().unwrap().sequence,
            MAXIMUM_EVIDENCE_EVENTS as u32 + 3
        );
    }

    #[test]
    fn conformance_inventory_owns_every_required_boundary_and_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/c4/netherwick-network.json"
        ))
        .unwrap();
        let ownership = fixture["ownership"].as_array().unwrap();
        for required in [
            "ap",
            "dhcp",
            "icmp",
            "tcp",
            "udp",
            "http",
            "sse",
            "websocket",
            "mdns",
            "motherbrain-registration",
            "create-uart",
        ] {
            assert!(ownership.iter().any(|entry| entry["behavior"] == required));
        }
        let cases = fixture["cases"].as_array().unwrap();
        for required in [
            "zero-client",
            "eight-clients",
            "ninth-client",
            "renew",
            "expiry",
            "client-identity-change",
            "wrong-boot",
            "wrong-device",
            "stale-registration",
            "ap-unavailable",
            "cyw43-init-failure",
            "malformed-packet",
            "icmp-rate-bound",
            "tcp-port-conflict",
            "udp-port-conflict",
            "http-sse-websocket-cancellation",
            "mdns-name-conflict",
            "pressure",
            "cancel",
            "reboot",
            "provider-loss",
            "no-route-no-bridge-no-nat",
            "describe-only-no-effects",
            "forged-source-authority",
            "no-radio-fixture-no-authority",
            "physical-provider-missing-binding",
            "physical-provider-revoked-at-use",
            "physical-provider-direct-handler-bypass",
            "network-never-grants-robot-authority",
        ] {
            assert!(
                cases.iter().any(|case| case["id"] == required),
                "{required}"
            );
        }
    }
}
