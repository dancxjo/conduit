//! Representation-only converter regeneration against retained original metadata.
use super::generate::{rust_pascal_identifier, RustBindingGenerationError as Error};
use super::generate_options::RustBindingOptions;
use crate::prelude::*;
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{KindId, StructuredInfoType, StructuredInfoTypeShape};

/// Complete original binding shape. This contains no admission metadata and is
/// never a substitute for the original descriptor's contracts or ordered laws.
pub struct PreparedNativeConverterShape<'a> {
    pub name: &'a str,
    pub identity: &'a KindId,
    pub value_type: &'a StructuredInfoType,
}

pub struct GeneratedPreparedNativeConverter {
    pub rust_name: String,
    /// Complete prepared impl, including unchanged ordinary conversion and the
    /// additive scoped conversion. It references original descriptor metadata.
    pub source: String,
}

/// Regenerates converter code without materializing unchanged law-program ASTs
/// or emitting replacement descriptors. This grants no metadata provenance or
/// Source authority. When updating a completed binding, callers must compare the
/// ordinary converter with its original before adding the scoped method, and
/// retain the complete original binding/layout/descriptor/law/contract closure.
/// Complete generated-family admission is still required at runtime.
pub fn generate_prepared_native_converters(
    shapes: &[PreparedNativeConverterShape<'_>],
    options: &RustBindingOptions,
) -> Result<Vec<GeneratedPreparedNativeConverter>, Error> {
    if shapes.is_empty() {
        return Err(Error::EmptyTypeSet);
    }
    if shapes.len() > super::MAXIMUM_GENERATED_NATIVE_FAMILY_TYPES {
        return Err(Error::InvalidSemanticType);
    }
    let prefix = if options.type_prefix.is_empty() {
        String::new()
    } else {
        rust_pascal_identifier(&options.type_prefix)?
    };
    let mut names = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for shape in shapes {
        let schema = match shape.value_type.shape() {
            StructuredInfoTypeShape::Record { schema, .. }
            | StructuredInfoTypeShape::Variant { schema, .. }
            | StructuredInfoTypeShape::Nominal { schema, .. } => schema,
            _ => return Err(Error::InvalidSemanticType),
        };
        if schema != shape.identity {
            return Err(Error::InvalidSemanticType);
        }
        let name = format!("{prefix}{}", rust_pascal_identifier(shape.name)?);
        if !seen.insert(name.clone()) {
            return Err(Error::DuplicateRustIdentifier(name));
        }
        if names
            .insert(shape.identity.as_str().to_string(), name)
            .is_some()
        {
            return Err(Error::InvalidSemanticType);
        }
    }
    shapes
        .iter()
        .map(|shape| {
            let rust_name = names[shape.identity.as_str()].clone();
            let mut source = String::new();
            super::generate_prepared_family::emit_converter(
                &mut source,
                shape.value_type,
                shape.name,
                &rust_name,
                &names,
                options,
            )?;
            Ok(GeneratedPreparedNativeConverter { rust_name, source })
        })
        .collect()
}
