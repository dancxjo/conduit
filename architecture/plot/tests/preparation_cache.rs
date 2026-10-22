#![cfg(target_has_atomic = "ptr")]
use conduit_core::*;
use conduit_plot::*;
use std::sync::Arc;
const SOURCE: &str = "plot main {\n a: test/unit\n}\nplot other {\n b: test/unit\n}\n";
fn kind(revision: &str) -> Kind {
    Kind {
        kind_id: kind_id("test/unit"),
        kind_contract_revision: revision.into(),
        startup_parameters: vec![],
        shorthand: None,
        configuration: vec![],
        inputs: vec![],
        outputs: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: 64,
        },
    }
}
fn catalog(revision: &str) -> (Arc<StartupCatalog>, Arc<ProfileCatalog>) {
    let mut profile = ProfileCatalog::new();
    profile.insert_kind(kind(revision)).unwrap();
    (
        Arc::new(profile.startup_catalog().unwrap()),
        Arc::new(profile),
    )
}
fn cache() -> SourcePreparationCache {
    SourcePreparationCache::new(PreparationCacheLimits::default()).unwrap()
}
fn cold(
    source: &str,
    startup: &StartupCatalog,
    entry: &str,
    profile: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
) -> ExpandedAuthoringPlot {
    let checked = check_syntax_document(&parse_syntax_document(source), startup).unwrap();
    expand_canonical_plot_for_authoring_with_backs(&checked, entry, profile, backs).unwrap()
}
#[test]
fn repeated_exact_inputs_share_checked_and_expanded_owners_with_cold_equality() {
    let (startup, profile) = catalog("unit@1");
    let backs = Arc::new(CanonicalBackCatalog::new());
    let mut c = cache();
    let a = c.check(SOURCE, &startup).unwrap();
    let b = c.check(SOURCE, &Arc::new((*startup).clone())).unwrap();
    assert!(Arc::ptr_eq(a.checked(), b.checked()));
    assert_eq!(a.source(), SOURCE);
    let x = c.expand(&a, "main", &profile, &backs).unwrap();
    let y = c
        .expand(&b, "main", &Arc::new((*profile).clone()), &backs)
        .unwrap();
    assert!(Arc::ptr_eq(&x, &y));
    assert_eq!(*x, cold(SOURCE, &startup, "main", &profile, &backs));
    assert_eq!(c.stats().checks, 1);
    assert_eq!(c.stats().check_hits, 1);
    assert_eq!(c.stats().expansions, 1);
    assert_eq!(c.stats().expansion_hits, 1);
}
#[test]
fn source_bytes_and_observed_startup_context_are_invalidation_dependencies() {
    let (startup, profile) = catalog("unit@1");
    let mut c = cache();
    let a = c.check(SOURCE, &startup).unwrap();
    let changed = format!("# shifted spans\n{SOURCE}");
    let b = c.check(&changed, &startup).unwrap();
    assert_ne!(a.source_digest(), b.source_digest());
    assert!(!Arc::ptr_eq(a.checked(), b.checked()));
    let mut extra = (*startup).clone();
    extra
        .insert(KindSignature {
            kind: "unrelated/signature".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let d = c.check(SOURCE, &Arc::new(extra)).unwrap();
    assert!(Arc::ptr_eq(a.checked(), d.checked()));
    assert!(
        c.check(SOURCE, &Arc::new(StartupCatalog::new())).is_err(),
        "removed checking dependency must not reuse a successful check"
    );
    assert_eq!(c.stats().checks, 3);
    let backs = Arc::new(CanonicalBackCatalog::new());
    let expanded = c.expand(&b, "main", &profile, &backs).unwrap();
    assert_eq!(
        *expanded,
        cold(&changed, &startup, "main", &profile, &backs)
    );
}
#[test]
fn profile_revision_laws_and_limits_invalidate_expansion_independently_of_check() {
    let (startup, profile) = catalog("unit@1");
    let backs = Arc::new(CanonicalBackCatalog::new());
    let mut c = cache();
    let source = c.check(SOURCE, &startup).unwrap();
    let original = c.expand(&source, "main", &profile, &backs).unwrap();
    for mutation in 0..3 {
        let mut k = kind("unit@1");
        match mutation {
            0 => k.kind_contract_revision = "unit@2".into(),
            1 => k
                .semantic_laws
                .push(KindSemanticLaw::Replay(ReplayBehavior::Exact)),
            _ => k.limits.max_active_instances = 8,
        };
        let mut p = ProfileCatalog::new();
        p.insert_kind(k).unwrap();
        let p = Arc::new(p);
        let result = c.expand(&source, "main", &p, &backs).unwrap();
        assert!(!Arc::ptr_eq(&original, &result));
        assert_eq!(*result, cold(SOURCE, &startup, "main", &p, &backs));
    }
    assert_eq!(c.stats().checks, 1);
    assert_eq!(c.stats().expansions, 4);
}
#[test]
fn entry_identity_fifo_eviction_and_retained_handles_remain_exact() {
    let (startup, profile) = catalog("unit@1");
    let backs = Arc::new(CanonicalBackCatalog::new());
    let mut c = SourcePreparationCache::new(PreparationCacheLimits {
        sources: 1,
        expansions_per_source: 1,
        source_bytes: 4096,
    })
    .unwrap();
    let old = c.check(SOURCE, &startup).unwrap();
    let main = c.expand(&old, "main", &profile, &backs).unwrap();
    let other = c.expand(&old, "other", &profile, &backs).unwrap();
    assert_ne!(
        main.expanded.checked_plot_id,
        other.expanded.checked_plot_id
    );
    assert_eq!(c.retained_expansions(), 1);
    let again = c.expand(&old, "main", &profile, &backs).unwrap();
    assert!(!Arc::ptr_eq(&main, &again));
    assert_eq!(*main, *again);
    let next = c.check(&format!("# new\n{SOURCE}"), &startup).unwrap();
    assert!(!Arc::ptr_eq(old.checked(), next.checked()));
    assert_eq!(c.retained_sources(), 1);
    let old_again = c.expand(&old, "main", &profile, &backs).unwrap();
    assert_eq!(*old_again, *main);
    assert_eq!(c.retained_expansions(), 0);
    c.clear();
    assert_eq!(c.retained_sources(), 0);
    assert_eq!(main.expanded.gears.len(), 1);
    assert_eq!(c.stats().source_evictions, 1);
    assert_eq!(c.stats().expansion_evictions, 2);
}
#[test]
fn errors_are_not_cached_and_do_not_displace_successful_entries() {
    let (startup, profile) = catalog("unit@1");
    let backs = Arc::new(CanonicalBackCatalog::new());
    let mut c = cache();
    let source = c.check(SOURCE, &startup).unwrap();
    for _ in 0..2 {
        assert!(c
            .check("plot broken {\n a: unknown/kind\n}\n", &startup)
            .is_err());
        assert!(c.expand(&source, "missing", &profile, &backs).is_err());
    }
    assert_eq!(c.retained_sources(), 1);
    assert_eq!(c.retained_expansions(), 0);
    assert_eq!(c.stats().checks, 3);
    assert_eq!(c.stats().expansions, 2);
    let again = c.check(SOURCE, &startup).unwrap();
    assert!(Arc::ptr_eq(source.checked(), again.checked()));
}
#[test]
fn caller_mutation_uses_copy_on_write_and_cannot_poison_cached_truth() {
    let (startup, profile) = catalog("unit@1");
    let backs = Arc::new(CanonicalBackCatalog::new());
    let mut c = cache();
    let token = c.check(SOURCE, &startup).unwrap();
    let mut checked = token.checked().clone();
    Arc::make_mut(&mut checked).plots[0].name = "forged".into();
    let mut expanded = c.expand(&token, "main", &profile, &backs).unwrap();
    Arc::make_mut(&mut expanded).expanded.gears.clear();
    let result = c.expand(&token, "main", &profile, &backs).unwrap();
    assert_eq!(result.expanded.gears.len(), 1);
    assert_eq!(token.checked().plots[0].name, "main");
    assert_eq!(*result, cold(SOURCE, &startup, "main", &profile, &backs));
}
#[test]
fn zero_and_oversized_retention_limits_use_the_unmodified_cold_path() {
    let (startup, profile) = catalog("unit@1");
    let backs = Arc::new(CanonicalBackCatalog::new());
    for limits in [
        PreparationCacheLimits {
            sources: 0,
            expansions_per_source: 0,
            source_bytes: 4096,
        },
        PreparationCacheLimits {
            sources: 1,
            expansions_per_source: 1,
            source_bytes: 1,
        },
    ] {
        let mut c = SourcePreparationCache::new(limits).unwrap();
        let a = c.check(SOURCE, &startup).unwrap();
        let b = c.check(SOURCE, &startup).unwrap();
        assert!(!Arc::ptr_eq(a.checked(), b.checked()));
        assert_eq!(
            *c.expand(&a, "main", &profile, &backs).unwrap(),
            cold(SOURCE, &startup, "main", &profile, &backs)
        );
        assert_eq!(c.retained_sources(), 0);
        assert_eq!(c.stats().unretained_checks, 2);
    }
    assert!(SourcePreparationCache::new(PreparationCacheLimits {
        sources: 17,
        ..PreparationCacheLimits::default()
    })
    .is_err());
    assert!(SourcePreparationCache::new(PreparationCacheLimits {
        expansions_per_source: 17,
        ..PreparationCacheLimits::default()
    })
    .is_err());
    assert!(SourcePreparationCache::new(PreparationCacheLimits {
        source_bytes: 4 * 1024 * 1024 + 1,
        ..PreparationCacheLimits::default()
    })
    .is_err());
}

#[test]
fn changed_back_owner_is_not_hidden_by_a_cached_primitive_expansion() {
    let (startup, profile) = catalog("unit@1");
    let empty = Arc::new(CanonicalBackCatalog::new());
    let mut c = cache();
    let token = c.check(SOURCE, &startup).unwrap();
    let original = c.expand(&token, "main", &profile, &empty).unwrap();
    let mut backs = CanonicalBackCatalog::new();
    backs
        .insert(&kind("unit@1"), token.checked(), "main")
        .unwrap();
    let backs = Arc::new(backs);
    let cold =
        expand_canonical_plot_for_authoring_with_backs(token.checked(), "main", &profile, &backs);
    assert!(
        cold.is_err(),
        "recursive Back context is not an admitted primitive"
    );
    for _ in 0..2 {
        assert_eq!(
            c.expand(&token, "main", &profile, &backs).unwrap_err(),
            cold.clone().unwrap_err()
        );
    }
    assert!(Arc::ptr_eq(
        &original,
        &c.expand(&token, "main", &profile, &empty).unwrap()
    ));
    assert_eq!(c.stats().expansions, 3);
    assert_eq!(c.stats().expansion_hits, 1);
}

#[path = "preparation_cache/dependencies.rs"]
mod dependencies;

#[path = "preparation_cache/observations.rs"]
mod observations;
