//! Import the exact checked Native dependency closure, never inspection claims.
use super::*;
use crate::CheckedSyntaxDocument;
use conduit_core::{KindId, StructuredInfoType, StructuredInfoTypeShape as S};
impl StartupCatalog {
    pub(crate) fn retain_native_dependency(
        &mut self,
        owner: CheckedNativeType,
    ) -> Result<(), String> {
        if let Some(existing) = self.retained_native_dependencies.get(&owner.identity) {
            if existing.value_type != owner.value_type
                || existing.value_contracts != owner.value_contracts
                || existing.invariants != owner.invariants
            {
                return Err("conflicting checked Native dependency".into());
            }
            return Ok(());
        }
        self.retained_native_dependencies
            .insert(owner.identity.clone(), owner);
        Ok(())
    }
    /// Transactionally import an exact root and its private checked dependency
    /// closure. Only the root receives a Source alias; subordinate owners remain
    /// private. Public inspection rows can detect conflicts, never grant owners.
    pub fn insert_checked_native_type_from_document(
        &mut self,
        name: impl Into<String>,
        document: &CheckedSyntaxDocument,
        identity: &KindId,
    ) -> Result<(), String> {
        let root = document
            .retained_native_owner(identity)
            .map_err(|why| format!("checked Native owner: {why:?}"))?;
        let mut staged = self.clone();
        let mut visited = alloc::collections::BTreeSet::new();
        collect(&mut staged, document, &root.value_type, &mut visited)?;
        staged.insert_checked_native_type(name, root)?;
        *self = staged;
        Ok(())
    }
}
fn collect(
    catalog: &mut StartupCatalog,
    document: &CheckedSyntaxDocument,
    ty: &StructuredInfoType,
    visited: &mut alloc::collections::BTreeSet<KindId>,
) -> Result<(), String> {
    let identity = match ty.shape() {
        S::Nominal { schema, .. } | S::Record { schema, .. } | S::Variant { schema, .. } => {
            Some(schema)
        }
        _ => None,
    };
    if let Some(identity) = identity {
        if !visited.insert(identity.clone()) {
            return Ok(());
        }
        match document.retained_native_owner(identity) {
            Ok(owner) => {
                if &owner.value_type != ty {
                    return Err("checked Native dependency differs from owner schema".into());
                }
                catalog.retain_native_dependency(owner.clone())?;
            }
            Err(crate::checked_syntax::NativeOwnerRefusal::Missing) if exact_optional(ty) => (),
            Err(why) => return Err(format!("checked Native dependency: {why:?}")),
        }
    }
    match ty.shape() {
        S::Leaf(_) => (),
        S::Nominal { representation, .. } => collect(catalog, document, representation, visited)?,
        S::Collection { element, .. } | S::Sequence { element, .. } => {
            collect(catalog, document, element, visited)?
        }
        S::Record { fields, .. } => {
            for field in fields {
                collect(catalog, document, field.value_type(), visited)?;
            }
        }
        S::Variant { cases, .. } => {
            for case in cases {
                collect(catalog, document, case.payload_type(), visited)?;
            }
        }
    }
    Ok(())
}
fn exact_optional(ty: &StructuredInfoType) -> bool {
    let S::Variant { cases, .. } = ty.shape() else {
        return false;
    };
    let Some(some) = cases.iter().find(|case| case.tag() == "some") else {
        return false;
    };
    conduit_core::optional_info_type(some.payload_type().clone())
        .is_ok_and(|expected| &expected == ty)
}
