#![cfg(feature = "parser-model")]
//! Model-free contract diagnosis: synthetic legal trees and checked Source
//! rank/merge, agreement and contiguous frontier. No streaming accuracy claim.
#[path = "common/parser_fixture.rs"]
mod parser_fixture;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/parser_selection_io.rs"]
mod parser_selection_io;
#[path = "common/parser_structural_consensus.rs"]
mod structural;
use conduit_language::*;
use structural::Structural;

#[test]
fn larger_winning_score_does_not_retire_a_legal_cardinality_survivor() {
    let mut source = Structural::new();
    let trees = ["dep", "obj", "obl", "nmod"].map(|relation| source.tree(3, 1, relation));
    for (index, tree) in trees.iter().enumerate() {
        assert!(
            !trees[..index].contains(tree),
            "unique canonical complete graphs"
        );
    }
    for scores in [[30, 20, 10, 0], [900_000, 20, 10, 0]] {
        let candidates = core::array::from_fn(|index| {
            Structural::hypothesis(&trees[index], index as u64, scores[index], true)
        });
        for order in [[0, 1, 2, 3], [3, 1, 0, 2]] {
            let ranked = source.rank(&candidates, order);
            assert_eq!(ranked.candidate0(), &candidates[0]);
            assert_eq!(ranked.candidate3(), &candidates[3]);
            let beam = Structural::admitted(&ranked);
            let agreed = source.agreement(&beam, 0);
            assert!(*agreed.agrees());
            assert_eq!(*agreed.survivors(), 4);
            assert!(!*source.agreement(&beam, 1).agrees());
            assert!(
                *source.agreement(&beam, 2).agrees(),
                "later agreement cannot bridge an earlier gap"
            );
            let frontier = source.frontier(&beam, 3);
            assert_eq!(*frontier.stable(), 1);
            let refused = source.commit(&frontier, 3);
            assert!(!*refused.accepted());
            assert_eq!(refused.frontier(), &frontier);
        }
    }
}

#[test]
fn distinct_continuations_can_agree_on_an_earlier_contiguous_prefix() {
    let mut source = Structural::new();
    let trees = ["dep", "obj", "obl", "nmod"].map(|relation| source.tree(4, 3, relation));
    let candidates = core::array::from_fn(|index| {
        Structural::hypothesis(&trees[index], index as u64, 40 - index as i64, true)
    });
    let ranked = source.rank(&candidates, [2, 0, 3, 1]);
    let beam = Structural::admitted(&ranked);
    assert!(!*source.agreement(&beam, 3).agrees());
    let frontier = source.frontier(&beam, 4);
    assert_eq!(*frontier.stable(), 3);
    let result = source.commit(&frontier, 3);
    assert!(*result.accepted());
    assert_eq!(*result.frontier().committed(), 3);
    assert_eq!(*frontier.committed(), 0, "the input frontier is immutable");
}

#[test]
fn duplicate_derivations_are_not_independent_graphs_and_foreign_basis_refuses() {
    let mut source = Structural::new();
    let tree = source.tree(3, 1, "dep");
    let duplicates = core::array::from_fn(|index| {
        Structural::hypothesis(&tree, index as u64, 10 - index as i64, true)
    });
    let ranked = source.rank(&duplicates, [3, 2, 1, 0]);
    let beam = Structural::admitted(&ranked);
    let agreement = source.agreement(&beam, 1);
    assert!(*agreement.agrees());
    assert_eq!(*agreement.survivors(), 4);
    assert_eq!(
        ranked.candidate0().parser().state(),
        ranked.candidate3().parser().state()
    );
    assert_ne!(
        ranked.candidate0().parser().identity(),
        ranked.candidate3().parser().identity()
    );
    // Current merge preserves multiplicity. Four slots are not four independent
    // graph alternatives and this test produces no probability/confidence value.
    let foreign = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("foreign-analysis".into()).unwrap(),
        tree.basis().source_revision().clone(),
        tree.basis().text().clone(),
    )
    .unwrap();
    assert!(LanguageParserAdmittedBeam::new(
        foreign,
        beam.candidate0().clone(),
        beam.candidate1().clone(),
        beam.candidate2().clone(),
        beam.candidate3().clone()
    )
    .is_err());
    let inactive = Structural::hypothesis(&tree, 99, 0, false);
    let empty = LanguageParserAdmittedBeam::new(
        tree.basis().clone(),
        inactive.parser().clone(),
        inactive.parser().clone(),
        inactive.parser().clone(),
        inactive.parser().clone(),
    )
    .unwrap();
    let result = source.agreement(&empty, 0);
    assert!(!*result.agrees());
    assert_eq!(*result.survivors(), 0);
}
