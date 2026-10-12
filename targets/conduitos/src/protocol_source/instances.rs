//! Exact prepared identities for finite instances of generic semantic families.
use super::ProtocolValue;
use alloc::{format, string::String};
use conduit_core::KindId;
use sha2::{Digest, Sha256};

pub(crate) fn instance_kind_id(family: &str, values: &[&ProtocolValue]) -> Result<KindId, String> {
    let mut digest = Sha256::new();
    digest.update(b"conduit.protocol/typed-kind-instance@1\0");
    digest.update((family.len() as u32).to_le_bytes());
    digest.update(family.as_bytes());
    digest.update((values.len() as u32).to_le_bytes());
    for value in values {
        let schema = value
            .schema
            .canonical_bytes()
            .map_err(|error| format!("{error:?}"))?;
        let contract = value.contract.identity_bytes();
        digest.update((schema.len() as u32).to_le_bytes());
        digest.update(schema);
        digest.update((contract.len() as u32).to_le_bytes());
        digest.update(contract);
    }
    Ok(format!("{family}/typed-{:x}", digest.finalize()).into())
}

pub(crate) fn install_kind(
    kind: conduit_core::Kind,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    startup.insert(conduit_plot::KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: alloc::vec::Vec::new(),
    })?;
    startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
    profile
        .insert_kind(kind)
        .map_err(|error| format!("{error:?}"))
}

pub(crate) fn binding_import(kind: &KindId, alias: &str) -> Result<String, String> {
    if alias.is_empty()
        || alias.len() > 128
        || !alias
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        || !alias.as_bytes()[0].is_ascii_alphabetic()
    {
        return Err("typed instance requires a bounded Source alias".into());
    }
    Ok(format!("with {} as {}\n", kind.as_str(), alias))
}

use super::{ProtocolOperations, ProtocolSourceRefusal as Error};

pub(crate) fn install_until(
    value: &ProtocolValue,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
    operations: &mut ProtocolOperations,
) -> Result<conduit_core::CapabilityOffer, Error> {
    let mut kind = conduit_semantic_catalog::seeded_state_until_semantic_contract(
        &value.contract,
        &value.schema,
    )
    .map_err(Error::Specialization)?;
    kind.kind_id = instance_kind_id(kind.kind_id.as_str(), &[value]).map_err(Error::Catalog)?;
    install_kind(kind, startup, profile).map_err(Error::Catalog)?;
    operations
        .states
        .install_until_instance(&value.contract, &value.schema)
        .map_err(Error::Specialization)
}

pub(crate) fn install_zip(
    left: &ProtocolValue,
    right: &ProtocolValue,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
    operations: &mut ProtocolOperations,
    feedback: bool,
) -> Result<conduit_core::CapabilityOffer, Error> {
    let semantic = if feedback {
        conduit_semantic_catalog::flow_zip_feedback_semantic_contract
    } else {
        conduit_semantic_catalog::flow_zip_finite_semantic_contract
    };
    let mut kind = semantic(&left.contract, &left.schema, &right.contract, &right.schema)
        .map_err(Error::Specialization)?;
    kind.kind_id =
        instance_kind_id(kind.kind_id.as_str(), &[left, right]).map_err(Error::Catalog)?;
    let pair = conduit_core::PreparedTypedTuplePairEncoder::new(
        left.schema.clone(),
        left.contract.maximum_bytes,
        right.schema.clone(),
        right.contract.maximum_bytes,
    )
    .map_err(|error| Error::Catalog(alloc::format!("{error:?}")))?;
    startup
        .insert_structured_type(
            alloc::format!("{}/paired", kind.kind_id.as_str()),
            pair.value_type().clone(),
        )
        .map_err(Error::Catalog)?;
    install_kind(kind, startup, profile).map_err(Error::Catalog)?;
    let offer = if feedback {
        operations.joins.install_feedback_instance(
            &left.contract,
            &left.schema,
            &right.contract,
            &right.schema,
        )
    } else {
        operations.joins.install_instance(
            &left.contract,
            &left.schema,
            &right.contract,
            &right.schema,
        )
    };
    offer.map_err(Error::Catalog)
}

#[cfg(test)]
#[path = "instances_finite_tests.rs"]
mod finite_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol_source::{
        PreparedProtocolEntry, ProtocolSourcePackage, ProtocolSpecializationRequest,
        ProtocolValueReference,
    };
    #[test]
    fn two_exact_state_instances_prepare_without_overwriting_families() {
        let source = "type FirstState = {\n terminal: Boolean\n}\ntype SecondState = {\n count: U64\n terminal: Boolean\n}\nplot first-next (\n state: FirstState...| >> next: FirstState...|\n) = ({terminal: true})\nplot second-next (\n state: SecondState...| >> next: SecondState...|\n) = ({count: .count, terminal: true})\nplot first (\n >> seed: FirstState...|\n observed: FirstState...| >>\n) {\n cell: FirstCell\n seed >> cell.seed\n cell.current >> first-next() >> cell.next\n cell.current >> observed\n}\nplot second (\n >> seed: SecondState...|\n observed: SecondState...| >>\n) {\n cell: SecondCell\n seed >> cell.seed\n cell.current >> second-next() >> cell.next\n cell.current >> observed\n}";
        let requests = [("FirstCell", "FirstState"), ("SecondCell", "SecondState")].map(
            |(alias, type_name)| ProtocolSpecializationRequest::SeededUntilInstance {
                alias: alias.into(),
                value: ProtocolValueReference {
                    type_name: type_name.into(),
                    maximum_bytes: 4096,
                },
            },
        );
        let package = ProtocolSourcePackage::compile(source.into(), &requests).unwrap();
        let bytes = serde_json::to_vec(&package).unwrap();
        let first = PreparedProtocolEntry::prepare(&bytes, "first").unwrap();
        let second = PreparedProtocolEntry::prepare(&bytes, "second").unwrap();
        let first_id = &first
            .expanded()
            .expanded
            .gears
            .iter()
            .find(|g| {
                g.kind_id
                    .as_str()
                    .starts_with("state/seeded/flow/until/typed-")
            })
            .unwrap()
            .kind_id;
        let second_id = &second
            .expanded()
            .expanded
            .gears
            .iter()
            .find(|g| {
                g.kind_id
                    .as_str()
                    .starts_with("state/seeded/flow/until/typed-")
            })
            .unwrap()
            .kind_id;
        assert_ne!(first_id, second_id);
    }
    #[test]
    fn source_alias_cannot_inject_an_import_or_change_kind_identity() {
        let kind: KindId = "state/seeded/flow/until/typed-test".into();
        assert!(binding_import(&kind, "ProducerCell").is_ok());
        for alias in ["", "first\nwith other as Evil", "1Invalid", "a/b"] {
            assert!(binding_import(&kind, alias).is_err());
        }
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use conduit_core::{CheckedValueContract, StructuredInfoType, kind_id};
    #[test]
    fn family_contract_schema_and_pair_order_are_exact_instance_identity() {
        let boolean = ProtocolValue {
            contract: CheckedValueContract::new(kind_id("value/bool"), 1, alloc::vec![]).unwrap(),
            schema: StructuredInfoType::leaf(kind_id("value/bool")).unwrap(),
        };
        let mut wider = boolean.clone();
        wider.contract.maximum_bytes = 2;
        let original = instance_kind_id("state/seeded/flow/until", &[&boolean]).unwrap();
        assert_eq!(
            original,
            instance_kind_id("state/seeded/flow/until", &[&boolean]).unwrap()
        );
        assert_ne!(
            original,
            instance_kind_id("state/seeded/flow/until", &[&wider]).unwrap()
        );
        assert_ne!(
            original,
            instance_kind_id("flow/zip/feedback", &[&boolean]).unwrap()
        );
        assert_ne!(
            instance_kind_id("flow/zip/feedback", &[&boolean, &wider]).unwrap(),
            instance_kind_id("flow/zip/feedback", &[&wider, &boolean]).unwrap()
        );
    }
}

#[cfg(test)]
mod factory_tests {
    use conduit_core::*;
    #[test]
    fn legacy_and_typed_state_offers_coexist_and_unsupported_constraints_stay_refused() {
        let schema = StructuredInfoType::record(
            kind_id("test/state@1"),
            alloc::vec![
                StructuredFieldType::new(
                    "terminal",
                    StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap()
                )
                .unwrap()
            ],
        )
        .unwrap();
        let plain = CheckedValueContract::new(
            schema.profile().unwrap().value_kind().clone(),
            4096,
            alloc::vec![],
        )
        .unwrap();
        let first = CheckedValueContract::new(
            plain.value_kind.clone(),
            4096,
            alloc::vec![ValueConstraint::ByteLength {
                minimum: 0,
                maximum: 4096
            }],
        )
        .unwrap();
        let second = CheckedValueContract::new(
            plain.value_kind.clone(),
            4096,
            alloc::vec![ValueConstraint::ByteLength {
                minimum: 1,
                maximum: 4096
            }],
        )
        .unwrap();
        let mut factory = crate::seeded_state::SeededStateOperationFactory::default();
        let legacy = factory.install_until(&plain, &schema).unwrap();
        let a = factory.install_until_instance(&plain, &schema).unwrap();
        assert!(factory.install_until_instance(&first, &schema).is_err());
        assert!(factory.install_until_instance(&second, &schema).is_err());
        assert_ne!(legacy.capability_id, a.capability_id);
        assert_eq!(legacy.kind_id.as_str(), "state/seeded/flow/until");
        assert!(factory.install_until_instance(&plain, &schema).is_err());
        assert_eq!(factory.offers().count(), 2);
    }
}
