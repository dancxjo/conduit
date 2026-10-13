//! Derive generic Back specializations from named, checked Source types.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolValueReference {
    pub type_name: String,
    pub maximum_bytes: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProtocolSpecializationRequest {
    Singleton {
        value: ProtocolValueReference,
    },
    SeededFlow {
        value: ProtocolValueReference,
    },
    SeededUntil {
        value: ProtocolValueReference,
    },
    SeededUntilInstance {
        alias: String,
        value: ProtocolValueReference,
    },
    FeedbackZipInstance {
        alias: String,
        paired_alias: String,
        left: ProtocolValueReference,
        right: ProtocolValueReference,
    },
    Concat {
        value: ProtocolValueReference,
    },
    Merge {
        value: ProtocolValueReference,
    },
    MergeInstance {
        alias: String,
        value: ProtocolValueReference,
    },
    Zip {
        left: ProtocolValueReference,
        right: ProtocolValueReference,
    },
    FeedbackZip {
        left: ProtocolValueReference,
        right: ProtocolValueReference,
    },
}

impl ProtocolSourcePackage {
    /// Prepare inert Source packaging metadata. No native resources or authority
    /// are selected here; final entry preparation still checks the complete plot.
    pub fn compile(
        source: String,
        requests: &[ProtocolSpecializationRequest],
    ) -> Result<Self, ProtocolSourceRefusal> {
        use ProtocolSourceRefusal as Error;
        if source.is_empty()
            || source.len() > MAXIMUM_SOURCE_BYTES
            || requests.len() > MAXIMUM_SPECIALIZATIONS
        {
            return Err(Error::Bounds);
        }
        let (mut startup, mut profile) = crate::i2c_base::contract::I2cContract::prepare()
            .map_err(Error::Contract)?
            .catalogs();
        install_clock(&mut startup, &mut profile)?;
        install_literal(&mut startup, &mut profile)?;
        usb_catalog::install(&mut startup, &mut profile)?;
        storage_catalog::install(&mut startup, &mut profile)?;
        crate::serial_base::contract::SerialDuplexContract::prepare()
            .map_err(Error::Contract)?
            .install_catalogs(&mut startup, &mut profile)
            .map_err(Error::Catalog)?;
        crate::ethernet_frame::contract::EthernetFrameContract::prepare()
            .map_err(Error::Contract)?
            .install_catalogs(&mut startup, &mut profile)
            .map_err(Error::Catalog)?;
        // The first pass checks only declarations. Generic kind Fore contracts
        // depend on these schemas; the full Source is checked after installing
        // the resulting specializations by PreparedProtocolEntry::prepare.
        let checked = conduit_plot::prepare_source_types(&parse_syntax_document(&source), &startup)
            .map_err(Error::Source)?;
        let value = |reference: &ProtocolValueReference| -> Result<ProtocolValue, Error> {
            if reference.type_name.is_empty()
                || reference.type_name.len() > 128
                || reference.maximum_bytes == 0
                || reference.maximum_bytes > 32768
            {
                return Err(Error::Bounds);
            }
            let schema = checked
                .named_type(&reference.type_name)
                .ok_or(Error::Specialization("unknown named Source type"))?
                .clone();
            let kind = schema
                .profile()
                .map_err(Error::Contract)?
                .value_kind()
                .clone();
            let contract = CheckedValueContract::new(kind, reference.maximum_bytes, vec![])
                .map_err(|_| Error::Specialization("invalid named type envelope"))?;
            Ok(ProtocolValue { schema, contract })
        };
        let mut imports = String::new();
        let mut specializations = Vec::with_capacity(requests.len());
        let mut aliases = alloc::collections::BTreeSet::new();
        let mut reserve_alias = |alias: &str| -> Result<(), Error> {
            if !aliases.insert(String::from(alias)) {
                return Err(Error::Catalog(
                    "typed instance Source alias already bound".into(),
                ));
            }
            Ok(())
        };
        for request in requests {
            use ProtocolSpecializationRequest as Request;
            specializations.push(match request {
                Request::Singleton { value: reference } => ProtocolSpecialization::Singleton {
                    value: value(reference)?,
                },
                Request::SeededFlow { value: reference } => ProtocolSpecialization::SeededFlow {
                    value: value(reference)?,
                },
                Request::SeededUntil { value: reference } => ProtocolSpecialization::SeededUntil {
                    value: value(reference)?,
                },
                Request::SeededUntilInstance {
                    alias,
                    value: reference,
                } => {
                    reserve_alias(alias)?;
                    let value = value(reference)?;
                    let kind = instances::instance_kind_id("state/seeded/flow/until", &[&value])
                        .map_err(Error::Catalog)?;
                    imports.push_str(
                        &instances::binding_import(&kind, alias).map_err(Error::Catalog)?,
                    );
                    ProtocolSpecialization::SeededUntilInstance { value }
                }
                Request::FeedbackZipInstance {
                    alias,
                    paired_alias,
                    left,
                    right,
                } => {
                    reserve_alias(alias)?;
                    reserve_alias(paired_alias)?;
                    let left = value(left)?;
                    let right = value(right)?;
                    let kind = instances::instance_kind_id(
                        conduit_semantic_catalog::FLOW_ZIP_FEEDBACK_KIND,
                        &[&left, &right],
                    )
                    .map_err(Error::Catalog)?;
                    imports.push_str(
                        &instances::binding_import(&kind, alias).map_err(Error::Catalog)?,
                    );
                    let paired = conduit_core::kind_id(&alloc::format!("{}/paired", kind.as_str()));
                    imports.push_str(
                        &instances::binding_import(&paired, paired_alias)
                            .map_err(Error::Catalog)?,
                    );
                    ProtocolSpecialization::FeedbackZipInstance { left, right }
                }
                Request::Concat { value: reference } => ProtocolSpecialization::Concat {
                    value: value(reference)?,
                },
                Request::Merge { value: reference } => ProtocolSpecialization::Merge {
                    value: value(reference)?,
                },
                Request::MergeInstance {
                    alias,
                    value: reference,
                } => {
                    reserve_alias(alias)?;
                    let value = value(reference)?;
                    let kind = instances::instance_kind_id(
                        conduit_semantic_catalog::FLOW_MERGE_FINITE_KIND,
                        &[&value],
                    )
                    .map_err(Error::Catalog)?;
                    imports.push_str(
                        &instances::binding_import(&kind, alias).map_err(Error::Catalog)?,
                    );
                    ProtocolSpecialization::MergeInstance { value }
                }
                Request::Zip { left, right } => ProtocolSpecialization::Zip {
                    left: value(left)?,
                    right: value(right)?,
                },
                Request::FeedbackZip { left, right } => ProtocolSpecialization::FeedbackZip {
                    left: value(left)?,
                    right: value(right)?,
                },
            });
        }
        Ok(Self {
            schema: PACKAGE_SCHEMA.into(),
            backs: vec![],
            source: alloc::format!("{imports}{source}"),
            specializations,
        })
    }
}

#[cfg(test)]
mod tests;
