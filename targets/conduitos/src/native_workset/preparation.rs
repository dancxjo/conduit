//! Reuse checked embedded Source; never cache offers, admission or execution.
use super::{NativePlot, WorksetRefusal};
use crate::native_observation::{Phase, Progress};
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
mod observation;

struct NativePreparation {
    startup: Arc<StartupCatalog>,
    profile: Arc<ProfileCatalog>,
    backs: Arc<CanonicalBackCatalog>,
    cache: SourcePreparationCache,
}

impl NativePreparation {
    fn new() -> Result<Self, WorksetRefusal> {
        Self::with_retention(!cfg!(feature = "native-cache-zero-retention"))
    }

    fn with_retention(retain: bool) -> Result<Self, WorksetRefusal> {
        let (startup, profile) =
            Progress::result(Phase::CatalogInstall, "native-inventory", catalogs)?;
        Ok(Self {
            startup: Arc::new(startup),
            profile: Arc::new(profile),
            backs: Arc::new(CanonicalBackCatalog::new()),
            cache: SourcePreparationCache::new(PreparationCacheLimits {
                sources: if retain {
                    super::NATIVE_CANDIDATE_CAPACITY
                } else {
                    0
                },
                expansions_per_source: usize::from(retain),
                source_bytes: if retain { SOURCE_BYTES } else { 0 },
            })
            .map_err(|_| WorksetRefusal::Catalog)?,
        })
    }

    fn observed(
        &mut self,
        plot: NativePlot,
        fallback: bool,
    ) -> Result<Arc<ExpandedAuthoringPlot>, WorksetRefusal> {
        let before = self.cache.stats();
        let result = self.expanded(plot);
        observation::publish(
            plot,
            before,
            self.cache.stats(),
            self.cache.retained_sources(),
            self.cache.retained_expansions(),
            fallback,
            result.is_ok(),
        );
        result
    }

    fn expanded(&mut self, plot: NativePlot) -> Result<Arc<ExpandedAuthoringPlot>, WorksetRefusal> {
        let mut observer = preparation_observer(plot.name());
        let source = self
            .cache
            .check_observed(plot.source(), &self.startup, &mut observer)
            .map_err(|_| WorksetRefusal::Catalog)?;
        let expanded = self
            .cache
            .expand_observed(
                &source,
                plot.name(),
                &self.profile,
                &self.backs,
                &mut observer,
            )
            .map_err(|_| WorksetRefusal::Catalog)?;
        // Only Patchbay has an explicitly admitted resident return Fore. Other
        // installed roots stay closed; every output boundary remains unsupported.
        if !expanded.output_bindings.is_empty()
            || (!expanded.input_bindings.is_empty() && plot != NativePlot::Patchbay)
        {
            return Err(WorksetRefusal::Catalog);
        }
        Ok(expanded)
    }
}

// Observe actual cold cache work. Hits must not claim parsing or expansion.
fn preparation_observer(context: &'static str) -> impl FnMut(conduit_plot::SourcePreparationEvent) {
    use conduit_plot::SourcePreparationEvent as Event;
    let mut active: Option<Progress> = None;
    move |event| match event {
        Event::ParseStarted => active = Some(Progress::begin(Phase::SourceParse, context)),
        Event::CheckStarted => active = Some(Progress::begin(Phase::SourceCheck, context)),
        Event::ExpansionStarted => active = Some(Progress::begin(Phase::SourceExpand, context)),
        Event::ParseFinished => {
            if let Some(span) = active.take() {
                span.finish();
            }
        }
        Event::CheckFinished { accepted } | Event::ExpansionFinished { accepted } => {
            if let Some(span) = active.take() {
                if accepted {
                    span.finish();
                } else {
                    span.refuse();
                }
            }
        }
        Event::CheckHit | Event::ExpansionHit => {}
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
    super::patchbay_ingress::install_catalogs(&mut startup, &mut profile)?;
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
    with_authoring(plot, |authored| visit(&authored.expanded))
}

pub(super) fn with_authoring<T>(
    plot: NativePlot,
    visit: impl FnOnce(&ExpandedAuthoringPlot) -> T,
) -> Result<T, WorksetRefusal> {
    let _timing =
        crate::native_observation::Interval::begin(crate::native_observation::Stage::PlotBuild);
    Progress::result(Phase::PlotBuild, plot.name(), || {
        // Preparation is outside kernel Steps. Do not spin, retry, or block input
        // if another preparation owns the cache: use the ordinary finite cold path.
        if PREPARATION
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            let expanded = NativePreparation::new()?.observed(plot, true)?;
            return Ok(visit(&expanded));
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
            .observed(plot, false)?;
        Ok(visit(&expanded))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_mode_has_real_zero_retention_or_real_cache_hits() {
        let mut prepared = NativePreparation::new().unwrap();
        let plot = super::super::candidate_inventory()[0];
        prepared.expanded(plot).unwrap();
        prepared.expanded(plot).unwrap();
        let stats = prepared.cache.stats();
        if cfg!(feature = "native-cache-zero-retention") {
            assert_eq!(
                (stats.checks, stats.expansions, stats.unretained_checks),
                (2, 2, 2)
            );
            assert_eq!((stats.check_hits, stats.expansion_hits), (0, 0));
            assert_eq!(
                (
                    prepared.cache.retained_sources(),
                    prepared.cache.retained_expansions()
                ),
                (0, 0)
            );
        } else {
            assert_eq!(
                (stats.checks, stats.expansions, stats.unretained_checks),
                (1, 1, 0)
            );
            assert_eq!((stats.check_hits, stats.expansion_hits), (1, 1));
            assert_eq!(
                (
                    prepared.cache.retained_sources(),
                    prepared.cache.retained_expansions()
                ),
                (1, 1)
            );
        }
    }

    #[test]
    fn zero_retention_rechecks_same_inventory_without_changing_meaning() {
        let mut retained = NativePreparation::with_retention(true).unwrap();
        let mut control = NativePreparation::with_retention(false).unwrap();
        for plot in super::super::candidate_inventory() {
            let expected = retained.expanded(plot).unwrap();
            for _ in 0..2 {
                let actual = control.expanded(plot).unwrap();
                assert_eq!(*actual, *expected);
                assert!(!Arc::ptr_eq(&actual, &expected));
            }
        }
        let count = super::super::NATIVE_CANDIDATE_CAPACITY as u64;
        let stats = control.cache.stats();
        assert_eq!(stats.checks, count * 2);
        assert_eq!(stats.expansions, count * 2);
        assert_eq!(stats.unretained_checks, count * 2);
        assert_eq!((stats.check_hits, stats.expansion_hits), (0, 0));
        assert_eq!(
            (
                control.cache.retained_sources(),
                control.cache.retained_expansions()
            ),
            (0, 0)
        );
        assert_eq!(*control.startup, *retained.startup);
        assert_eq!(*control.profile, *retained.profile);
        assert_eq!(*control.backs, *retained.backs);
    }

    #[test]
    fn every_native_source_reuses_exact_cold_meaning_with_finite_entries() {
        fn assert_send<T: Send>() {}
        assert_send::<NativePreparation>();
        let mut prepared = NativePreparation::with_retention(true).unwrap();
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
