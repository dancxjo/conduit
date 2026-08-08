use crate::checked_syntax::{CheckedPoolDeclaration, SyntaxCheckDiagnostic, SyntaxCheckError};
use crate::syntax::{BackStatement, FormSyntax, PoolDeclaration};
use conduit_core::CheckedFace;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn check_pool_declarations(
    form: &FormSyntax,
    face_names: &BTreeSet<String>,
    form_faces: &BTreeMap<String, CheckedFace>,
) -> Result<BTreeSet<String>, SyntaxCheckDiagnostic> {
    let mut pool_names = BTreeSet::new();
    for statement in &form.back {
        let BackStatement::Pool(pool) = statement else {
            continue;
        };
        if face_names.contains(&pool.name.text) || !pool_names.insert(pool.name.text.clone()) {
            return Err(
                SyntaxCheckError::DuplicateCell(pool.name.text.clone()).diagnostic(pool.span)
            );
        }
        if !form_faces.contains_key(&pool.member_form.text) {
            return Err(
                SyntaxCheckError::UnsupportedOperation(pool.member_form.text.clone())
                    .diagnostic(pool.member_form.span),
            );
        }
    }
    Ok(pool_names)
}

pub(super) fn checked_pool(
    pool: &PoolDeclaration,
    form_faces: &BTreeMap<String, CheckedFace>,
) -> CheckedPoolDeclaration {
    CheckedPoolDeclaration {
        name: pool.name.text.clone(),
        member_form: pool.member_form.text.clone(),
        member_face: form_faces[&pool.member_form.text].clone(),
        maximum_members: pool.maximum_members,
    }
}
