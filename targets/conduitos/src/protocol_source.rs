//! Bounded preparation of packaged Source and generic typed protocol Backs.
//! Packages contain no native resource possession or controller selection.
use crate::protocol_operations::ProtocolOperations;
use alloc::{string::String, vec, vec::Vec};
use conduit_core::{CapabilityOffer, CheckedValueContract, StructuredInfoType};
use conduit_plot::{
    CheckedSyntaxDocument, ProfileCatalog, StartupCatalog, check_syntax_document,
    parse_syntax_document,
};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_PACKAGE_BYTES: usize = 1024 * 1024;
pub const MAXIMUM_SOURCE_BYTES: usize = 256 * 1024;
pub const MAXIMUM_SPECIALIZATIONS: usize = 16;
/// Native containing-machine storage budget per external Fore boundary.
pub const MAXIMUM_PROTOCOL_FORE_BYTES: u32 = 4096;
pub const PACKAGE_SCHEMA: &str = "conduit.conduitos/protocol-source@1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolValue {
    #[serde(with = "schema_encoding")]
    pub schema: StructuredInfoType,
    pub contract: CheckedValueContract,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProtocolSpecialization {
    SeededFlow {
        value: ProtocolValue,
    },
    SeededUntil {
        value: ProtocolValue,
    },
    Concat {
        value: ProtocolValue,
    },
    Merge {
        value: ProtocolValue,
    },
    Zip {
        left: ProtocolValue,
        right: ProtocolValue,
    },
    FeedbackZip {
        left: ProtocolValue,
        right: ProtocolValue,
    },
}

/// Source owns protocol policy; metadata specializes only generic typed Backs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolSourcePackage {
    pub schema: String,
    pub source: String,
    pub specializations: Vec<ProtocolSpecialization>,
}

#[derive(Debug)]
pub enum ProtocolSourceRefusal {
    Bounds,
    Encoding,
    UnsupportedSchema,
    Contract(conduit_core::StructuredInfoRefusal),
    Specialization(&'static str),
    Catalog(String),
    Source(conduit_plot::SyntaxCheckDiagnostic),
    Expansion(conduit_plot::CanonicalExpansionDiagnostic),
    Offer,
    Plan(conduit_planner::PlannerError),
    Admission(crate::protocol_host_calls::ProtocolCallRefusal),
}

impl ProtocolSourcePackage {
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolSourceRefusal> {
        if bytes.is_empty() || bytes.len() > MAXIMUM_PACKAGE_BYTES {
            return Err(ProtocolSourceRefusal::Bounds);
        }
        let package: Self =
            serde_json::from_slice(bytes).map_err(|_| ProtocolSourceRefusal::Encoding)?;
        package.validate_bounds()?;
        Ok(package)
    }
    fn validate_bounds(&self) -> Result<(), ProtocolSourceRefusal> {
        if self.schema != PACKAGE_SCHEMA {
            return Err(ProtocolSourceRefusal::UnsupportedSchema);
        }
        if self.source.is_empty()
            || self.source.len() > MAXIMUM_SOURCE_BYTES
            || self.specializations.len() > MAXIMUM_SPECIALIZATIONS
        {
            return Err(ProtocolSourceRefusal::Bounds);
        }
        Ok(())
    }
}

pub struct PreparedProtocolSource {
    pub checked: CheckedSyntaxDocument,
    pub profile: ProfileCatalog,
    pub operations: ProtocolOperations,
    pub capabilities: Vec<CapabilityOffer>,
}

impl PreparedProtocolSource {
    /// Prepare pure code before planning. Native bases are admitted separately.
    pub fn prepare(package: ProtocolSourcePackage) -> Result<Self, ProtocolSourceRefusal> {
        use ProtocolSourceRefusal as Error;
        package.validate_bounds()?;
        let i2c = crate::i2c_base::contract::I2cContract::prepare().map_err(Error::Contract)?;
        let (mut startup, mut profile) = i2c.catalogs();
        install_clock(&mut startup, &mut profile)?;
        usb_catalog::install(&mut startup, &mut profile)?;
        let mut operations = ProtocolOperations::default();
        let mut capabilities = Vec::new();
        for specialization in &package.specializations {
            let offer = match specialization {
                ProtocolSpecialization::SeededFlow { value } => {
                    conduit_semantic_catalog::install_seeded_state_flow_kind(
                        &value.contract,
                        &value.schema,
                        &mut startup,
                        &mut profile,
                    )
                    .map_err(Error::Catalog)?;
                    operations
                        .states
                        .install_flow(&value.contract, &value.schema)
                        .map_err(Error::Specialization)
                }
                ProtocolSpecialization::SeededUntil { value } => {
                    conduit_semantic_catalog::install_seeded_state_until_kind(
                        &value.contract,
                        &value.schema,
                        &mut startup,
                        &mut profile,
                    )
                    .map_err(Error::Catalog)?;
                    operations
                        .states
                        .install_until(&value.contract, &value.schema)
                        .map_err(Error::Specialization)
                }
                ProtocolSpecialization::Concat { value } => {
                    conduit_semantic_catalog::install_flow_concat_finite_kind(
                        &value.contract,
                        &value.schema,
                        &mut startup,
                        &mut profile,
                    )
                    .map_err(Error::Catalog)?;
                    operations
                        .concats
                        .install(&value.contract, &value.schema)
                        .map_err(Error::Catalog)
                }
                ProtocolSpecialization::Merge { value } => {
                    conduit_semantic_catalog::install_flow_merge_finite_kind(
                        &value.contract,
                        &value.schema,
                        &mut startup,
                        &mut profile,
                    )
                    .map_err(Error::Catalog)?;
                    operations
                        .merges
                        .install(&value.contract, &value.schema)
                        .map_err(Error::Catalog)
                }
                ProtocolSpecialization::Zip { left, right } => {
                    conduit_semantic_catalog::install_flow_zip_finite_kind(
                        &left.contract,
                        &left.schema,
                        &right.contract,
                        &right.schema,
                        &mut startup,
                        &mut profile,
                    )
                    .map_err(Error::Catalog)?;
                    operations
                        .joins
                        .install(&left.contract, &left.schema, &right.contract, &right.schema)
                        .map_err(Error::Catalog)
                }
                ProtocolSpecialization::FeedbackZip { left, right } => {
                    conduit_semantic_catalog::install_flow_zip_feedback_kind(
                        &left.contract,
                        &left.schema,
                        &right.contract,
                        &right.schema,
                        &mut startup,
                        &mut profile,
                    )
                    .map_err(Error::Catalog)?;
                    operations
                        .joins
                        .install_feedback(
                            &left.contract,
                            &left.schema,
                            &right.contract,
                            &right.schema,
                        )
                        .map_err(Error::Catalog)
                }
            }?;
            capabilities.push(offer);
        }
        preparation_stage("catalogs");
        let checked = check_syntax_document(&parse_syntax_document(&package.source), &startup)
            .map_err(Error::Source)?;
        preparation_stage("checked");
        let mut selectors = alloc::collections::BTreeSet::new();
        for stage in checked
            .plots
            .iter()
            .flat_map(|plot| &plot.cords)
            .flat_map(|cord| &cord.stages)
        {
            if let conduit_plot::CheckedCordStage::StructuredSelector { selector, .. } = stage {
                let definition = conduit_plot::structured_selector_definition(
                    selector,
                    conduit_core::PortTemporal::Flow { closes: true },
                );
                if selectors.insert(definition.kind_id.clone()) {
                    profile
                        .insert(definition)
                        .map_err(|error| Error::Catalog(alloc::format!("{error:?}")))?;
                }
            }
        }
        Ok(Self {
            checked,
            profile,
            operations,
            capabilities,
        })
    }
}

fn preparation_stage(stage: &str) {
    #[cfg(all(target_os = "none", target_arch = "x86_64"))]
    {
        use core::fmt::Write;
        let mut text = crate::sign_format::FixedText::new();
        let _ = writeln!(
            text,
            "CONDUIT_PROTOCOL_PREPARATION {stage} live={} peak={}",
            crate::allocation::BOOT_ARENA.live_bytes(),
            crate::allocation::BOOT_ARENA.used()
        );
        crate::arch::early_write(text.as_bytes());
    }
    #[cfg(not(all(target_os = "none", target_arch = "x86_64")))]
    let _ = stage;
}

fn install_clock(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), ProtocolSourceRefusal> {
    use ProtocolSourceRefusal as Error;
    let clock = crate::monotonic_clock::contract::MonotonicClockContract::prepare()
        .map_err(Error::Contract)?;
    let types = check_syntax_document(
        &parse_syntax_document(crate::monotonic_clock::contract::CLOCK_TYPES),
        &StartupCatalog::new(),
    )
    .map_err(Error::Source)?;
    for (name, path) in [
        ("MonotonicClockRequest", "machine/clock/at/request"),
        ("MonotonicClockResult", "machine/clock/at/result"),
    ] {
        let ty = types
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .ok_or(Error::Specialization("clock type absent"))?;
        startup
            .insert_checked_native_type(path, ty)
            .map_err(Error::Catalog)?;
    }
    startup
        .insert(conduit_plot::KindSignature {
            kind: "machine/clock/at".into(),
            startup_parameters: vec![],
        })
        .map_err(Error::Catalog)?;
    startup
        .insert_fore("machine/clock/at", clock.kind().checked_front())
        .map_err(Error::Catalog)?;
    profile
        .insert_kind(clock.kind().clone())
        .map_err(|error| Error::Catalog(alloc::format!("{error:?}")))
}

mod schema_encoding {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        schema: &StructuredInfoType,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let bytes = schema
            .canonical_bytes()
            .map_err(|error| serde::ser::Error::custom(alloc::format!("{error:?}")))?;
        bytes.serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<StructuredInfoType, D::Error> {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        StructuredInfoType::from_canonical_bytes(&bytes)
            .map_err(|error| serde::de::Error::custom(alloc::format!("{error:?}")))
    }
}

#[cfg(test)]
mod tests;

mod usb_catalog;
mod usb_hid;
pub use usb_hid::{usb_hid_endpoint_package, usb_hid_keyboard_order_package};

mod planning;
pub use planning::ProtocolQueueLimits;

mod admission;
pub use admission::PreparedProtocolArtifact;

mod entry;
pub use entry::PreparedProtocolEntry;

mod boot_source;
pub use boot_source::{PROTOCOL_MODULE_COMMAND, ProtocolBootRefusal, prepare_boot_source};

mod issuer;
pub use issuer::{NativeProtocolIssueRefusal, NativeProtocolIssuer, NativeProtocolPossession};

mod body_play;
pub use body_play::{
    PreparedProtocolBodyPlay, ProtocolBodyPreparationRefusal, ProtocolBodyRefusal,
};

mod native_preparation;
pub use native_preparation::{
    NativeProtocolOwners, NativeProtocolPreparationLimits, NativeProtocolPreparationReason,
    NativeProtocolPreparationRefusal, prepare_native_protocol,
};

mod package_compilation;
pub use package_compilation::{ProtocolSpecializationRequest, ProtocolValueReference};
