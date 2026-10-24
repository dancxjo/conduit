//! The Fore consumes finite Source-encoded packets, not temperatures or glyphs.
use alloc::vec;
use conduit_core as c;
pub const WRITE_KIND: &str = "machine/tm1637/write";
pub const WRITE_CALL: &str = "conduit.host/tm1637-write@1";
pub const MAXIMUM_BYTES: u32 = 1024;
pub struct WriteContract {
    kind: c::Kind,
    packets: c::StructuredInfoType,
}
impl WriteContract {
    pub fn prepare() -> Result<Self, c::StructuredInfoRefusal> {
        let packets = c::StructuredInfoType::from_canonical_bytes(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/packets.type"
        )))?;
        let profile = packets.profile()?;
        let kind = c::Kind {
            kind_id: c::kind_id(WRITE_KIND),
            kind_contract_revision: "machine/tm1637/write@1".into(),
            startup_parameters: vec![],
            shorthand: None,
            semantic_laws: vec![],
            configuration: vec![],
            inputs: vec![c::PortDescriptor {
                port_id: c::port_id("packets"),
                value_kind: profile.value_kind().clone(),
                direction: c::PortDirection::Input,
                temporal: c::PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            }],
            outputs: vec![],
            limits: c::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: MAXIMUM_BYTES,
            },
        };
        Ok(Self { kind, packets })
    }
    pub fn kind(&self) -> &c::Kind {
        &self.kind
    }
    pub fn packets_type(&self) -> &c::StructuredInfoType {
        &self.packets
    }
}
