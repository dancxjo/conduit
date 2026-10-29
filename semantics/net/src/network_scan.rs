//! Finite scan representation, never radio truth, possession or round renewal.
use crate::ObservedNetworkSecurity;
pub const NETWORK_SCAN_OPERATION: &str = "network/scan";
pub const NETWORK_SCAN_HOST_CALL: &str = "conduit.host/network-scan@1";
pub const NETWORK_SCAN_AUTHORITY: &str = "conduit.authority/network-scan@1";
pub const NETWORK_SCAN_SUBJECT: &str = "authority/network-scanner";
pub const MAXIMUM_SCAN_OBSERVATIONS: usize = 16;
pub const MAXIMUM_SCAN_INPUT_BYTES: u32 = 28;
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
    pub fn encode(self) -> Result<[u8; 28], ScanFailure> {
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
    pub fn decode(b: &[u8]) -> Result<Self, ScanFailure> {
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
        if bytes.len() < 24 || &bytes[..4] != b"CNR1" {
            return Err(ScanFailure::Malformed);
        }
        let v = Self {
            generation: u64::from_le_bytes(bytes[4..12].try_into().unwrap()),
            completed_ms: u64::from_le_bytes(bytes[12..20].try_into().unwrap()),
            count: bytes[20],
            total: u16::from_le_bytes(bytes[21..23].try_into().unwrap()),
        };
        if v.count > 16
            || bytes.len() != 24 + 44 * v.count as usize
            || bytes[23] != u8::from(v.truncated())
        {
            return Err(ScanFailure::Malformed);
        }
        // Validate everything before publishing any caller-owned extent.
        let mut staged = [ScanObservation::EMPTY; 16];
        for (slot, b) in staged.iter_mut().zip(bytes[24..].chunks_exact(44)) {
            if b[0] > 32 || b[1 + b[0] as usize..33].iter().any(|b| *b != 0) {
                return Err(ScanFailure::Malformed);
            }
            let security = match b[35] {
                0 => ObservedNetworkSecurity::Unknown,
                1 => ObservedNetworkSecurity::Unsupported,
                2 => ObservedNetworkSecurity::Known(crate::KnownSecurity::Wpa2Personal),
                3 => ObservedNetworkSecurity::Known(crate::KnownSecurity::Wpa3Personal),
                _ => return Err(ScanFailure::Malformed),
            };
            *slot = ScanObservation::new(
                &b[1..1 + b[0] as usize],
                security,
                i16::from_le_bytes(b[33..35].try_into().unwrap()),
                u64::from_le_bytes(b[36..44].try_into().unwrap()),
            )?;
        }
        v.validate(&staged[..v.count as usize])?;
        out[..v.count as usize].copy_from_slice(&staged[..v.count as usize]);
        Ok(v)
    }
}

pub const NETWORK_SCAN_EXECUTION_PROFILE: &str = "conduit.network/scan-base@1";
pub fn network_scan_contract_ref() -> conduit_core::KindDefinitionRef<'static> {
    use conduit_core::*;
    const INPUT: &[PortDescriptorRef<'static>] = &[PortDescriptorRef {
        port_id: "request",
        value_kind: "network/scan-request",
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }];
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
        shorthand: Some(("request", "observations")),
        kind_id: NETWORK_SCAN_OPERATION,
        kind_contract_revision: "conduit.network/scan@1",
        inputs: PortDescriptorRefs::Borrowed(INPUT),
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
    use alloc::vec;
    use conduit_core::*;
    BackOfferBuilder::new(
        network_scan_contract(),
        Back {
            capability_id: capability,
            implementation_id: implementation,
            artifact_id: artifact,
            execution_profile_id: NETWORK_SCAN_EXECUTION_PROFILE.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: NETWORK_SCAN_HOST_CALL.into(),
                target_kind: None,
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_SCAN_INPUT_BYTES,
                maximum_output_bytes: MAXIMUM_SCAN_OUTPUT_BYTES,
            }],
            resource_requirements: vec![ResourceRequirement {
                class_id: crate::WIFI_STATION_RESOURCE_CLASS.into(),
                units: 1,
                protected_role: None,
                compute: None,
                content: None,
            }],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: NETWORK_SCAN_AUTHORITY.into(),
                host_call_contract_id: NETWORK_SCAN_HOST_CALL.into(),
                subject_kind: NETWORK_SCAN_SUBJECT.into(),
            }],
        },
    )
    .build()
}
