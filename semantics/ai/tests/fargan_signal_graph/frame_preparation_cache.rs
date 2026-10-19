//! Actual full Source preparation reuse; Plan selection always runs again.
use super::*;
#[test]
fn full_frame_check_and_expansion_reuse_match_cold_plans_and_execution() {
    crate::on_large_stack(|| {
        let schema = schema();
        let resources = session_tests::zero_resources(&schema);
        let state = State {
            conv: [0.1; 164],
            gru1: [0.2; 160],
            gru2: [-0.4; 128],
            gru3: [0.6; 128],
            pitch: [0.1; 256],
            deemphasis: 0.,
        };
        let input = inputs(&schema, &[0.01; 20], &[0.2; 128], 81, 80, &state);
        let mut cache = conduit_plot::SourcePreparationCache::new(
            conduit_plot::PreparationCacheLimits::default(),
        )
        .unwrap();
        let original = source();
        for profile in [
            NumericProfile::Unfused,
            NumericProfile::CompactMatrixDag,
            NumericProfile::CompactMatrixDagLocalExpressions,
        ] {
            let cold = prepare_entry_plan_profile(&schema, &original, ENTRY, profile);
            for _ in 0..2 {
                let cached = prepare_entry_plan_profile_cached(
                    &schema, &original, ENTRY, profile, &mut cache,
                );
                assert_eq!(
                    cold, cached,
                    "exact typed Plan and provenance for {profile:?}"
                );
            }
            let cached =
                prepare_entry_plan_profile_cached(&schema, &original, ENTRY, profile, &mut cache);
            let expected =
                run_entry_plan(cold, &resources, input.clone(), ExecutionMode::Normal).unwrap();
            let actual =
                run_entry_plan(cached, &resources, input.clone(), ExecutionMode::Normal).unwrap();
            assert_eq!(expected.outputs, actual.outputs);
            assert_eq!(expected.signs, actual.signs);
        }
        assert_eq!(cache.stats().checks, 1);
        assert_eq!(cache.stats().expansions, 1);
        assert_eq!(cache.stats().check_hits, 8);
        assert_eq!(cache.stats().expansion_hits, 8);
        let edited = format!("# shifted source spans\n{original}");
        let cold = prepare_entry_plan_profile(
            &schema,
            &edited,
            ENTRY,
            NumericProfile::CompactMatrixDagLocalExpressions,
        );
        let cached = prepare_entry_plan_profile_cached(
            &schema,
            &edited,
            ENTRY,
            NumericProfile::CompactMatrixDagLocalExpressions,
            &mut cache,
        );
        assert_eq!(cold, cached);
        assert_eq!(cache.stats().checks, 2);
        assert_eq!(cache.stats().expansions, 2);
        eprintln!("full768-node1211-Cord preparation:9 current Plans share1 check and1 expansion, then edited Source cold equality; exact outputs and complete Signs for3 physical profiles; cache stats {:?}",cache.stats());
    });
}
