//! Prepared leaf-law checks over borrowed canonical structure.
use super::*;
use crate::{CheckedValueContract, ConstraintDefinitionError, ValueConstraintRefusal};
use alloc::string::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredContractValidationRefusal {
    Structure(Refusal),
    InvalidContractPath,
    Definition(ConstraintDefinitionError),
    Constraint(ValueConstraintRefusal),
}

/// Preparation retains exact leaf contracts; Play borrows canonical bytes and
/// validates every active occurrence without materializing an owned value.
pub struct PreparedStructuredContractValidator {
    structure: PreparedStructuredValueValidator,
    contracts: Vec<(String, CheckedValueContract)>,
}

impl PreparedStructuredContractValidator {
    pub fn new(
        ty: &StructuredInfoType,
        maximum_bytes: usize,
        contracts: &[(String, CheckedValueContract)],
    ) -> Result<Self, StructuredContractValidationRefusal> {
        for (path, contract) in contracts {
            contract
                .validate_definition()
                .map_err(StructuredContractValidationRefusal::Definition)?;
            if !contract_path(ty, path, contract) {
                return Err(StructuredContractValidationRefusal::InvalidContractPath);
            }
        }
        Ok(Self {
            structure: PreparedStructuredValueValidator::new(ty, maximum_bytes)
                .map_err(StructuredContractValidationRefusal::Structure)?,
            contracts: contracts.to_vec(),
        })
    }
    pub fn validate(&self, bytes: &[u8]) -> Result<(), StructuredContractValidationRefusal> {
        self.structure
            .validate(bytes)
            .map_err(StructuredContractValidationRefusal::Structure)?;
        let node = bytes
            .strip_prefix(self.structure.prefix.as_slice())
            .ok_or(StructuredContractValidationRefusal::InvalidContractPath)?;
        for (path, contract) in &self.contracts {
            let mut cursor = Cursor::new(node);
            let mut budget = MAXIMUM_STRUCTURED_INFO_NODES;
            validate_contract_node(
                &self.structure.value_type,
                &mut cursor,
                path,
                contract,
                &mut budget,
            )?;
        }
        Ok(())
    }
}

fn component(path: &str) -> (&str, &str) {
    let end = path.find(['.', '|', '[', '?']).unwrap_or(path.len());
    path.split_at(end)
}

fn contract_path(ty: &StructuredInfoType, path: &str, contract: &CheckedValueContract) -> bool {
    match ty.shape() {
        Shape::Nominal { representation, .. } => contract_path(representation, path, contract),
        Shape::Leaf(kind) => path.is_empty() && kind == &contract.value_kind,
        Shape::Collection { element, .. } | Shape::Sequence { element, .. } => path
            .strip_prefix("[]")
            .is_some_and(|rest| contract_path(element, rest, contract)),
        Shape::Record { fields, .. } => path.strip_prefix('.').is_some_and(|rest| {
            let (name, rest) = component(rest);
            fields
                .iter()
                .find(|f| f.name() == name)
                .is_some_and(|f| contract_path(f.value_type(), rest, contract))
        }),
        Shape::Variant { cases, .. } => path
            .strip_prefix('|')
            .or_else(|| path.strip_prefix('?'))
            .is_some_and(|rest| {
                let (tag, rest) = component(rest);
                cases
                    .iter()
                    .find(|c| c.tag() == tag)
                    .is_some_and(|c| contract_path(c.payload_type(), rest, contract))
            }),
    }
}

fn validate_contract_node(
    ty: &StructuredInfoType,
    cursor: &mut Cursor<'_>,
    path: &str,
    contract: &CheckedValueContract,
    budget: &mut usize,
) -> Result<(), StructuredContractValidationRefusal> {
    use StructuredContractValidationRefusal::{InvalidContractPath as Invalid, Structure};
    match ty.shape() {
        Shape::Nominal { representation, .. } => {
            validate_contract_node(representation, cursor, path, contract, budget)?
        }
        Shape::Leaf(_) => {
            expect(cursor.byte().map_err(Structure)? == 0).map_err(Structure)?;
            contract
                .validate(cursor.bytes().map_err(Structure)?)
                .map_err(StructuredContractValidationRefusal::Constraint)?;
        }
        Shape::Collection { element, .. } | Shape::Sequence { element, .. } => {
            let rest = path.strip_prefix("[]").ok_or(Invalid)?;
            expect(cursor.byte().map_err(Structure)? == 1).map_err(Structure)?;
            let length = cursor.length().map_err(Structure)?;
            for _ in 0..length {
                validate_contract_node(element, cursor, rest, contract, budget)?;
            }
        }
        Shape::Record { fields, .. } => {
            let (name, rest) = component(path.strip_prefix('.').ok_or(Invalid)?);
            expect(cursor.byte().map_err(Structure)? == 2).map_err(Structure)?;
            let _ = cursor.length().map_err(Structure)?;
            for field in fields {
                let _ = cursor.bytes().map_err(Structure)?;
                if field.name() == name {
                    validate_contract_node(field.value_type(), cursor, rest, contract, budget)?;
                } else {
                    validate_node(field.value_type(), cursor, budget).map_err(Structure)?;
                }
            }
        }
        Shape::Variant { cases, .. } => {
            let (wanted, rest) = component(
                path.strip_prefix('|')
                    .or_else(|| path.strip_prefix('?'))
                    .ok_or(Invalid)?,
            );
            expect(cursor.byte().map_err(Structure)? == 3).map_err(Structure)?;
            let tag = cursor.bytes().map_err(Structure)?;
            let case = cases
                .iter()
                .find(|case| case.tag().as_bytes() == tag)
                .ok_or(Invalid)?;
            if tag == wanted.as_bytes() {
                validate_contract_node(case.payload_type(), cursor, rest, contract, budget)?;
            } else {
                validate_node(case.payload_type(), cursor, budget).map_err(Structure)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoValue, ValueConstraint,
    };
    #[test]
    fn prepared_paths_require_exact_leaf_kinds_and_valid_contract_definitions() {
        let leaf = StructuredInfoType::leaf(kind_id(crate::F32_INFO_ID)).unwrap();
        let ty = StructuredInfoType::record(
            kind_id("checked-record"),
            alloc::vec![StructuredFieldType::new(
                "samples",
                StructuredInfoType::collection(leaf.clone(), Some(2)).unwrap()
            )
            .unwrap()],
        )
        .unwrap();
        let contract = CheckedValueContract::new(
            kind_id(crate::F32_INFO_ID),
            4,
            alloc::vec![ValueConstraint::FloatFinite],
        )
        .unwrap();
        assert!(PreparedStructuredContractValidator::new(
            &ty,
            4096,
            &[(".missing[]".into(), contract.clone())]
        )
        .is_err());
        assert!(PreparedStructuredContractValidator::new(
            &ty,
            4096,
            &[(".samples".into(), contract.clone())]
        )
        .is_err());
        let wrong = CheckedValueContract::new(kind_id("value/u16"), 2, alloc::vec![]).unwrap();
        assert!(PreparedStructuredContractValidator::new(
            &ty,
            4096,
            &[(".samples[]".into(), wrong)]
        )
        .is_err());
        let mut invalid = contract.clone();
        invalid.constraints.push(ValueConstraint::FloatFinite);
        assert!(PreparedStructuredContractValidator::new(
            &ty,
            4096,
            &[(".samples[]".into(), invalid)]
        )
        .is_err());
        let checker =
            PreparedStructuredContractValidator::new(&ty, 4096, &[(".samples[]".into(), contract)])
                .unwrap();
        let collection_type = match ty.shape() {
            Shape::Record { fields, .. } => fields[0].value_type().clone(),
            _ => panic!("record"),
        };
        let values = StructuredInfoValue::collection(
            collection_type,
            alloc::vec![
                StructuredInfoValue::leaf(leaf.clone(), 0.25f32.to_le_bytes().to_vec()).unwrap(),
                StructuredInfoValue::leaf(leaf, (-0.0f32).to_le_bytes().to_vec()).unwrap()
            ],
        )
        .unwrap();
        let value = StructuredInfoValue::record(
            ty.clone(),
            alloc::vec![StructuredFieldValue::new("samples", values).unwrap()],
        )
        .unwrap();
        let good = value.canonical_bytes().unwrap();
        checker.validate(&good).unwrap();
        let mut malformed = good.clone();
        malformed.push(0);
        assert!(checker.validate(&malformed).is_err());
        let mut invalid = good.clone();
        let index = invalid
            .windows(4)
            .position(|b| b == 0.25f32.to_le_bytes())
            .unwrap();
        invalid[index..index + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(checker.validate(&invalid).is_err());
        let structure = PreparedStructuredValueValidator::new(&ty, 4096).unwrap();
        let mut leaves = 0;
        structure
            .visit_nodes::<()>(&good, |node, body| {
                if matches!(node.shape(), Shape::Leaf(_)) {
                    assert_eq!(body[0], 0);
                    leaves += 1;
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(leaves, 2);
        let mut called = false;
        assert!(structure
            .visit_nodes::<()>(&malformed, |_, _| {
                called = true;
                Ok(())
            })
            .is_err());
        assert!(!called);
        assert!(matches!(
            structure.visit_nodes(&good, |_, _| Err(7u8)),
            Err(StructuredNodeVisitRefusal::Visitor(7))
        ));
    }
}
