use crate::prelude::*;
use crate::NativeTypeValueContract;
use conduit_core::{KindId, StructuredFieldType, StructuredInfoType, StructuredVariantCase};
use sha2::{Digest, Sha256};

pub(super) fn schema_identity_for_record(
    name: &str,
    generic_context: Option<&str>,
    fields: &[StructuredFieldType],
    contracts: &[NativeTypeValueContract],
    invariants: &[crate::Expression],
) -> KindId {
    let mut canonical = b"conduit.native-type.record@1\0".to_vec();
    push(&mut canonical, name.as_bytes());
    push_generic_context(&mut canonical, generic_context);
    for field in fields {
        push(&mut canonical, field.name().as_bytes());
        push(
            &mut canonical,
            &field
                .value_type()
                .canonical_bytes()
                .expect("checked field type"),
        );
    }
    push_contracts(&mut canonical, contracts);
    let mut invariant_meanings = invariants
        .iter()
        .map(|invariant| crate::syntax_identity::canonical_expression(&invariant.syntax))
        .collect::<Vec<_>>();
    invariant_meanings.sort();
    for invariant in invariant_meanings {
        push(&mut canonical, invariant.as_bytes());
    }
    semantic_id(name, &canonical)
}

pub(super) fn schema_identity_for_variant(
    name: &str,
    generic_context: Option<&str>,
    cases: &[StructuredVariantCase],
) -> KindId {
    let mut canonical = b"conduit.native-type.variant@1\0".to_vec();
    push(&mut canonical, name.as_bytes());
    push_generic_context(&mut canonical, generic_context);
    for case in cases {
        push(&mut canonical, case.tag().as_bytes());
        push(
            &mut canonical,
            &case
                .payload_type()
                .canonical_bytes()
                .expect("checked case type"),
        );
    }
    semantic_id(name, &canonical)
}

pub(super) fn schema_identity(
    name: &str,
    generic_context: Option<&str>,
    representation: &StructuredInfoType,
    contracts: &[NativeTypeValueContract],
    invariants: &[crate::Expression],
) -> KindId {
    let mut canonical = b"conduit.native-type.scalar@1\0".to_vec();
    push(&mut canonical, name.as_bytes());
    push_generic_context(&mut canonical, generic_context);
    push(
        &mut canonical,
        &representation
            .canonical_bytes()
            .expect("checked representation"),
    );
    push_contracts(&mut canonical, contracts);
    push_invariants(&mut canonical, invariants);
    semantic_id(name, &canonical)
}

fn push_invariants(canonical: &mut Vec<u8>, invariants: &[crate::Expression]) {
    let mut meanings = invariants
        .iter()
        .map(|invariant| crate::syntax_identity::canonical_expression(&invariant.syntax))
        .collect::<Vec<_>>();
    meanings.sort();
    for meaning in meanings {
        push(canonical, meaning.as_bytes());
    }
}

fn push_generic_context(canonical: &mut Vec<u8>, generic_context: Option<&str>) {
    if let Some(generic_context) = generic_context {
        push(canonical, b"generic-instantiation@1");
        push(canonical, generic_context.as_bytes());
    }
}

fn push_contracts(canonical: &mut Vec<u8>, contracts: &[NativeTypeValueContract]) {
    for contract in contracts {
        push(canonical, contract.representation_path.as_bytes());
        push(canonical, &contract.contract.identity_bytes());
    }
}

fn push(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn semantic_id(name: &str, canonical: &[u8]) -> KindId {
    let digest = Sha256::digest(canonical);
    let mut suffix = String::with_capacity(64);
    for byte in digest {
        use core::fmt::Write;
        write!(&mut suffix, "{byte:02x}").expect("writing to String cannot fail");
    }
    KindId::from(alloc::format!("type/{name}@{suffix}"))
}
