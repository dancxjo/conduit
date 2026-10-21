//! Observations retain actual cold/hit/refusal distinctions and exact meaning.
use super::*;
use conduit_plot::SourcePreparationEvent as Event;

#[test]
fn observed_cold_and_hit_paths_retain_exact_owners_and_cold_meaning() {
    let (startup, profile) = catalog("unit@1");
    let backs = Arc::new(CanonicalBackCatalog::new());
    let mut c = cache();
    let mut events = Vec::new();
    let a = c
        .check_observed(SOURCE, &startup, |event| events.push(event))
        .unwrap();
    assert_eq!(
        events,
        [
            Event::ParseStarted,
            Event::ParseFinished,
            Event::CheckStarted,
            Event::CheckFinished { accepted: true }
        ]
    );
    events.clear();
    let b = c
        .check_observed(SOURCE, &startup, |event| events.push(event))
        .unwrap();
    assert_eq!(events, [Event::CheckHit]);
    assert!(Arc::ptr_eq(a.checked(), b.checked()));
    events.clear();
    let x = c
        .expand_observed(&a, "main", &profile, &backs, |event| events.push(event))
        .unwrap();
    assert_eq!(
        events,
        [
            Event::ExpansionStarted,
            Event::ExpansionFinished { accepted: true }
        ]
    );
    assert_eq!(*x, cold(SOURCE, &startup, "main", &profile, &backs));
    events.clear();
    let y = c
        .expand_observed(&b, "main", &profile, &backs, |event| events.push(event))
        .unwrap();
    assert_eq!(events, [Event::ExpansionHit]);
    assert!(Arc::ptr_eq(&x, &y));
    assert_eq!(
        (
            c.stats().checks,
            c.stats().check_hits,
            c.stats().expansions,
            c.stats().expansion_hits
        ),
        (1, 1, 1, 1)
    );
}
#[test]
fn observed_failures_finish_as_refused_and_are_not_cached_or_promoted_to_hits() {
    let (startup, profile) = catalog("unit@1");
    let backs = Arc::new(CanonicalBackCatalog::new());
    let mut c = cache();
    for _ in 0..2 {
        let mut events = Vec::new();
        assert!(c
            .check_observed("plot bad { nope: missing() }", &startup, |event| events
                .push(event))
            .is_err());
        assert_eq!(
            events,
            [
                Event::ParseStarted,
                Event::ParseFinished,
                Event::CheckStarted,
                Event::CheckFinished { accepted: false }
            ]
        );
    }
    assert_eq!(c.retained_sources(), 0);
    assert_eq!(c.stats().checks, 2);
    assert_eq!(c.stats().check_hits, 0);
    let source = c.check(SOURCE, &startup).unwrap();
    for _ in 0..2 {
        let mut events = Vec::new();
        assert!(c
            .expand_observed(&source, "absent", &profile, &backs, |event| events
                .push(event))
            .is_err());
        assert_eq!(
            events,
            [
                Event::ExpansionStarted,
                Event::ExpansionFinished { accepted: false }
            ]
        );
    }
    assert_eq!(c.retained_expansions(), 0);
    assert_eq!(c.stats().expansions, 2);
    assert_eq!(c.stats().expansion_hits, 0);
}
#[test]
fn removing_a_checking_dependency_rechecks_and_reports_the_real_refusal() {
    let (startup, _) = catalog("unit@1");
    let mut c = cache();
    c.check(SOURCE, &startup).unwrap();
    let mut events = Vec::new();
    assert!(c
        .check_observed(SOURCE, &Arc::new(StartupCatalog::new()), |event| events
            .push(event))
        .is_err());
    assert_eq!(
        events,
        [
            Event::ParseStarted,
            Event::ParseFinished,
            Event::CheckStarted,
            Event::CheckFinished { accepted: false }
        ]
    );
    assert_eq!(c.stats().checks, 2);
    assert_eq!(c.stats().check_hits, 0);
}
#[test]
fn checking_can_hit_when_only_the_expansion_dependency_changes() {
    let (startup, profile) = catalog("unit@1");
    let (new_startup, new_profile) = catalog("unit@2");
    let backs = Arc::new(CanonicalBackCatalog::new());
    let mut c = cache();
    let original = c.check(SOURCE, &startup).unwrap();
    c.expand(&original, "main", &profile, &backs).unwrap();
    let mut events = Vec::new();
    let checked = c
        .check_observed(SOURCE, &new_startup, |event| events.push(event))
        .unwrap();
    assert_eq!(events, [Event::CheckHit]);
    assert!(Arc::ptr_eq(original.checked(), checked.checked()));
    events.clear();
    let expanded = c
        .expand_observed(&checked, "main", &new_profile, &backs, |event| {
            events.push(event)
        })
        .unwrap();
    assert_eq!(
        events,
        [
            Event::ExpansionStarted,
            Event::ExpansionFinished { accepted: true }
        ]
    );
    assert_eq!(
        *expanded,
        cold(SOURCE, &new_startup, "main", &new_profile, &backs)
    );
    assert_eq!(
        (
            c.stats().checks,
            c.stats().check_hits,
            c.stats().expansions,
            c.stats().expansion_hits
        ),
        (1, 1, 2, 0)
    );
}
#[test]
fn observations_do_not_turn_zero_retention_into_a_cache() {
    let (startup, profile) = catalog("unit@1");
    let backs = Arc::new(CanonicalBackCatalog::new());
    let mut c = SourcePreparationCache::new(PreparationCacheLimits {
        sources: 0,
        expansions_per_source: 0,
        source_bytes: 0,
    })
    .unwrap();
    for _ in 0..2 {
        let mut events = Vec::new();
        let source = c
            .check_observed(SOURCE, &startup, |event| events.push(event))
            .unwrap();
        let value = c
            .expand_observed(&source, "main", &profile, &backs, |event| {
                events.push(event)
            })
            .unwrap();
        assert_eq!(*value, cold(SOURCE, &startup, "main", &profile, &backs));
        assert!(!events.contains(&Event::CheckHit) && !events.contains(&Event::ExpansionHit));
    }
    assert_eq!((c.retained_sources(), c.retained_expansions()), (0, 0));
    assert_eq!(c.stats().unretained_checks, 2);
}
