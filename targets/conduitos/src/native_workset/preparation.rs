//! Reuse checked embedded Source; never cache offers, admission or execution.
use super::{NativePlot, WorksetRefusal};
use alloc::sync::Arc;
use conduit_plot::{
    CanonicalBackCatalog, ExpandedAuthoringPlot, ExpandedCanonicalPlot, PreparationCacheLimits,
    ProfileCatalog, SourcePreparationCache, StartupCatalog,
};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

const SOURCE_BYTES: usize = 8192;

struct NativePreparation {
    startup: Arc<StartupCatalog>,
    profile: Arc<ProfileCatalog>,
    backs: Arc<CanonicalBackCatalog>,
    cache: SourcePreparationCache,
}

impl NativePreparation {
    fn new() -> Result<Self, WorksetRefusal> {
        let (startup, profile) = catalogs()?;
        Ok(Self {
            startup: Arc::new(startup),
            profile: Arc::new(profile),
            backs: Arc::new(CanonicalBackCatalog::new()),
            cache: SourcePreparationCache::new(PreparationCacheLimits {
                sources: super::NATIVE_CANDIDATE_CAPACITY,
                expansions_per_source: 1,
                source_bytes: SOURCE_BYTES,
            })
            .map_err(|_| WorksetRefusal::Catalog)?,
        })
    }

    fn expanded(&mut self, plot: NativePlot) -> Result<Arc<ExpandedAuthoringPlot>, WorksetRefusal> {
        let source = self
            .cache
            .check(plot.source(), &self.startup)
            .map_err(|_| WorksetRefusal::Catalog)?;
        let expanded = self
            .cache
            .expand(&source, plot.name(), &self.profile, &self.backs)
            .map_err(|_| WorksetRefusal::Catalog)?;
        // Preserve the closed-root law of expand_canonical_plot. The general
        // authoring cache also supports open Fores, which this entrance does not.
        if !expanded.input_bindings.is_empty() || !expanded.output_bindings.is_empty() {
            return Err(WorksetRefusal::Catalog);
        }
        Ok(expanded)
    }
}

fn catalogs() -> Result<(StartupCatalog, ProfileCatalog), WorksetRefusal> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    conduit_semantic_catalog::install_text_state_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    conduit_semantic_catalog::install_application_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    Ok((startup, profile))
}

struct PreparationStore {
    locked: AtomicBool,
    value: UnsafeCell<Option<NativePreparation>>,
}

// Every access to value is exclusive under locked. The guard releases even
// during unwinding; borrowed cache state cannot escape. NativePreparation is
// Send (also asserted below), and contains no machine or lifecycle authority.
unsafe impl Sync for PreparationStore {}

static PREPARATION: PreparationStore = PreparationStore {
    locked: AtomicBool::new(false),
    value: UnsafeCell::new(None),
};

struct Guard<'a>(&'a AtomicBool);
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub(super) fn with_plot<T>(
    plot: NativePlot,
    visit: impl FnOnce(&ExpandedCanonicalPlot) -> T,
) -> Result<T, WorksetRefusal> {
    let _timing =
        crate::native_observation::Interval::begin(crate::native_observation::Stage::PlotBuild);
    // Preparation is outside kernel Steps. Do not spin, retry, or block input
    // if another preparation owns the cache: use the ordinary finite cold path.
    if PREPARATION
        .locked
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        let expanded = NativePreparation::new()?.expanded(plot)?;
        return Ok(visit(&expanded.expanded));
    }
    let _guard = Guard(&PREPARATION.locked);
    // SAFETY: the guard exclusively owns value, including initialization, until
    // after visit returns an owned result. No reference to value is published.
    let preparation = unsafe { &mut *PREPARATION.value.get() };
    if preparation.is_none() {
        *preparation = Some(NativePreparation::new()?);
    }
    let expanded = preparation
        .as_mut()
        .expect("initialized above")
        .expanded(plot)?;
    Ok(visit(&expanded.expanded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_native_source_reuses_exact_cold_meaning_with_finite_entries() {
        fn assert_send<T: Send>() {}
        assert_send::<NativePreparation>();
        let mut prepared = NativePreparation::new().unwrap();
        let (startup, profile) = catalogs().unwrap();
        for plot in super::super::candidate_inventory() {
            assert!(plot.source().len() <= SOURCE_BYTES);
            let checked = conduit_plot::check_syntax_document(
                &conduit_plot::parse_syntax_document(plot.source()),
                &startup,
            )
            .unwrap();
            let cold =
                conduit_plot::expand_canonical_plot(&checked, plot.name(), &profile).unwrap();
            let first = prepared.expanded(plot).unwrap();
            let second = prepared.expanded(plot).unwrap();
            assert_eq!(first.expanded, cold);
            assert!(Arc::ptr_eq(&first, &second));
        }
        let count = super::super::NATIVE_CANDIDATE_CAPACITY;
        assert_eq!(prepared.cache.retained_sources(), count);
        assert_eq!(prepared.cache.retained_expansions(), count);
        let stats = prepared.cache.stats();
        assert_eq!(stats.checks, count as u64);
        assert_eq!(stats.check_hits, count as u64);
        assert_eq!(stats.expansions, count as u64);
        assert_eq!(stats.expansion_hits, count as u64);
        assert_eq!(stats.source_evictions, 0);
        assert_eq!(stats.expansion_evictions, 0);
    }
}
