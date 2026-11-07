//! Preparation-only contract derived from reviewed Conduit Types.

#[cfg(feature = "source-catalog")]
use alloc::{string::ToString, vec};
use conduit_core::{Kind, StructuredInfoRefusal, StructuredInfoType};
mod preparation;
pub use preparation::I2cContractPreparationRefusal;
include!(concat!(env!("OUT_DIR"), "/profiles.rs"));
#[cfg(feature = "source-catalog")]
use conduit_plot::{check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog};

pub const I2C_KIND: &str = "machine/i2c/transact";
pub const I2C_CALL: &str = "conduit.host/i2c-transact@1";
pub const I2C_MAXIMUM_BYTES: u32 = 4096;
// Derived from the checked Types on the fabrication host, not from a sampled
// value or the transport ceiling. The public Fore and capability bounds stay
// unchanged; these constants size valid canonical value storage only.
include!(concat!(env!("OUT_DIR"), "/value_bounds.rs"));
#[cfg(feature = "source-catalog")]
pub const I2C_TYPES: &str = include_str!("../../../../plots/device-protocols/i2c-types.conduit");

/// Exact compiled interface, prepared before Play. Schemas remain in flash. A definition is not an available Back,
/// selected endpoint, resource reservation or authority to perform a transfer.
pub struct I2cContract {
    kind: Kind,
}

impl I2cContract {
    /// Explicit authoring-side materialization. Execution uses immutable schemas.
    pub fn decode_request_type(&self) -> Result<StructuredInfoType, StructuredInfoRefusal> {
        StructuredInfoType::from_canonical_bytes(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/request.type"
        )))
    }
    pub fn decode_result_type(&self) -> Result<StructuredInfoType, StructuredInfoRefusal> {
        StructuredInfoType::from_canonical_bytes(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/result.type"
        )))
    }
    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    /// Authoring-only catalogs retain the checked refinement laws. MCU execution
    /// can omit this feature and its Source machinery entirely.
    #[cfg(feature = "source-catalog")]
    pub fn catalogs(&self) -> (StartupCatalog, ProfileCatalog) {
        let mut startup = StartupCatalog::new();
        let mut profiles = ProfileCatalog::new();
        self.install_catalogs(&mut startup, &mut profiles)
            .expect("build-checked I2C authoring catalogue");
        (startup, profiles)
    }

    /// Add only checked semantic declarations to an existing authoring context.
    /// No physical controller, available offer or transfer authority is created.
    /// Conflicts leave both caller catalogues unchanged.
    #[cfg(feature = "source-catalog")]
    pub fn install_catalogs(
        &self,
        startup: &mut StartupCatalog,
        profiles: &mut ProfileCatalog,
    ) -> Result<(), alloc::string::String> {
        let syntax = parse_syntax_document(I2C_TYPES);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new())
            .expect("build-checked I2C Type Source");
        let mut staged = startup.clone();
        let mut profile = profiles.clone();
        for (name, path) in [
            ("I2cTransaction", "machine/i2c/transact/request"),
            ("I2cResult", "machine/i2c/transact/result"),
        ] {
            let ty = checked
                .native_types
                .iter()
                .find(|ty| ty.name == name)
                .expect("checked I2C Type");
            staged.insert_checked_native_type_from_document(path, &checked, &ty.identity)?;
        }
        staged
            .insert(conduit_plot::KindSignature {
                kind: I2C_KIND.into(),
                startup_parameters: vec![],
            })
            .map_err(|error| error.to_string())?;
        staged
            .insert_fore(I2C_KIND, self.kind.checked_front())
            .map_err(|error| error.to_string())?;
        profile
            .insert_kind(self.kind.clone())
            .map_err(|error| error.to_string())?;
        *startup = staged;
        *profiles = profile;
        Ok(())
    }
}
