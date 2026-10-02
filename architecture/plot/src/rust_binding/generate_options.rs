//! Target binding layout options, separate from semantic Type identity.
use super::generate::RustBindingGenerationError;
use crate::prelude::*;
use crate::CheckedNativeType;
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::StructuredInfoTypeShape;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RustBindingOptions {
    /// A Rust-only spelling prefix. It never participates in semantic identity.
    pub type_prefix: String,
    /// Adds form-only Serde derives to variants whose generated Rust payloads
    /// also support Serde.
    /// This never participates in semantic identity and requires the consuming
    /// crate to provide `serde` with derive support.
    pub derive_serde_for_variants: bool,
    /// Authored Type names whose generated Rust variants must retain a
    /// pre-existing non-Serde API even when other variants opt into Serde.
    pub serde_variant_exclusions: BTreeSet<String>,
    /// Authored record Type names whose Rust bindings retain a pre-existing
    /// Serde wire contract. This binding choice is not semantic Type truth.
    pub serde_record_types: BTreeSet<String>,
    /// Serde-enabled authored record Type names whose established wire API
    /// rejects fields absent from the semantic record.
    pub serde_deny_unknown_record_types: BTreeSet<String>,
    /// Authored record Type names whose entirely-copyable Rust bindings retain
    /// a pre-existing `Copy` API.
    pub copy_record_types: BTreeSet<String>,
    /// Authored nominal Type names whose copyable Rust representations retain
    /// a `Copy` binding. This target trait never changes semantic identity.
    pub copy_nominal_types: BTreeSet<String>,
    /// Authored nominal Type names whose hashable Rust representations retain
    /// a `Hash` binding. This target trait never changes semantic identity.
    pub hash_nominal_types: BTreeSet<String>,
    /// Authored nominal Types whose Rust newtypes retain transparent Serde.
    /// This target codec choice never participates in semantic identity.
    pub serde_nominal_types: BTreeSet<String>,
    /// Exact `Type.case` variant payloads boxed only in generated Rust layout.
    /// This target ownership choice never participates in semantic identity.
    pub boxed_variant_payloads: BTreeSet<String>,
    /// Exact authored variant Types that retain inline payloads to preserve an
    /// allocation-free runtime contract. Permits their intentional size disparity.
    /// Cannot be combined with boxed payloads of the same Type.
    pub inline_variant_types: BTreeSet<String>,
    /// Authored constrained record Types whose generated constructor validates
    /// direct primitive field bounds without materializing a structured value.
    /// Nested generated fields retain their own construction invariants.
    pub direct_checked_record_constructors: BTreeSet<String>,
    /// Copy record bindings whose generated getters retain an established
    /// by-value Rust API instead of returning references.
    pub copy_record_value_getters: BTreeSet<String>,
    /// Authored record Type names whose generated fields retain an established
    /// public struct-literal API. Public fields are Rust binding compatibility,
    /// never permission to skip the semantic owner's validation boundary.
    pub public_record_fields: BTreeSet<String>,
    /// Optional Rust constructor argument order for retaining an established
    /// record API. Semantic record identity remains canonically field-ordered.
    pub record_constructor_orders: BTreeMap<String, Vec<String>>,
    /// Optional Rust constructor names for records whose established public
    /// constructor maps domain-specific refusals before native validation.
    pub record_constructor_names: BTreeMap<String, String>,
    /// Optional Rust enum declaration order for preserving an established
    /// Serde variant-index ABI. Keys are authored Type names and values are an
    /// exhaustive, unique list of authored variant tags. This is binding-only
    /// compatibility truth and never changes native Type identity.
    pub serde_variant_orders: BTreeMap<String, Vec<String>>,
}

pub(super) fn validate_inline_variants(
    types: &[CheckedNativeType],
    options: &RustBindingOptions,
) -> Result<(), RustBindingGenerationError> {
    for name in &options.inline_variant_types {
        if !types.iter().any(|ty| {
            ty.name == *name
                && matches!(
                    ty.value_type.shape(),
                    StructuredInfoTypeShape::Variant { .. }
                )
        }) || options
            .boxed_variant_payloads
            .iter()
            .any(|payload| payload.split_once('.').is_some_and(|(ty, _)| ty == name))
        {
            return Err(RustBindingGenerationError::InvalidSemanticType);
        }
    }
    Ok(())
}

pub(super) fn emit_inline_variant_attribute(
    out: &mut String,
    name: &str,
    options: &RustBindingOptions,
) {
    if options.inline_variant_types.contains(name) {
        out.push_str("// Inline payloads preserve the owner's allocation-free runtime contract.\n#[allow(clippy::large_enum_variant)]\n");
    }
}
