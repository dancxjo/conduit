mod names;
use crate::prelude::*;
use crate::{CheckedNativeType, CheckedTypeForm};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{PrimitiveInfoKind, StructuredInfoType, StructuredInfoTypeShape};
use core::fmt::Write;
use names::{byte_literals, rust_screaming_identifier};
pub(super) use names::{rust_pascal_identifier, rust_snake_identifier};

pub use super::generate_options::RustBindingOptions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustBindingModule {
    pub source: String,
    pub semantic_type_bytes: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExternalNativeRustBinding<'a> {
    pub semantic_identity: &'a str,
    pub rust_type_path: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalRustBindingGenerationError {
    MissingExternalBinding(String),
    DuplicateExternalBinding(String),
    UnusedExternalBinding(String),
    ExternalBindingIdentityDrift(String),
    InvalidExternalRustPath(String),
    Generation(RustBindingGenerationError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RustBindingGenerationError {
    EmptyTypeSet,
    InvalidRustIdentifier(String),
    DuplicateRustIdentifier(String),
    InvalidSemanticType,
    UnsupportedLeaf(String),
}

pub fn generate_rust_bindings(
    types: &[CheckedNativeType],
    options: &RustBindingOptions,
) -> Result<RustBindingModule, RustBindingGenerationError> {
    generate_rust_bindings_with_forms(types, &[], options)
}

pub fn generate_rust_bindings_with_forms(
    types: &[CheckedNativeType],
    type_forms: &[CheckedTypeForm],
    options: &RustBindingOptions,
) -> Result<RustBindingModule, RustBindingGenerationError> {
    generate_rust_bindings_with_external_names(types, type_forms, options, &BTreeMap::new())
}

/// Generates bindings for source-owned Types while reusing exact native Rust
/// bindings for referenced Types owned by another semantic crate.
pub fn generate_rust_bindings_with_external_bindings(
    types: &[CheckedNativeType],
    external_types: &[StructuredInfoType],
    external_bindings: &[ExternalNativeRustBinding<'_>],
    options: &RustBindingOptions,
) -> Result<RustBindingModule, ExternalRustBindingGenerationError> {
    let external_names = validate_external_bindings(types, external_types, external_bindings)?;
    generate_rust_bindings_with_external_names(types, &[], options, &external_names)
        .map_err(ExternalRustBindingGenerationError::Generation)
}

/// Generates bindings and compact Forms while reusing exact bindings owned by
/// other semantic crates.
pub fn generate_rust_bindings_with_forms_and_external_bindings(
    types: &[CheckedNativeType],
    type_forms: &[CheckedTypeForm],
    external_types: &[StructuredInfoType],
    external_bindings: &[ExternalNativeRustBinding<'_>],
    options: &RustBindingOptions,
) -> Result<RustBindingModule, ExternalRustBindingGenerationError> {
    let external_names = validate_external_bindings(types, external_types, external_bindings)?;
    generate_rust_bindings_with_external_names(types, type_forms, options, &external_names)
        .map_err(ExternalRustBindingGenerationError::Generation)
}

pub(super) fn validate_external_bindings(
    types: &[CheckedNativeType],
    external_types: &[StructuredInfoType],
    external_bindings: &[ExternalNativeRustBinding<'_>],
) -> Result<BTreeMap<String, String>, ExternalRustBindingGenerationError> {
    let owned = types
        .iter()
        .map(|value_type| value_type.identity.as_str().to_string())
        .collect::<BTreeSet<_>>();
    let mut available = BTreeSet::new();
    for value_type in external_types {
        collect_root_schema_identity(value_type, &mut available);
    }
    let mut required = BTreeSet::new();
    for value_type in types {
        collect_external_boundaries(&value_type.value_type, &owned, &available, &mut required);
    }
    let mut external_names = BTreeMap::new();
    for binding in external_bindings {
        if !super::generate_package::valid_rust_type_path(binding.rust_type_path) {
            return Err(ExternalRustBindingGenerationError::InvalidExternalRustPath(
                binding.rust_type_path.into(),
            ));
        }
        if external_names
            .insert(
                binding.semantic_identity.into(),
                binding.rust_type_path.into(),
            )
            .is_some()
        {
            return Err(
                ExternalRustBindingGenerationError::DuplicateExternalBinding(
                    binding.semantic_identity.into(),
                ),
            );
        }
        if !available.contains(binding.semantic_identity) {
            return Err(
                ExternalRustBindingGenerationError::ExternalBindingIdentityDrift(
                    binding.semantic_identity.into(),
                ),
            );
        }
        if !required.contains(binding.semantic_identity) {
            return Err(ExternalRustBindingGenerationError::UnusedExternalBinding(
                binding.semantic_identity.into(),
            ));
        }
    }
    if let Some(missing) = required
        .iter()
        .find(|identity| !external_names.contains_key(identity.as_str()))
    {
        return Err(ExternalRustBindingGenerationError::MissingExternalBinding(
            missing.clone(),
        ));
    }
    Ok(external_names)
}

fn collect_root_schema_identity(
    value_type: &StructuredInfoType,
    identities: &mut BTreeSet<String>,
) {
    match value_type.shape() {
        StructuredInfoTypeShape::Nominal { schema, .. }
        | StructuredInfoTypeShape::Record { schema, .. }
        | StructuredInfoTypeShape::Variant { schema, .. } => {
            identities.insert(schema.as_str().into());
        }
        StructuredInfoTypeShape::Sequence { .. }
        | StructuredInfoTypeShape::Collection { .. }
        | StructuredInfoTypeShape::Leaf(_) => {}
    }
}

fn collect_external_boundaries(
    value_type: &StructuredInfoType,
    owned: &BTreeSet<String>,
    available: &BTreeSet<String>,
    required: &mut BTreeSet<String>,
) {
    let schema = match value_type.shape() {
        StructuredInfoTypeShape::Nominal { schema, .. }
        | StructuredInfoTypeShape::Record { schema, .. }
        | StructuredInfoTypeShape::Variant { schema, .. } => Some(schema.as_str()),
        _ => None,
    };
    if let Some(schema) = schema {
        if available.contains(schema) && !owned.contains(schema) {
            required.insert(schema.into());
            return;
        }
    }
    match value_type.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            collect_external_boundaries(representation, owned, available, required);
        }
        StructuredInfoTypeShape::Record { fields, .. } => {
            for field in fields {
                collect_external_boundaries(field.value_type(), owned, available, required);
            }
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            for case in cases {
                collect_external_boundaries(case.payload_type(), owned, available, required);
            }
        }
        StructuredInfoTypeShape::Sequence { element, .. }
        | StructuredInfoTypeShape::Collection { element, .. } => {
            collect_external_boundaries(element, owned, available, required);
        }
        StructuredInfoTypeShape::Leaf(_) => {}
    }
}

pub(super) fn generate_rust_bindings_with_external_names(
    types: &[CheckedNativeType],
    type_forms: &[CheckedTypeForm],
    options: &RustBindingOptions,
    external_names: &BTreeMap<String, String>,
) -> Result<RustBindingModule, RustBindingGenerationError> {
    if types.is_empty() {
        return Err(RustBindingGenerationError::EmptyTypeSet);
    }
    let mut names = external_names.clone();
    let mut seen = BTreeSet::new();
    let owned_identities = types
        .iter()
        .map(|value_type| value_type.identity.as_str().to_string())
        .collect::<BTreeSet<_>>();
    let mut semantic_type_bytes = BTreeMap::new();
    for value_type in types {
        let prefix = if options.type_prefix.is_empty() {
            String::new()
        } else {
            rust_pascal_identifier(&options.type_prefix)?
        };
        let rust_name = format!("{}{}", prefix, rust_pascal_identifier(&value_type.name)?);
        if !seen.insert(rust_name.clone()) {
            return Err(RustBindingGenerationError::DuplicateRustIdentifier(
                rust_name,
            ));
        }
        if names
            .insert(value_type.identity.as_str().to_string(), rust_name)
            .is_some()
        {
            return Err(RustBindingGenerationError::InvalidSemanticType);
        }
        semantic_type_bytes.insert(
            value_type.identity.as_str().to_string(),
            value_type
                .value_type
                .canonical_bytes()
                .map_err(|_| RustBindingGenerationError::InvalidSemanticType)?,
        );
    }
    validate_boxed_variant_payloads(types, options, external_names)?;
    super::generate_options::validate_inline_variants(types, options)?;

    let mut source = String::from(
        "// @generated by Conduit from checked semantic Types.\n\
         // Rust names and layout are bindings, never semantic identity.\n\
         extern crate alloc;\n\
         #[allow(unused_imports)]\n\
         use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeBindingRefusal, NativeFormRefusal, NativeRustBinding};\n\
         use conduit_plot::rust_binding::semantic_core as conduit_core;\n\
         #[allow(unused_imports)]\n\
         use conduit_core::{StructuredFieldValue, StructuredInfoType, StructuredInfoValue, StructuredInfoValueShape};\n\
         #[allow(unused_imports)]\n\
         use alloc::{boxed::Box, string::String, vec, vec::Vec};\n\n",
    );
    for value_type in types {
        emit_type(
            &mut source,
            value_type,
            &names,
            &semantic_type_bytes,
            &owned_identities,
            options,
        )?;
    }
    emit_forms(&mut source, type_forms, &names)?;
    Ok(RustBindingModule {
        source,
        semantic_type_bytes,
    })
}

fn emit_forms(
    out: &mut String,
    type_forms: &[CheckedTypeForm],
    names: &BTreeMap<String, String>,
) -> Result<(), RustBindingGenerationError> {
    let mut codec_names = BTreeSet::new();
    for form in type_forms {
        let rust_type = names
            .get(form.value_type.as_str())
            .ok_or(RustBindingGenerationError::InvalidSemanticType)?;
        let codec = format!("{rust_type}Form");
        if !codec_names.insert(codec.clone()) {
            return Err(RustBindingGenerationError::DuplicateRustIdentifier(codec));
        }
        writeln!(out, "pub struct {codec};").expect("String writing is infallible");
        writeln!(out, "impl {codec} {{").expect("String writing is infallible");
        writeln!(out, "    pub const NAME: &str = {:?};", form.name)
            .expect("String writing is infallible");
        writeln!(
            out,
            "    pub const IDENTITY: &str = {:?};",
            form.compatibility_id
        )
        .expect("String writing is infallible");
        writeln!(
            out,
            "    pub const EXACT_BYTES: usize = {};",
            form.exact_bytes
        )
        .expect("String writing is infallible");
        writeln!(
            out,
            "    pub const MAXIMUM_BYTES: usize = {};",
            form.maximum_bytes
        )
        .expect("String writing is infallible");
        writeln!(
            out,
            "    pub const MAXIMUM_DECODE_STEPS: usize = {};",
            form.maximum_decode_steps
        )
        .expect("String writing is infallible");
        writeln!(
            out,
            "    pub const fn encode(value: {rust_type}) -> [u8; 1] {{"
        )
        .expect("String writing is infallible");
        writeln!(out, "        [match value {{").expect("String writing is infallible");
        for mapping in &form.mappings {
            writeln!(
                out,
                "            {rust_type}::{} => {},",
                rust_pascal_identifier(&mapping.variant)?,
                mapping.discriminant
            )
            .expect("String writing is infallible");
        }
        writeln!(out, "        }}]").expect("String writing is infallible");
        writeln!(out, "    }}").expect("String writing is infallible");
        writeln!(
            out,
            "    pub fn decode(encoded: &[u8]) -> Result<{rust_type}, NativeFormRefusal> {{"
        )
        .expect("String writing is infallible");
        writeln!(out, "        let [tag] = encoded else {{ return Err(NativeFormRefusal::WrongLength {{ actual: encoded.len() }}); }};")
            .expect("String writing is infallible");
        writeln!(out, "        match *tag {{").expect("String writing is infallible");
        for mapping in &form.mappings {
            writeln!(
                out,
                "            {} => Ok({rust_type}::{}),",
                mapping.discriminant,
                rust_pascal_identifier(&mapping.variant)?
            )
            .expect("String writing is infallible");
        }
        writeln!(
            out,
            "            actual => Err(NativeFormRefusal::InvalidTag {{ actual }}),"
        )
        .expect("String writing is infallible");
        writeln!(out, "        }}").expect("String writing is infallible");
        writeln!(out, "    }}\n}}\n").expect("String writing is infallible");
    }
    Ok(())
}

fn emit_type(
    out: &mut String,
    value_type: &CheckedNativeType,
    names: &BTreeMap<String, String>,
    bytes_by_identity: &BTreeMap<String, Vec<u8>>,
    owned_identities: &BTreeSet<String>,
    options: &RustBindingOptions,
) -> Result<(), RustBindingGenerationError> {
    let rust_name = &names[value_type.identity.as_str()];
    let constant = format!("{}_SEMANTIC_TYPE", rust_screaming_identifier(rust_name)?);
    let bytes = &bytes_by_identity[value_type.identity.as_str()];
    writeln!(
        out,
        "pub const {constant}: &[u8] = &[{}];",
        byte_literals(bytes)
    )
    .expect("String writing is infallible");
    writeln!(
        out,
        "pub const {constant}_IDENTITY: &str = {:?};",
        value_type.identity.as_str()
    )
    .expect("String writing is infallible");

    match value_type.value_type.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            let inner = rust_type(representation, names)?;
            let derive_copy = options.copy_nominal_types.contains(&value_type.name);
            let derive_hash = options.hash_nominal_types.contains(&value_type.name);
            let derive_serde = options.serde_nominal_types.contains(&value_type.name);
            if derive_copy && !copy_type(representation)
                || derive_hash && !hash_type(representation)
            {
                return Err(RustBindingGenerationError::InvalidSemanticType);
            }
            let derives = match (derive_copy, derive_hash, derive_serde) {
                (true, true, true) => {
                    "Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize"
                }
                (true, false, true) => {
                    "Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize"
                }
                (false, true, true) => {
                    "Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize"
                }
                (false, false, true) => {
                    "Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize"
                }
                (true, true, false) => "Debug, Clone, Copy, PartialEq, Eq, Hash",
                (true, false, false) => "Debug, Clone, Copy, PartialEq, Eq",
                (false, true, false) => "Debug, Clone, PartialEq, Eq, Hash",
                (false, false, false) => "Debug, Clone, PartialEq, Eq",
            };
            writeln!(
                out,
                "#[derive({derives})]{}\npub struct {rust_name}({inner});",
                if derive_serde {
                    "\n#[serde(transparent)]"
                } else {
                    ""
                }
            )
            .expect("String writing is infallible");
            writeln!(out, "impl {rust_name} {{").expect("String writing is infallible");
            writeln!(out, "    pub const fn get(&self) -> &{inner} {{ &self.0 }}")
                .expect("String writing is infallible");
            writeln!(out, "}}\n").expect("String writing is infallible");
        }
        StructuredInfoTypeShape::Record { fields, .. } => {
            let derive_copy = options.copy_record_types.contains(&value_type.name);
            let derive_serde = options.serde_record_types.contains(&value_type.name);
            let deny_unknown = options
                .serde_deny_unknown_record_types
                .contains(&value_type.name);
            if deny_unknown && !derive_serde {
                return Err(RustBindingGenerationError::InvalidSemanticType);
            }
            if derive_copy && !copy_type(&value_type.value_type) {
                return Err(RustBindingGenerationError::InvalidSemanticType);
            }
            let derives = match (derive_copy, derive_serde) {
                (true, true) => {
                    "Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize"
                }
                (true, false) => "Debug, Clone, Copy, PartialEq, Eq",
                (false, true) => {
                    "Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize"
                }
                (false, false) => "Debug, Clone, PartialEq, Eq",
            };
            writeln!(
                out,
                "#[derive({derives})]{}\npub struct {rust_name} {{",
                if deny_unknown {
                    "\n#[serde(deny_unknown_fields)]"
                } else {
                    ""
                }
            )
            .expect("String writing is infallible");
            for field in fields {
                writeln!(
                    out,
                    "    {}{}: {},",
                    if options.public_record_fields.contains(&value_type.name) {
                        "pub "
                    } else {
                        ""
                    },
                    rust_snake_identifier(field.name())?,
                    rust_type(field.value_type(), names)?
                )
                .expect("String writing is infallible");
            }
            writeln!(out, "}}\n").expect("String writing is infallible");
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            let derive_serde = options.derive_serde_for_variants
                && !options.serde_variant_exclusions.contains(&value_type.name);
            let ordered_cases =
                if let Some(order) = options.serde_variant_orders.get(&value_type.name) {
                    if order.len() != cases.len() {
                        return Err(RustBindingGenerationError::InvalidSemanticType);
                    }
                    let mut seen = BTreeSet::new();
                    let mut ordered = Vec::with_capacity(cases.len());
                    for tag in order {
                        if !seen.insert(tag) {
                            return Err(RustBindingGenerationError::InvalidSemanticType);
                        }
                        ordered.push(
                            cases
                                .iter()
                                .find(|case| case.tag() == tag)
                                .ok_or(RustBindingGenerationError::InvalidSemanticType)?,
                        );
                    }
                    ordered
                } else {
                    cases.iter().collect::<Vec<_>>()
                };
            let unit_only = ordered_cases
                .iter()
                .all(|case| unit_type(case.payload_type()));
            for case in &ordered_cases {
                if matches!(
                    case.payload_type().shape(),
                    StructuredInfoTypeShape::Record { .. }
                ) {
                    let payload = format!("{rust_name}{}", rust_pascal_identifier(case.tag())?);
                    emit_payload_struct(out, &payload, case.payload_type(), names, derive_serde)?;
                }
            }
            let copy_payloads = ordered_cases.iter().all(|case| {
                !boxed_variant_payload(options, &value_type.name, case.tag())
                    && !matches!(
                        case.payload_type().shape(),
                        StructuredInfoTypeShape::Record { .. }
                    )
                    && !references_external_type(case.payload_type(), owned_identities)
                    && copy_type(case.payload_type())
            });
            let derives = if unit_only && derive_serde {
                "Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize"
            } else if unit_only {
                "Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash"
            } else if copy_payloads && derive_serde {
                "Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize"
            } else if copy_payloads {
                "Debug, Clone, Copy, PartialEq, Eq"
            } else if derive_serde {
                "Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize"
            } else {
                "Debug, Clone, PartialEq, Eq"
            };
            super::generate_options::emit_inline_variant_attribute(out, &value_type.name, options);
            writeln!(out, "#[derive({derives})]\npub enum {rust_name} {{")
                .expect("String writing is infallible");
            for case in &ordered_cases {
                let variant = rust_pascal_identifier(case.tag())?;
                if unit_type(case.payload_type()) {
                    writeln!(out, "    {variant},").expect("String writing is infallible");
                } else if matches!(
                    case.payload_type().shape(),
                    StructuredInfoTypeShape::Record { .. }
                ) {
                    let payload = format!("{rust_name}{variant}");
                    let payload = if boxed_variant_payload(options, &value_type.name, case.tag()) {
                        format!("Box<{payload}>")
                    } else {
                        payload
                    };
                    writeln!(out, "    {variant}({payload}),")
                        .expect("String writing is infallible");
                } else {
                    let payload = rust_type(case.payload_type(), names)?;
                    let payload = if boxed_variant_payload(options, &value_type.name, case.tag()) {
                        format!("Box<{payload}>")
                    } else {
                        payload
                    };
                    writeln!(out, "    {variant}({payload}),",)
                        .expect("String writing is infallible");
                }
            }
            writeln!(out, "}}\n").expect("String writing is infallible");
        }
        _ => return Err(RustBindingGenerationError::InvalidSemanticType),
    }
    emit_semantic_type_impl(out, rust_name, &constant);
    super::generate_value::emit_value_impl(
        out,
        value_type,
        rust_name,
        &constant,
        names,
        owned_identities,
        super::generate_value::RecordBindingOptions {
            nominal_copy: options.copy_nominal_types.contains(&value_type.name),
            copy: options.copy_record_types.contains(&value_type.name),
            value_getters: options.copy_record_value_getters.contains(&value_type.name),
            direct_checked: options
                .direct_checked_record_constructors
                .contains(&value_type.name),
            constructor_order: options
                .record_constructor_orders
                .get(&value_type.name)
                .map(Vec::as_slice),
            constructor_name: options
                .record_constructor_names
                .get(&value_type.name)
                .map(String::as_str)
                .unwrap_or("new"),
            boxed_variant_payloads: &options.boxed_variant_payloads,
            authored_type_name: &value_type.name,
        },
    )?;
    Ok(())
}

pub(super) fn boxed_variant_payload(
    options: &RustBindingOptions,
    type_name: &str,
    case: &str,
) -> bool {
    options
        .boxed_variant_payloads
        .contains(&format!("{type_name}.{case}"))
}

fn validate_boxed_variant_payloads(
    types: &[CheckedNativeType],
    options: &RustBindingOptions,
    external_names: &BTreeMap<String, String>,
) -> Result<(), RustBindingGenerationError> {
    for path in &options.boxed_variant_payloads {
        let (type_name, case_name) = path
            .split_once('.')
            .ok_or(RustBindingGenerationError::InvalidSemanticType)?;
        let value_type = types
            .iter()
            .find(|value| value.name == type_name)
            .ok_or(RustBindingGenerationError::InvalidSemanticType)?;
        let StructuredInfoTypeShape::Variant { cases, .. } = value_type.value_type.shape() else {
            return Err(RustBindingGenerationError::InvalidSemanticType);
        };
        let case = cases
            .iter()
            .find(|case| case.tag() == case_name)
            .ok_or(RustBindingGenerationError::InvalidSemanticType)?;
        let StructuredInfoTypeShape::Record { schema, .. } = case.payload_type().shape() else {
            return Err(RustBindingGenerationError::InvalidSemanticType);
        };
        if external_names.contains_key(schema.as_str()) {
            return Err(RustBindingGenerationError::InvalidSemanticType);
        }
    }
    Ok(())
}

pub(super) fn references_external_type(
    value_type: &StructuredInfoType,
    owned_identities: &BTreeSet<String>,
) -> bool {
    match value_type.shape() {
        StructuredInfoTypeShape::Nominal {
            schema,
            representation,
        } => {
            !owned_identities.contains(schema.as_str())
                || references_external_type(representation, owned_identities)
        }
        StructuredInfoTypeShape::Record { schema, fields } => {
            !owned_identities.contains(schema.as_str())
                || fields
                    .iter()
                    .any(|field| references_external_type(field.value_type(), owned_identities))
        }
        StructuredInfoTypeShape::Variant { schema, cases } => {
            schema.as_str() != "conduit.conduitese.optional.v1"
                && !owned_identities.contains(schema.as_str())
                || cases
                    .iter()
                    .any(|case| references_external_type(case.payload_type(), owned_identities))
        }
        StructuredInfoTypeShape::Sequence { element, .. }
        | StructuredInfoTypeShape::Collection { element, .. } => {
            references_external_type(element, owned_identities)
        }
        StructuredInfoTypeShape::Leaf(_) => false,
    }
}

fn emit_payload_struct(
    out: &mut String,
    name: &str,
    value_type: &StructuredInfoType,
    names: &BTreeMap<String, String>,
    derive_serde: bool,
) -> Result<(), RustBindingGenerationError> {
    let StructuredInfoTypeShape::Record { fields, .. } = value_type.shape() else {
        return Err(RustBindingGenerationError::InvalidSemanticType);
    };
    let derives = if derive_serde {
        "Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize"
    } else {
        "Debug, Clone, PartialEq, Eq"
    };
    writeln!(out, "#[derive({derives})]\npub struct {name} {{")
        .expect("String writing is infallible");
    for field in fields {
        writeln!(
            out,
            "    {}: {},",
            rust_snake_identifier(field.name())?,
            rust_type(field.value_type(), names)?
        )
        .expect("String writing is infallible");
    }
    writeln!(out, "}}\n\nimpl {name} {{").expect("String writing is infallible");
    for field in fields {
        let field_name = rust_snake_identifier(field.name())?;
        let field_type = rust_type(field.value_type(), names)?;
        writeln!(
            out,
            "    pub fn {field_name}(&self) -> &{field_type} {{ &self.{field_name} }}"
        )
        .expect("String writing is infallible");
    }
    writeln!(out, "}}\n").expect("String writing is infallible");
    Ok(())
}

pub(super) fn copy_type(value_type: &StructuredInfoType) -> bool {
    match value_type.shape() {
        StructuredInfoTypeShape::Leaf(kind) => !matches!(
            conduit_core::primitive_info_kind(kind.as_str()),
            Some(PrimitiveInfoKind::Text | PrimitiveInfoKind::Bytes) | None
        ),
        StructuredInfoTypeShape::Nominal { representation, .. } => copy_type(representation),
        StructuredInfoTypeShape::Record { fields, .. } => {
            fields.iter().all(|field| copy_type(field.value_type()))
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            cases.iter().all(|case| copy_type(case.payload_type()))
        }
        StructuredInfoTypeShape::Sequence { .. } => false,
        StructuredInfoTypeShape::Collection { element, .. } => copy_type(element),
    }
}

fn hash_type(value_type: &StructuredInfoType) -> bool {
    match value_type.shape() {
        StructuredInfoTypeShape::Leaf(kind) => matches!(
            conduit_core::primitive_info_kind(kind.as_str()),
            Some(
                PrimitiveInfoKind::Unit
                    | PrimitiveInfoKind::CancellationRequest
                    | PrimitiveInfoKind::Bool
                    | PrimitiveInfoKind::Count
                    | PrimitiveInfoKind::Text
                    | PrimitiveInfoKind::I8
                    | PrimitiveInfoKind::U8
                    | PrimitiveInfoKind::I16
                    | PrimitiveInfoKind::U16
                    | PrimitiveInfoKind::I32
                    | PrimitiveInfoKind::U32
                    | PrimitiveInfoKind::I64
                    | PrimitiveInfoKind::U64
                    | PrimitiveInfoKind::I128
                    | PrimitiveInfoKind::U128
                    | PrimitiveInfoKind::F32
                    | PrimitiveInfoKind::F64
            )
        ),
        StructuredInfoTypeShape::Nominal { representation, .. } => hash_type(representation),
        StructuredInfoTypeShape::Collection { element, .. } => hash_type(element),
        StructuredInfoTypeShape::Record { .. }
        | StructuredInfoTypeShape::Variant { .. }
        | StructuredInfoTypeShape::Sequence { .. } => false,
    }
}

fn emit_semantic_type_impl(out: &mut String, rust_name: &str, constant: &str) {
    writeln!(out, "impl {rust_name} {{").expect("String writing is infallible");
    writeln!(
        out,
        "    pub fn semantic_type() -> Result<StructuredInfoType, NativeBindingRefusal> {{"
    )
    .expect("String writing is infallible");
    writeln!(out, "        StructuredInfoType::from_canonical_bytes({constant}).map_err(NativeBindingRefusal::InvalidSemanticType)")
        .expect("String writing is infallible");
    writeln!(out, "    }}\n}}\n").expect("String writing is infallible");
}

pub(super) fn rust_type(
    value_type: &StructuredInfoType,
    names: &BTreeMap<String, String>,
) -> Result<String, RustBindingGenerationError> {
    match value_type.shape() {
        StructuredInfoTypeShape::Nominal { schema, .. }
        | StructuredInfoTypeShape::Record { schema, .. }
        | StructuredInfoTypeShape::Variant { schema, .. }
            if names.contains_key(schema.as_str()) =>
        {
            Ok(names[schema.as_str()].clone())
        }
        StructuredInfoTypeShape::Sequence {
            element,
            maximum_items,
            ..
        } => Ok(format!(
            "BoundedSequence<{}, {}>",
            rust_type(element, names)?,
            maximum_items
        )),
        StructuredInfoTypeShape::Collection { element, length } => {
            Ok(format!("[{}; {}]", rust_type(element, names)?, length))
        }
        StructuredInfoTypeShape::Variant { schema, cases }
            if schema.as_str() == "conduit.conduitese.optional.v1" =>
        {
            let some = cases
                .iter()
                .find(|case| case.tag() == "some")
                .ok_or(RustBindingGenerationError::InvalidSemanticType)?;
            Ok(format!(
                "Option<{}>",
                rust_type(some.payload_type(), names)?
            ))
        }
        StructuredInfoTypeShape::Leaf(kind) => primitive_rust_type(kind.as_str()),
        _ => Err(RustBindingGenerationError::InvalidSemanticType),
    }
}

pub(super) fn primitive_rust_type(identity: &str) -> Result<String, RustBindingGenerationError> {
    let value = match conduit_core::primitive_info_kind(identity) {
        Some(PrimitiveInfoKind::Unit | PrimitiveInfoKind::CancellationRequest) => "()",
        Some(PrimitiveInfoKind::Bool) => "bool",
        Some(PrimitiveInfoKind::Count) => "u64",
        Some(PrimitiveInfoKind::Scalar) => "conduit_core::Scalar",
        Some(PrimitiveInfoKind::Text) => "String",
        Some(PrimitiveInfoKind::Bytes) | None
            if identity.starts_with("data/generation-reference<") =>
        {
            "conduit_data::DataReference"
        }
        None if identity == conduit_core::RESOURCE_REFERENCE_INFO_ID => {
            "conduit_core::BoundedResourceRef"
        }
        // A generated primitive carrier must be able to retain every byte
        // value admitted by the checked semantic contract. Per-field and
        // nominal limits are still enforced by generated value contracts;
        // this binding capacity is the structured substrate's finite ceiling,
        // not an additional 4 KiB semantic restriction.
        Some(PrimitiveInfoKind::Bytes) => "BoundedBytes<65536>",
        Some(
            PrimitiveInfoKind::Quantity
            | PrimitiveInfoKind::Distance
            | PrimitiveInfoKind::Frequency
            | PrimitiveInfoKind::Duration
            | PrimitiveInfoKind::Voltage
            | PrimitiveInfoKind::Temperature
            | PrimitiveInfoKind::Angle
            | PrimitiveInfoKind::Ratio
            | PrimitiveInfoKind::PixelCount,
        ) => "conduit_core::Quantity",
        Some(PrimitiveInfoKind::QuantityUnit) => "conduit_core::QuantityUnit",
        Some(PrimitiveInfoKind::U8) => "u8",
        Some(PrimitiveInfoKind::U16) => "u16",
        Some(PrimitiveInfoKind::U32) => "u32",
        Some(PrimitiveInfoKind::U64) => "u64",
        Some(PrimitiveInfoKind::U128) => "u128",
        Some(PrimitiveInfoKind::I8) => "i8",
        Some(PrimitiveInfoKind::I16) => "i16",
        Some(PrimitiveInfoKind::I32) => "i32",
        Some(PrimitiveInfoKind::I64) => "i64",
        Some(PrimitiveInfoKind::I128) => "i128",
        Some(PrimitiveInfoKind::F32) => "conduit_core::IeeeF32",
        Some(PrimitiveInfoKind::F64) => "conduit_core::IeeeF64",
        Some(PrimitiveInfoKind::Terminal) | None => {
            return Err(RustBindingGenerationError::UnsupportedLeaf(identity.into()));
        }
    };
    Ok(value.into())
}

pub(super) fn data_reference_content_kind(identity: &str) -> Option<&str> {
    identity
        .strip_prefix("data/generation-reference<")
        .and_then(|identity| identity.strip_suffix('>'))
        .filter(|identity| !identity.is_empty())
}

pub(super) fn unit_type(value_type: &StructuredInfoType) -> bool {
    matches!(value_type.shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == conduit_core::UNIT_INFO_ID)
}
