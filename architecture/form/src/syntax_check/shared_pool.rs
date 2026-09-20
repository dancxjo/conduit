use crate::checked_syntax::{CheckedPoolDeclaration, SyntaxCheckDiagnostic, SyntaxCheckError};
use crate::prelude::*;
use crate::syntax::{BackStatement, FormSyntax, PoolDeclaration};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::CheckedFront;

pub(super) fn check_pool_declarations(
    form: &FormSyntax,
    front_names: &BTreeSet<String>,
    form_fronts: &BTreeMap<String, CheckedFront>,
) -> Result<BTreeSet<String>, SyntaxCheckDiagnostic> {
    let mut pool_names = BTreeSet::new();
    for statement in &form.back {
        let BackStatement::Pool(pool) = statement else {
            continue;
        };
        if front_names.contains(&pool.name.text) || !pool_names.insert(pool.name.text.clone()) {
            return Err(
                SyntaxCheckError::DuplicateGear(pool.name.text.clone()).diagnostic(pool.span)
            );
        }
        if !form_fronts.contains_key(&pool.member_form.text) {
            return Err(
                SyntaxCheckError::UnsupportedKind(pool.member_form.text.clone())
                    .diagnostic(pool.member_form.span),
            );
        }
    }
    Ok(pool_names)
}

pub(super) fn checked_pool(
    pool: &PoolDeclaration,
    form_fronts: &BTreeMap<String, CheckedFront>,
) -> CheckedPoolDeclaration {
    CheckedPoolDeclaration {
        name: pool.name.text.clone(),
        member_form: pool.member_form.text.clone(),
        member_front: form_fronts[&pool.member_form.text].clone(),
        maximum_members: pool.maximum_members,
    }
}
