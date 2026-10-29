//! Finite scan representation, never radio truth, possession or round renewal.
use crate::ObservedNetworkSecurity;
#[path = "network_scan_view.rs"]
mod view;
pub use view::ScanCompletionView;
pub const NETWORK_SCAN_OPERATION: &str = "network/scan";
pub const NETWORK_SCAN_HOST_CALL: &str = "conduit.host/network-scan@1";
pub const NETWORK_SCAN_AUTHORITY: &str = "conduit.authority/network-scan@1";
pub const NETWORK_SCAN_SUBJECT: &str = "authority/network-scanner";
pub const MAXIMUM_SCAN_OBSERVATIONS: usize = 16;
/// Finite Host Call accounting/capacity bound, matching credential sources.
/// The authored Kind has no input ports and actual invocation accepts zero bytes.
pub const MAXIMUM_SCAN_INPUT_BYTES: u32 = 1;
/// Optional private provider-context representation, never an authored input port.
pub const SCAN_PROVIDER_CONTEXT_BYTES: usize = 28;
pub const MAXIMUM_SCAN_OUTPUT_BYTES: u32 = 728;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanFailure {
    Malformed,
    Pressure,
    Deadline,
    Rejected,
    Unavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Independently admitted Root invocation context supplied by the retained
/// initialized owner. These fields are not authored or remote request input.
pub struct NetworkScanRequest {
    pub generation: u64,
    pub scan_deadline_ms: u64,
    pub cleanup_deadline_ms: u64,
}
impl NetworkScanRequest {
    pub fn validate(self) -> Result<Self, ScanFailure> {
        if self.generation == 0
            || self.scan_deadline_ms == 0
            || self.cleanup_deadline_ms <= self.scan_deadline_ms
        {
            Err(ScanFailure::Malformed)
        } else {
            Ok(self)
        }
    }
    pub fn encode_context(self) -> Result<[u8; SCAN_PROVIDER_CONTEXT_BYTES], ScanFailure> {
        self.validate()?;
        let mut b = [0; 28];
        b[..4].copy_from_slice(b"CNS1");
        for (i, n) in [
            self.generation,
            self.scan_deadline_ms,
            self.cleanup_deadline_ms,
        ]
        .into_iter()
        .enumerate()
        {
            b[4 + i * 8..12 + i * 8].copy_from_slice(&n.to_le_bytes());
        }
        Ok(b)
    }
    pub fn decode_context(b: &[u8]) -> Result<Self, ScanFailure> {
        if b.len() != 28 || &b[..4] != b"CNS1" {
            return Err(ScanFailure::Malformed);
        }
        Self {
            generation: u64::from_le_bytes(b[4..12].try_into().unwrap()),
            scan_deadline_ms: u64::from_le_bytes(b[12..20].try_into().unwrap()),
            cleanup_deadline_ms: u64::from_le_bytes(b[20..28].try_into().unwrap()),
        }
        .validate()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanObservation {
    ssid: [u8; 32],
    length: u8,
    pub security: ObservedNetworkSecurity,
    pub rssi_dbm: i16,
    pub observed_ms: u64,
}
impl ScanObservation {
    pub const EMPTY: Self = Self {
        ssid: [0; 32],
        length: 0,
        security: ObservedNetworkSecurity::Unknown,
        rssi_dbm: 0,
        observed_ms: 0,
    };
    pub fn new(
        ssid: &[u8],
        security: ObservedNetworkSecurity,
        rssi_dbm: i16,
        observed_ms: u64,
    ) -> Result<Self, ScanFailure> {
        if ssid.len() > 32 {
            return Err(ScanFailure::Malformed);
        }
        let mut v = Self::EMPTY;
        v.ssid[..ssid.len()].copy_from_slice(ssid);
        v.length = ssid.len() as u8;
        v.security = security;
        v.rssi_dbm = rssi_dbm;
        v.observed_ms = observed_ms;
        Ok(v)
    }
    pub fn ssid(&self) -> &[u8] {
        &self.ssid[..self.length as usize]
    }
    pub fn as_known(&self) -> crate::KnownScanObservation<'_> {
        crate::KnownScanObservation {
            ssid: self.ssid(),
            security: self.security,
            rssi_dbm: self.rssi_dbm,
            observed_ms: self.observed_ms,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanCompletion {
    pub generation: u64,
    pub completed_ms: u64,
    pub count: u8,
    pub total: u16,
}
impl ScanCompletion {
    pub fn truncated(self) -> bool {
        self.total > u16::from(self.count)
    }
    pub fn validate(self, records: &[ScanObservation]) -> Result<(), ScanFailure> {
        if self.generation == 0
            || self.count as usize > 16
            || records.len() != self.count as usize
            || self.total < u16::from(self.count)
            || records.iter().any(|r| r.observed_ms > self.completed_ms)
        {
            Err(ScanFailure::Malformed)
        } else {
            Ok(())
        }
    }
    pub fn encode(self, records: &[ScanObservation], out: &mut [u8]) -> Result<usize, ScanFailure> {
        self.validate(records)?;
        let len = 24 + 44 * records.len();
        if out.len() < len {
            return Err(ScanFailure::Pressure);
        }
        out[..4].copy_from_slice(b"CNR1");
        out[4..12].copy_from_slice(&self.generation.to_le_bytes());
        out[12..20].copy_from_slice(&self.completed_ms.to_le_bytes());
        out[20] = self.count;
        out[21..23].copy_from_slice(&self.total.to_le_bytes());
        out[23] = u8::from(self.truncated());
        for (r, b) in records.iter().zip(out[24..len].chunks_exact_mut(44)) {
            b[0] = r.length;
            b[1..33].copy_from_slice(&r.ssid);
            b[33..35].copy_from_slice(&r.rssi_dbm.to_le_bytes());
            b[35] = match r.security {
                ObservedNetworkSecurity::Unknown => 0,
                ObservedNetworkSecurity::Unsupported => 1,
                ObservedNetworkSecurity::Known(crate::KnownSecurity::Wpa2Personal) => 2,
                ObservedNetworkSecurity::Known(crate::KnownSecurity::Wpa3Personal) => 3,
            };
            b[36..44].copy_from_slice(&r.observed_ms.to_le_bytes());
        }
        Ok(len)
    }
    pub fn decode(bytes: &[u8], out: &mut [ScanObservation; 16]) -> Result<Self, ScanFailure> {
        // Validate the immutable complete extent first. Publishing then cannot
        // fail: no second 16-record array or partial caller mutation is needed.
        let checked = ScanCompletionView::parse(bytes)?;
        for (slot, record) in out.iter_mut().zip(checked.observations()) {
            *slot = ScanObservation::new(
                record.ssid,
                record.security,
                record.rssi_dbm,
                record.observed_ms,
            )
            .expect("same fully validated immutable scan bytes");
        }
        Ok(checked.completion())
    }
}

pub const NETWORK_SCAN_EXECUTION_PROFILE: &str = "conduit.network/scan-base@1";
pub fn network_scan_contract_ref() -> conduit_core::KindDefinitionRef<'static> {
    use conduit_core::*;
    const OUTPUT: &[PortDescriptorRef<'static>] = &[PortDescriptorRef {
        port_id: "observations",
        value_kind: "network/scan-observations",
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }];
    const LIMITS: CapabilityLimits = CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: MAXIMUM_SCAN_OUTPUT_BYTES,
    };
    KindDefinitionRef {
        startup_parameters: &[],
        shorthand: None,
        kind_id: NETWORK_SCAN_OPERATION,
        kind_contract_revision: "conduit.network/scan@1",
        inputs: PortDescriptorRefs::Borrowed(&[]),
        outputs: PortDescriptorRefs::Borrowed(OUTPUT),
        configuration: &[],
        semantic_laws: &[],
        limits: &LIMITS,
    }
}
pub fn network_scan_contract() -> conduit_core::Kind {
    network_scan_contract_ref().materialize()
}
pub fn network_scan_offer(
    capability: conduit_core::CapabilityId,
    implementation: conduit_core::ImplementationId,
    artifact: conduit_core::ArtifactId,
) -> conduit_core::CapabilityOffer {
    crate::borrowed_contracts::materialize(
        crate::borrowed_contracts::Shape::Scan,
        capability,
        implementation,
        artifact,
    )
}
