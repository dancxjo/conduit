//! Preparation-only contract derived from reviewed Conduit Types.

use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{
    CapabilityLimits, CheckedValueContract, FrontValueContract, FrontValueLocation, Kind,
    KindIdentity, KindSemanticLaw, PortDescriptor, PortDirection, PortTemporal,
    PreparedStructuredValueValidator, StructuredInfoRefusal, StructuredInfoType, kind_id, port_id,
};
use conduit_plot::{
    CheckedNativeType, ProfileCatalog, StartupCatalog, check_syntax_document, parse_syntax_document,
};

pub const ENDPOINT_READ_KIND: &str = "machine/usb/endpoint-read";
pub const ENDPOINT_READ_CALL: &str = "conduit.host/usb-endpoint-read@1";
pub const ENDPOINT_READ_MAXIMUM_BYTES: u32 = 4096;
pub const ENDPOINT_READ_DATA_BYTES: u16 = 2048;
pub const ENDPOINT_READ_TYPES: &str = include_str!("../../plots/usb/endpoint-read-types.conduit");

/// Exact schemas, prepared before Play. A definition is not an available Back,
/// selected endpoint, resource reservation or authority to perform a transfer.
pub struct EndpointReadContract {
    request: StructuredInfoType,
    result: StructuredInfoType,
    kind: Kind,
    checked_types: Vec<CheckedNativeType>,
}

impl EndpointReadContract {
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
        let syntax = parse_syntax_document(ENDPOINT_READ_TYPES);
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
        let request = find("UsbEndpointReadRequest")?;
        let result = find("UsbEndpointReadResult")?;
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
                    ENDPOINT_READ_MAXIMUM_BYTES,
                    Vec::new(),
                )
                .map_err(|_| StructuredInfoRefusal::WrongType)?,
            },
            FrontValueContract {
                location: FrontValueLocation::Output(output.port_id.clone()),
                contract: CheckedValueContract::new(
                    output.value_kind.clone(),
                    ENDPOINT_READ_MAXIMUM_BYTES,
                    Vec::new(),
                )
                .map_err(|_| StructuredInfoRefusal::WrongType)?,
            },
        ];
        let kind = Kind {
            kind_id: kind_id(ENDPOINT_READ_KIND),
            kind_contract_revision: KindIdentity::from("machine/usb/endpoint-read@1"),
            startup_parameters: vec![],
            shorthand: Some((input.port_id.clone(), output.port_id.clone())),
            inputs: vec![input],
            outputs: vec![output],
            configuration: vec![],
            semantic_laws: vec![KindSemanticLaw::ValueContracts(contracts)],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: ENDPOINT_READ_MAXIMUM_BYTES,
            },
        };
        Ok(Self {
            request,
            result,
            kind,
            checked_types: checked.native_types,
        })
    }

    pub fn request_validator(
        &self,
    ) -> Result<PreparedStructuredValueValidator, StructuredInfoRefusal> {
        PreparedStructuredValueValidator::new(&self.request, ENDPOINT_READ_MAXIMUM_BYTES as usize)
    }

    /// Register inert checked contracts, preserving nested byte refinements.
    /// This creates no implementation offer, resource possession, or authority.
    pub fn install_catalogs(
        &self,
        startup: &mut StartupCatalog,
        profile: &mut ProfileCatalog,
    ) -> Result<(), String> {
        for (path, name) in [
            (
                "machine/usb/endpoint-read/request",
                "UsbEndpointReadRequest",
            ),
            ("machine/usb/endpoint-read/result", "UsbEndpointReadResult"),
        ] {
            let ty = self
                .checked_types
                .iter()
                .find(|ty| ty.name == name)
                .expect("prepared endpoint Type");
            startup.insert_checked_native_type(path, ty)?;
        }
        startup.insert(conduit_plot::KindSignature {
            kind: ENDPOINT_READ_KIND.into(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(ENDPOINT_READ_KIND, self.kind.checked_front())?;
        profile
            .insert_kind(self.kind.clone())
            .map_err(|error| format!("{error:?}"))
    }

    pub fn catalogs(&self) -> (StartupCatalog, ProfileCatalog) {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        self.install_catalogs(&mut startup, &mut profile)
            .expect("prepared endpoint catalogs");
        (startup, profile)
    }
}
