//! Preparation-only contract derived from reviewed Conduit Types.

use alloc::{vec, vec::Vec};
use conduit_core::{
    CapabilityLimits, CheckedValueContract, FrontValueContract, FrontValueLocation, Kind,
    KindIdentity, KindSemanticLaw, PortDescriptor, PortDirection, PortTemporal,
    PreparedStructuredValueValidator, StructuredInfoRefusal, StructuredInfoType, kind_id, port_id,
};
use conduit_plot::{ProfileCatalog, StartupCatalog, check_syntax_document, parse_syntax_document};

pub const CONTROL_KIND: &str = "machine/usb/control";
pub const CONTROL_CALL: &str = "conduit.host/usb-control@1";
pub const CONTROL_MAXIMUM_BYTES: u32 = 4096;
pub const CONTROL_TYPES: &str = include_str!("../../plots/usb/control-types.conduit");

/// Exact schemas, prepared before Play. A definition is not an available Back,
/// selected endpoint, resource reservation or authority to perform a transfer.
pub struct ControlContract {
    request: StructuredInfoType,
    result: StructuredInfoType,
    kind: Kind,
}

impl ControlContract {
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
        let syntax = parse_syntax_document(CONTROL_TYPES);
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
        let request = find("UsbControlRequest")?;
        let result = find("UsbControlResult")?;
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
        let contracts = vec![
            FrontValueContract {
                location: FrontValueLocation::Input(input.port_id.clone()),
                contract: CheckedValueContract::new(
                    input.value_kind.clone(),
                    CONTROL_MAXIMUM_BYTES,
                    Vec::new(),
                )
                .map_err(|_| StructuredInfoRefusal::WrongType)?,
            },
            FrontValueContract {
                location: FrontValueLocation::Output(output.port_id.clone()),
                contract: CheckedValueContract::new(
                    output.value_kind.clone(),
                    CONTROL_MAXIMUM_BYTES,
                    Vec::new(),
                )
                .map_err(|_| StructuredInfoRefusal::WrongType)?,
            },
        ];
        let kind = Kind {
            kind_id: kind_id(CONTROL_KIND),
            kind_contract_revision: KindIdentity::from("machine/usb/control@1"),
            startup_parameters: vec![],
            shorthand: Some((input.port_id.clone(), output.port_id.clone())),
            inputs: vec![input],
            outputs: vec![output],
            configuration: vec![],
            semantic_laws: vec![KindSemanticLaw::ValueContracts(contracts)],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: CONTROL_MAXIMUM_BYTES,
            },
        };
        Ok(Self {
            request,
            result,
            kind,
        })
    }

    pub fn request_validator(
        &self,
    ) -> Result<PreparedStructuredValueValidator, StructuredInfoRefusal> {
        PreparedStructuredValueValidator::new(&self.request, CONTROL_MAXIMUM_BYTES as usize)
    }

    pub fn catalogs(&self) -> (StartupCatalog, ProfileCatalog) {
        let mut startup = StartupCatalog::new();
        startup
            .insert_structured_type("machine/usb/control/request", self.request.clone())
            .expect("request Type");
        startup
            .insert_structured_type("machine/usb/control/result", self.result.clone())
            .expect("result Type");
        startup
            .insert(conduit_plot::KindSignature {
                kind: CONTROL_KIND.into(),
                startup_parameters: vec![],
            })
            .expect("control startup");
        startup
            .insert_fore(CONTROL_KIND, self.kind.checked_front())
            .expect("control Fore");
        let mut profile = ProfileCatalog::new();
        profile
            .insert_kind(self.kind.clone())
            .expect("control Kind");
        (startup, profile)
    }
}
