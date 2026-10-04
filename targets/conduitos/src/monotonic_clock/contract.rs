//! Preparation-only contract derived from reviewed Conduit Types.

use alloc::{vec, vec::Vec};
use conduit_core::{
    CapabilityLimits, Kind, KindIdentity, PortDescriptor, PortDirection, PortTemporal,
    PreparedStructuredValueValidator, StructuredInfoRefusal, StructuredInfoType, kind_id, port_id,
};
use conduit_plot::{ProfileCatalog, StartupCatalog, check_syntax_document, parse_syntax_document};

pub const CLOCK_KIND: &str = "machine/clock/at";
pub const CLOCK_CALL: &str = "conduit.host/monotonic-clock-at@1";
pub const CLOCK_MAXIMUM_BYTES: u32 = 512;
pub const CLOCK_TYPES: &str =
    include_str!("../../../../plots/device-protocols/clock-types.conduit");

/// Exact schemas, prepared before Play. A definition is not an available Back,
/// selected endpoint, resource reservation or authority to observe or wait on a clock.
pub struct MonotonicClockContract {
    request: StructuredInfoType,
    result: StructuredInfoType,
    kind: Kind,
    types: Vec<conduit_plot::CheckedNativeType>,
}

impl MonotonicClockContract {
    pub fn request_type(&self) -> &StructuredInfoType {
        &self.request
    }
    pub fn result_type(&self) -> &StructuredInfoType {
        &self.result
    }
    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    pub fn prepare() -> Result<Self, StructuredInfoRefusal> {
        let syntax = parse_syntax_document(CLOCK_TYPES);
        if !syntax.diagnostics.is_empty() {
            return Err(StructuredInfoRefusal::WrongType);
        }
        let checked = check_syntax_document(&syntax, &StartupCatalog::new())
            .map_err(|_| StructuredInfoRefusal::WrongType)?;
        let find = |name| {
            checked
                .native_types
                .iter()
                .find(|ty| ty.name == name)
                .map(|ty| ty.value_type.clone())
                .ok_or(StructuredInfoRefusal::WrongType)
        };
        let request = find("MonotonicClockRequest")?;
        let result = find("MonotonicClockResult")?;
        let port = |name: &str, ty: &StructuredInfoType, direction| {
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
            kind_id: kind_id(CLOCK_KIND),
            kind_contract_revision: KindIdentity::from("machine/clock/at@1"),
            startup_parameters: vec![],
            shorthand: Some((input.port_id.clone(), output.port_id.clone())),
            inputs: vec![input],
            outputs: vec![output],
            configuration: vec![],
            semantic_laws: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: CLOCK_MAXIMUM_BYTES,
            },
        };
        Ok(Self {
            request,
            result,
            kind,
            types: checked.native_types,
        })
    }

    pub fn request_validator(
        &self,
    ) -> Result<PreparedStructuredValueValidator, StructuredInfoRefusal> {
        PreparedStructuredValueValidator::new(&self.request, CLOCK_MAXIMUM_BYTES as usize)
    }

    pub fn catalogs(&self) -> (StartupCatalog, ProfileCatalog) {
        let mut startup = StartupCatalog::new();
        for (name, path) in [
            ("MonotonicClockRequest", "machine/clock/at/request"),
            ("MonotonicClockResult", "machine/clock/at/result"),
        ] {
            let ty = self
                .types
                .iter()
                .find(|ty| ty.name == name)
                .expect("checked clock Type");
            startup
                .insert_checked_native_type(path, ty)
                .expect("clock import");
        }
        startup
            .insert(conduit_plot::KindSignature {
                kind: CLOCK_KIND.into(),
                startup_parameters: vec![],
            })
            .expect("clock startup");
        startup
            .insert_fore(CLOCK_KIND, self.kind.checked_front())
            .expect("clock Fore");
        let mut profile = ProfileCatalog::new();
        profile.insert_kind(self.kind.clone()).expect("clock Kind");
        (startup, profile)
    }
}
