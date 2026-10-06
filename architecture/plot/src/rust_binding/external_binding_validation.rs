//! Variant record carriers flatten payload fields even across ownership seams.
//! Their fields therefore need exact external Rust bindings independently of
//! the opaque record boundary used by ordinary record fields.
use super::generate::{ExternalNativeRustBinding, ExternalRustBindingGenerationError};
use crate::CheckedNativeType;
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};

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
        collect_variant_fields(&value_type.value_type, &owned, &available, &mut required);
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

fn collect_variant_fields(
    ty: &StructuredInfoType,
    owned: &BTreeSet<String>,
    available: &BTreeSet<String>,
    required: &mut BTreeSet<String>,
) {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        return;
    };
    for case in cases {
        if let StructuredInfoTypeShape::Record { fields, .. } = case.payload_type().shape() {
            for field in fields {
                collect_external_boundaries(field.value_type(), owned, available, required);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rust_binding::{
        generate_rust_bindings_with_external_bindings, ExternalNativeRustBinding,
        RustBindingOptions,
    };
    use crate::{check_syntax_document, parse_syntax_document, StartupCatalog};

    #[test]
    fn foreign_record_variant_payload_retains_exact_nested_owner_bindings() {
        let owner = check_syntax_document(&parse_syntax_document(
            "type OwnerId = Text <= 32B not in [\"\"]\ntype OwnerRef = {\n    identity: OwnerId\n}\n"
        ), &StartupCatalog::new()).unwrap();
        let mut catalog = StartupCatalog::new();
        for ty in &owner.native_types {
            catalog
                .insert_structured_type(&ty.name, ty.value_type.clone())
                .unwrap();
        }
        let consumer = check_syntax_document(
            &parse_syntax_document("type Consumer =\n    reference OwnerRef\n    | absent\n"),
            &catalog,
        )
        .unwrap();
        let external = owner
            .native_types
            .iter()
            .map(|ty| ty.value_type.clone())
            .collect::<alloc::vec::Vec<_>>();
        let bindings = owner
            .native_types
            .iter()
            .map(|ty| ExternalNativeRustBinding {
                semantic_identity: ty.identity.as_str(),
                rust_type_path: if ty.name == "OwnerId" {
                    "owner::OwnerId"
                } else {
                    "owner::OwnerRef"
                },
            })
            .collect::<alloc::vec::Vec<_>>();
        let generated = generate_rust_bindings_with_external_bindings(
            &consumer.native_types,
            &external,
            &bindings,
            &RustBindingOptions::default(),
        )
        .unwrap();
        assert!(generated.source.contains("identity: owner::OwnerId"));
        assert!(!generated.source.contains("pub struct OwnerId"));
        assert!(!generated.source.contains("pub struct OwnerRef"));
    }
}
