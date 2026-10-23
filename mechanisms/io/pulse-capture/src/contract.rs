//! A Fore definition is neither an installed Back nor possession of a GPIO.
use alloc::vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoRefusal, StructuredInfoType,
};
pub const PULSE_READ_KIND: &str = "machine/pulse/read";
pub const PULSE_READ_CALL: &str = "conduit.host/pulse-read@1";
pub const PULSE_READ_MAXIMUM_BYTES: u32 = 1024;
pub struct PulseReadContract {
    pub(crate) request: StructuredInfoType,
    pub(crate) capture: StructuredInfoType,
    pub(crate) result: StructuredInfoType,
    kind: Kind,
}
impl PulseReadContract {
    pub fn prepare() -> Result<Self, StructuredInfoRefusal> {
        let request = StructuredInfoType::from_canonical_bytes(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/request.type"
        )))?;
        let capture = StructuredInfoType::from_canonical_bytes(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/capture.type"
        )))?;
        let result = StructuredInfoType::from_canonical_bytes(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/result.type"
        )))?;
        let port = |name, ty: &StructuredInfoType, direction| -> Result<_, StructuredInfoRefusal> {
            Ok(PortDescriptor {
                port_id: port_id(name),
                value_kind: ty.profile()?.value_kind().clone(),
                direction,
                temporal: PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            })
        };
        let input = port("request", &request, PortDirection::Input)?;
        let output = port("result", &result, PortDirection::Output)?;
        let kind = Kind {
            kind_id: kind_id(PULSE_READ_KIND),
            kind_contract_revision: "machine/pulse/read@1".into(),
            startup_parameters: vec![],
            shorthand: Some((input.port_id.clone(), output.port_id.clone())),
            inputs: vec![input],
            outputs: vec![output],
            configuration: vec![],
            semantic_laws: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: PULSE_READ_MAXIMUM_BYTES,
            },
        };
        Ok(Self {
            request,
            capture,
            result,
            kind,
        })
    }
    pub fn kind(&self) -> &Kind {
        &self.kind
    }
    pub fn request_type(&self) -> &StructuredInfoType {
        &self.request
    }
    pub fn result_type(&self) -> &StructuredInfoType {
        &self.result
    }
    #[cfg(feature = "plot-catalog")]
    pub fn install(
        &self,
        startup: &mut conduit_plot::StartupCatalog,
        profiles: &mut conduit_plot::ProfileCatalog,
    ) -> Result<(), alloc::string::String> {
        // A selector preserves the canonical Info envelope of an Empty payload.
        // Its Fore is distinct from a bare primitive Empty token.
        let absence = StructuredInfoType::leaf(kind_id(conduit_core::EMPTY_INFO_ID))
            .and_then(|ty| ty.profile())
            .map_err(|e| alloc::format!("pulse absence profile: {e:?}"))?;
        startup.insert_value_kind_alias("PulseReadAbsence", absence.value_kind().clone())?;
        startup
            .insert(conduit_plot::KindSignature {
                kind: PULSE_READ_KIND.into(),
                startup_parameters: vec![],
            })
            .map_err(|e| alloc::format!("pulse Kind: {e:?}"))?;
        startup
            .insert_fore(PULSE_READ_KIND, self.kind.checked_front())
            .map_err(|e| alloc::format!("pulse Fore: {e:?}"))?;
        profiles
            .insert_kind(self.kind.clone())
            .map_err(|e| alloc::format!("pulse profile: {e:?}"))?;
        Ok(())
    }
}
