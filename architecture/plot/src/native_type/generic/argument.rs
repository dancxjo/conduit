//! Canonical checked Type meaning for mixed value-family arguments.
use super::{binding::type_span, error, parameter, Context};
use crate::prelude::*;
use crate::{SyntaxCheckDiagnostic, TypeExpressionSyntax};
use sha2::{Digest, Sha256};

impl Context<'_> {
    pub(super) fn type_argument_identity(
        &self,
        value: &TypeExpressionSyntax,
    ) -> Result<Vec<u8>, SyntaxCheckDiagnostic> {
        let declarations = self
            .declarations
            .iter()
            .chain(self.generated.values())
            .cloned()
            .collect::<Vec<_>>();
        let catalog = parameter::prepare_expression(value, &declarations, self.catalog)?;
        let compiled = super::super::compile_expression(value, &catalog)?;
        let mut meaning = b"conduit.native-type-argument@1\0".to_vec();
        let representation = compiled.value_type.canonical_bytes().map_err(|refusal| {
            error(
                type_span(value),
                alloc::format!("invalid native Type argument: {refusal:?}"),
            )
        })?;
        append(&mut meaning, &representation);
        for contract in compiled.contracts {
            append(&mut meaning, contract.representation_path.as_bytes());
            append(&mut meaning, &contract.contract.identity_bytes());
        }
        Ok(Sha256::digest(&meaning).to_vec())
    }
}
fn append(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u64).to_le_bytes());
    target.extend_from_slice(value);
}
