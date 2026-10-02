use crate::checked_syntax::{CheckedPoolDeclaration, SyntaxCheckDiagnostic, SyntaxCheckError};
use crate::prelude::*;
use crate::syntax::{BackStatement, PlotSyntax, PoolDeclaration};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::CheckedFront;

pub(super) fn check_pool_declarations(
    plot: &PlotSyntax,
    front_names: &BTreeSet<String>,
    plot_fronts: &BTreeMap<String, CheckedFront>,
) -> Result<BTreeSet<String>, SyntaxCheckDiagnostic> {
    let mut pool_names = BTreeSet::new();
    for statement in &plot.back {
        let BackStatement::Pool(pool) = statement else {
            continue;
        };
        if front_names.contains(&pool.name.text) || !pool_names.insert(pool.name.text.clone()) {
            return Err(
                SyntaxCheckError::DuplicateGear(pool.name.text.clone()).diagnostic(pool.span)
            );
        }
        if !plot_fronts.contains_key(&pool.member_plot.text) {
            return Err(
                SyntaxCheckError::UnsupportedKind(pool.member_plot.text.clone())
                    .diagnostic(pool.member_plot.span),
            );
        }
    }
    Ok(pool_names)
}

pub(super) fn checked_pool(
    pool: &PoolDeclaration,
    plot_fronts: &BTreeMap<String, CheckedFront>,
) -> CheckedPoolDeclaration {
    CheckedPoolDeclaration {
        name: pool.name.text.clone(),
        member_plot: pool.member_plot.text.clone(),
        member_front: plot_fronts[&pool.member_plot.text].clone(),
        maximum_members: pool.maximum_members,
    }
}
