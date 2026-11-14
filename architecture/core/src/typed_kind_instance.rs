//! Exact semantic identity of a prepared instance of a generic Kind family.
//! Identity construction creates no implementation, availability or authority.
use crate::{CheckedValueContract, KindId, StructuredInfoRefusal, StructuredInfoType};
use alloc::format;
use sha2::{Digest, Sha256};

/// Preserve the original protocol typed-instance identity byte format. Schemas
/// and full value contracts are ordered inputs; Source aliases do not enter it.
/// This hashes supplied checked contracts, without widening or revalidating them.
pub fn typed_kind_instance_id<'a>(
    family: &str,
    values: impl ExactSizeIterator<Item = (&'a StructuredInfoType, &'a CheckedValueContract)>,
) -> Result<KindId, StructuredInfoRefusal> {
    let mut digest = Sha256::new();
    digest.update(b"conduit.protocol/typed-kind-instance@1\0");
    digest.update((family.len() as u32).to_le_bytes());
    digest.update(family.as_bytes());
    digest.update((values.len() as u32).to_le_bytes());
    for (schema, contract) in values {
        let schema = schema.canonical_bytes()?;
        let contract = contract.identity_bytes();
        digest.update((schema.len() as u32).to_le_bytes());
        digest.update(schema);
        digest.update((contract.len() as u32).to_le_bytes());
        digest.update(contract);
    }
    Ok(format!("{family}/typed-{:x}", digest.finalize()).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kind_id, StructuredFieldType, ValueConstraint, BOOL_INFO_ID};

    // Frozen pre-extraction algorithm from ConduitOS protocol_source/instances.
    // Keep this independent of the shared helper, including its length framing.
    fn original(family: &str, values: &[(&StructuredInfoType, &CheckedValueContract)]) -> KindId {
        let mut digest = Sha256::new();
        digest.update(b"conduit.protocol/typed-kind-instance@1\0");
        digest.update((family.len() as u32).to_le_bytes());
        digest.update(family.as_bytes());
        digest.update((values.len() as u32).to_le_bytes());
        for (schema, contract) in values {
            let schema = schema.canonical_bytes().unwrap();
            let contract = contract.identity_bytes();
            digest.update((schema.len() as u32).to_le_bytes());
            digest.update(schema);
            digest.update((contract.len() as u32).to_le_bytes());
            digest.update(contract);
        }
        format!("{family}/typed-{:x}", digest.finalize()).into()
    }
    fn value(schema: &StructuredInfoType, maximum_bytes: u32) -> CheckedValueContract {
        CheckedValueContract::new(
            schema.profile().unwrap().value_kind().clone(),
            maximum_bytes,
            alloc::vec![],
        )
        .unwrap()
    }

    #[test]
    fn shared_identity_is_byte_compatible_with_frozen_protocol_algorithm() {
        let boolean = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
        let count = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
        let (b, n) = (value(&boolean, 1), value(&count, 8));
        for family in [
            "state/seeded/flow/until",
            "flow/merge/finite",
            "flow/zip/feedback",
            "flow/zip/finite",
        ] {
            for values in [
                alloc::vec![],
                alloc::vec![(&boolean, &b)],
                alloc::vec![(&boolean, &b), (&count, &n)],
                alloc::vec![(&count, &n), (&boolean, &b)],
            ] {
                assert_eq!(
                    typed_kind_instance_id(family, values.iter().copied()).unwrap(),
                    original(family, &values)
                );
            }
        }
    }

    #[test]
    fn family_schema_order_and_complete_contract_remain_identity_inputs() {
        let boolean = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
        let count = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
        let (b, n) = (value(&boolean, 1), value(&count, 8));
        let identity = |family, values: &[(&StructuredInfoType, &CheckedValueContract)]| {
            typed_kind_instance_id(family, values.iter().copied()).unwrap()
        };
        let original = identity("flow/zip/feedback", &[(&boolean, &b), (&count, &n)]);
        assert_ne!(
            original,
            identity("flow/zip/finite", &[(&boolean, &b), (&count, &n)])
        );
        assert_ne!(
            original,
            identity("flow/zip/feedback", &[(&count, &n), (&boolean, &b)])
        );
        let wider = value(&boolean, 2);
        assert_ne!(
            original,
            identity("flow/zip/feedback", &[(&boolean, &wider), (&count, &n)])
        );
        let mut constrained = b.clone();
        constrained.constraints.push(ValueConstraint::ByteLength {
            minimum: 0,
            maximum: 1,
        });
        assert_ne!(
            original,
            identity(
                "flow/zip/feedback",
                &[(&boolean, &constrained), (&count, &n)]
            )
        );
        let record = |field| {
            StructuredInfoType::record(
                kind_id("test/instance-record"),
                alloc::vec![StructuredFieldType::new(field, boolean.clone()).unwrap()],
            )
            .unwrap()
        };
        let (first, second) = (record("first"), record("second"));
        let contract = value(&first, 64);
        // Hold the nominal envelope fixed to isolate canonical schema bytes.
        assert_ne!(
            identity("flow/merge/finite", &[(&first, &contract)]),
            identity("flow/merge/finite", &[(&second, &contract)])
        );
    }
}
