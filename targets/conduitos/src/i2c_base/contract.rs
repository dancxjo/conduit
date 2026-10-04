//! Preparation-only contract derived from reviewed Conduit Types.

use alloc::{vec, vec::Vec};
use conduit_core::{
    CapabilityLimits, Kind, KindIdentity, PortDescriptor, PortDirection, PortTemporal,
    PreparedStructuredValueValidator, StructuredInfoRefusal, StructuredInfoType, kind_id, port_id,
};
use conduit_plot::{ProfileCatalog, StartupCatalog, check_syntax_document, parse_syntax_document};

pub const I2C_KIND: &str = "machine/i2c/transact";
pub const I2C_CALL: &str = "conduit.host/i2c-transact@1";
pub const I2C_MAXIMUM_BYTES: u32 = 4096;
pub const I2C_TYPES: &str = include_str!("../../../../plots/device-protocols/i2c-types.conduit");

/// Exact schemas, prepared before Play. A definition is not an available Back,
/// selected endpoint, resource reservation or authority to perform a transfer.
pub struct I2cContract {
    request: StructuredInfoType,
    result: StructuredInfoType,
    kind: Kind,
    types: Vec<conduit_plot::CheckedNativeType>,
}

impl I2cContract {
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
        let syntax = parse_syntax_document(I2C_TYPES);
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
        let request = find("I2cTransaction")?;
        let result = find("I2cResult")?;
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
            kind_id: kind_id(I2C_KIND),
            kind_contract_revision: KindIdentity::from("machine/i2c/transact@1"),
            startup_parameters: vec![],
            shorthand: Some((input.port_id.clone(), output.port_id.clone())),
            inputs: vec![input],
            outputs: vec![output],
            configuration: vec![],
            semantic_laws: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: I2C_MAXIMUM_BYTES,
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
        PreparedStructuredValueValidator::new(&self.request, I2C_MAXIMUM_BYTES as usize)
    }

    pub fn catalogs(&self) -> (StartupCatalog, ProfileCatalog) {
        let mut startup = StartupCatalog::new();
        for (name, path) in [
            ("I2cTransaction", "machine/i2c/transact/request"),
            ("I2cResult", "machine/i2c/transact/result"),
        ] {
            let ty = self
                .types
                .iter()
                .find(|ty| ty.name == name)
                .expect("checked I2C Type");
            startup
                .insert_checked_native_type(path, ty)
                .expect("I2C import");
        }
        startup
            .insert(conduit_plot::KindSignature {
                kind: I2C_KIND.into(),
                startup_parameters: vec![],
            })
            .expect("I2C startup");
        startup
            .insert_fore(I2C_KIND, self.kind.checked_front())
            .expect("I2C Fore");
        let mut profile = ProfileCatalog::new();
        profile.insert_kind(self.kind.clone()).expect("I2C Kind");
        (startup, profile)
    }
}
