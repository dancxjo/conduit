//! Preparation-only contract derived from reviewed Conduit Types.

use alloc::{vec, vec::Vec};
use conduit_core::{
    CapabilityLimits, Kind, KindIdentity, PortDescriptor, PortDirection, PortTemporal,
    PreparedStructuredValueValidator, StructuredInfoRefusal, StructuredInfoType, kind_id, port_id,
};
use conduit_plot::{ProfileCatalog, StartupCatalog, check_syntax_document, parse_syntax_document};

pub const DIGITAL_KIND: &str = "machine/digital/access";
pub const DIGITAL_CALL: &str = "conduit.host/digital-access@1";
pub const DIGITAL_MAXIMUM_BYTES: u32 = 512;
pub const DIGITAL_TYPES: &str =
    include_str!("../../../../plots/device-protocols/digital-types.conduit");

/// Exact schemas, prepared before Play. A definition is not an available Back,
/// selected endpoint, resource reservation or authority to perform a transfer.
pub struct DigitalContract {
    request: StructuredInfoType,
    result: StructuredInfoType,
    kind: Kind,
    types: Vec<conduit_plot::CheckedNativeType>,
}

impl DigitalContract {
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
        let syntax = parse_syntax_document(DIGITAL_TYPES);
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
        let request = find("DigitalRequest")?;
        let result = find("DigitalResult")?;
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
            kind_id: kind_id(DIGITAL_KIND),
            kind_contract_revision: KindIdentity::from("machine/digital/access@1"),
            startup_parameters: vec![],
            shorthand: Some((input.port_id.clone(), output.port_id.clone())),
            inputs: vec![input],
            outputs: vec![output],
            configuration: vec![],
            semantic_laws: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: DIGITAL_MAXIMUM_BYTES,
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
        PreparedStructuredValueValidator::new(&self.request, DIGITAL_MAXIMUM_BYTES as usize)
    }

    /// Install inert checked contracts; this neither advertises a Back nor grants possession.
    pub fn install_catalogs(
        &self,
        startup: &mut StartupCatalog,
        profile: &mut ProfileCatalog,
    ) -> Result<(), alloc::string::String> {
        for (name, path) in [
            ("DigitalRequest", "machine/digital/access/request"),
            ("DigitalResult", "machine/digital/access/result"),
        ] {
            let ty = self
                .types
                .iter()
                .find(|ty| ty.name == name)
                .expect("prepared peripheral Type");
            startup.insert_checked_native_type(path, ty)?;
        }
        startup.insert(conduit_plot::KindSignature {
            kind: DIGITAL_KIND.into(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(DIGITAL_KIND, self.kind.checked_front())?;
        profile
            .insert_kind(self.kind.clone())
            .map_err(|error| alloc::format!("{error:?}"))
    }
    pub fn catalogs(&self) -> (StartupCatalog, ProfileCatalog) {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        self.install_catalogs(&mut startup, &mut profile)
            .expect("prepared peripheral catalogs");
        (startup, profile)
    }
}
