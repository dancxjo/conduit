//! Closed whole-Session preparation. No port or model can consume ingress until
//! all original plans, complete Native families and Source owners are ready.
use crate::{
    parser_production_families::{
        PreparedProductionParserFamilies, ProductionFamilyLimits, ProductionFamilyRefusal,
    },
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_fixed_ingress::{FixedRefusal, PreparedParserFixedIngress},
    parser_session_fixed_preparation::{prepare_fixed_target, FixedPreparationLimits},
    parser_session_historical_base::{HistoricalBaseRefusal, ParserSessionHistoricalBase},
    parser_session_mixed_preparation::{
        prepare_mixed_targets, MixedPreparationLimits, MixedPreparationRefusal, OwnedNumericTarget,
        OwnedSourceTarget,
    },
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_profile::{
        ParserSessionProfile, ParserSessionProfileRefusal, PreparedParserSessionProfile,
    },
    parser_session_queries::{
        ParserQueryPreparationLimits, ParserQueryRefusal, PreparedParserSessionQueries,
    },
    parser_session_target_contract::ParserSessionPreparedTarget,
    parser_session_target_registry::{ParserSessionTargetRegistry, RegistryRefusal, REQUIRED},
    LanguageParserV2ChoiceQuery, LanguageParserV2ModelScores,
};
use alloc::{rc::Rc, vec::Vec};
use conduit_core::Plan;
use core::mem::size_of;

#[derive(Clone, Copy)]
pub(crate) struct ParserSessionPreparationLimits {
    pub family: ProductionFamilyLimits,
    pub fixed: FixedPreparationLimits,
    pub mixed: MixedPreparationLimits,
    pub queries: ParserQueryPreparationLimits,
    pub maximum_existing_profile_bytes: usize,
    pub maximum_historical_base_bytes: usize,
    /// Revision frame pools, driver fragments and future historical readmission
    /// are separately proved reservations, retained here before owner creation.
    pub revision_and_driver_reserved_bytes: usize,
    pub maximum_combined_bytes: usize,
}
#[derive(Debug)]
pub(crate) enum ParserSessionPreparationRefusal<E> {
    Pressure,
    Entries,
    Profile(ParserSessionProfileRefusal),
    Family(ProductionFamilyRefusal),
    Fixed(FixedRefusal<E>),
    Mixed(MixedPreparationRefusal),
    Queries(ParserQueryRefusal),
    Historical(HistoricalBaseRefusal),
    Registry(RegistryRefusal),
}
pub(crate) struct PreparedParserSessionOwners<E, S, N>
where
    E: ParserSessionPreparedTarget,
    S: ParserSessionPreparedTarget,
    N: ParserSessionPreparedTarget
        + ParserNumericExecutor<Error = <N as ParserCanonicalSourceExecutor>::Error>,
{
    pub targets: ParserSessionTargetRegistry<E, OwnedSourceTarget<S>, OwnedNumericTarget<N>>,
    pub families: PreparedProductionParserFamilies,
    pub base: Rc<ParserSessionHistoricalBase>,
    pub queries: PreparedParserSessionQueries,
    pub combined_reserved_bytes_bound: usize,
}
fn add(a: usize, b: usize) -> Option<usize> {
    a.checked_add(b)
}
fn mul(a: usize, b: usize) -> Option<usize> {
    a.checked_mul(b)
}

/// Targets are original caller-owned objects. Their complete immutable Source
/// documents and plans remain retained; declared opaque target storage is
/// distinguished from Session-owned allocation bounds. Entry validation and the
/// complete aggregate reservation precede any Session allocation or Source call.
pub(crate) fn prepare_session_owners<E, S, N>(
    targets: Vec<E>,
    source: S,
    numeric: N,
    profile: ParserSessionProfile,
    limits: ParserSessionPreparationLimits,
) -> Result<
    PreparedParserSessionOwners<E, S, N>,
    ParserSessionPreparationRefusal<<E as ParserCanonicalSourceExecutor>::Error>,
>
where
    E: ParserSessionPreparedTarget,
    S: ParserSessionPreparedTarget,
    N: ParserSessionPreparedTarget
        + ParserNumericExecutor<Error = <N as ParserCanonicalSourceExecutor>::Error>,
{
    use ParserSessionPreparationRefusal as R;
    if targets.len() != REQUIRED.len() {
        return Err(R::Entries);
    }
    for &entry in REQUIRED {
        if targets
            .iter()
            .filter(|target| target.entry() == entry.name())
            .count()
            != 1
        {
            return Err(R::Entries);
        }
    }
    let profile =
        PreparedParserSessionProfile::admit(profile, limits.maximum_existing_profile_bytes)
            .map_err(R::Profile)?;
    let model = profile.selection.clone();
    let mut existing = profile.storage.combined_existing_input_bytes_bound;
    // Include actual allocated caller Vec slots as retained existing input.
    existing = add(
        existing,
        mul(targets.capacity(), size_of::<E>()).ok_or(R::Pressure)?,
    )
    .ok_or(R::Pressure)?;
    for target in &targets {
        existing = add(existing, target.storage_contract().combined_bytes()).ok_or(R::Pressure)?;
    }
    existing = add(existing, source.storage_contract().combined_bytes()).ok_or(R::Pressure)?;
    existing = add(existing, numeric.storage_contract().combined_bytes()).ok_or(R::Pressure)?;
    let historical_owner = size_of::<ParserSessionHistoricalBase>()
        .checked_add(2 * size_of::<usize>())
        .and_then(|n| n.checked_add(4 * core::mem::align_of::<ParserSessionHistoricalBase>()))
        .ok_or(R::Pressure)?;
    let slots = add(
        historical_owner,
        add(
            mul(REQUIRED.len(), size_of::<PreparedParserFixedIngress<E>>()).ok_or(R::Pressure)?,
            mul(REQUIRED.len() + 2, size_of::<Rc<Plan>>()).ok_or(R::Pressure)?,
        )
        .ok_or(R::Pressure)?,
    )
    .ok_or(R::Pressure)?;
    // All retained Source reservations coexist. Preparation scratch is reserved
    // once for the largest sequential phase, in addition to these full quotas.
    let source_retained = add(
        mul(REQUIRED.len(), limits.fixed.verification.retained_bytes).ok_or(R::Pressure)?,
        mul(3, limits.mixed.verification.retained_bytes).ok_or(R::Pressure)?,
    )
    .ok_or(R::Pressure)?;
    let fixed_scratch = add(
        add(
            limits.fixed.verification.preparation_peak_bytes,
            limits.fixed.maximum_plan_validation_temporary_bytes,
        )
        .ok_or(R::Pressure)?,
        add(
            limits.fixed.maximum_metadata_temporary_bytes,
            limits.fixed.maximum_endpoint_encoding_requested_bytes,
        )
        .ok_or(R::Pressure)?,
    )
    .ok_or(R::Pressure)?;
    let mixed_scratch = add(
        add(
            mul(3, limits.mixed.verification.preparation_peak_bytes).ok_or(R::Pressure)?,
            limits.mixed.maximum_plan_validation_temporary_bytes,
        )
        .ok_or(R::Pressure)?,
        add(
            limits.mixed.maximum_metadata_temporary_bytes,
            limits.mixed.maximum_endpoint_encoding_requested_bytes,
        )
        .ok_or(R::Pressure)?,
    )
    .ok_or(R::Pressure)?;
    let reserved = add(
        add(
            add(add(existing, slots).ok_or(R::Pressure)?, source_retained).ok_or(R::Pressure)?,
            fixed_scratch.max(mixed_scratch),
        )
        .ok_or(R::Pressure)?,
        add(
            add(
                limits.queries.maximum_preparation_requested_bytes,
                limits.mixed.canonical.maximum_preparation_peak_bytes,
            )
            .ok_or(R::Pressure)?,
            limits.revision_and_driver_reserved_bytes,
        )
        .ok_or(R::Pressure)?,
    )
    .ok_or(R::Pressure)?;
    if reserved > limits.maximum_combined_bytes {
        return Err(R::Pressure);
    }
    let mut family_limits = limits.family;
    family_limits.other_reserved_bytes = reserved;
    family_limits.maximum_combined_bytes = limits.maximum_combined_bytes;
    // Two full Native values coexist only inside a typed mixed Source execution.
    family_limits.concurrent_native_values = family_limits.concurrent_native_values.max(2);
    let families = PreparedProductionParserFamilies::prepare(family_limits).map_err(R::Family)?;
    let family_receipt = families.receipt();
    let static_bytes = add(
        add(
            family_receipt.static_resources.canonical_bytes_bound,
            family_receipt
                .static_resources
                .descriptor_storage_bytes_bound,
        )
        .ok_or(R::Pressure)?,
        family_receipt.static_resources.source_canonical_bytes_bound,
    )
    .ok_or(R::Pressure)?;
    let live_native = mul(
        family_limits.family.maximum_conversion_requested_bytes,
        family_limits.concurrent_native_values,
    )
    .ok_or(R::Pressure)?;
    let base_reserved = add(
        add(add(existing, slots).ok_or(R::Pressure)?, static_bytes).ok_or(R::Pressure)?,
        add(family_receipt.retained_heap_bytes_bound, live_native).ok_or(R::Pressure)?,
    )
    .ok_or(R::Pressure)?;
    let mut plans = Vec::new();
    plans
        .try_reserve_exact(REQUIRED.len() + 2)
        .map_err(|_| R::Pressure)?;
    let mut ports = Vec::new();
    ports
        .try_reserve_exact(REQUIRED.len())
        .map_err(|_| R::Pressure)?;
    let mut retained_sources = 0usize;
    for target in targets {
        let entry = *REQUIRED
            .iter()
            .find(|entry| entry.name() == target.entry())
            .ok_or(R::Entries)?;
        plans.push(target.original_plan_owner());
        let mut selected = limits.fixed;
        selected.other_existing_session_reserved_bytes = add(
            base_reserved,
            add(retained_sources, limits.revision_and_driver_reserved_bytes).ok_or(R::Pressure)?,
        )
        .ok_or(R::Pressure)?;
        selected.maximum_combined_bytes = limits.maximum_combined_bytes;
        let (port, receipt) = prepare_fixed_target(
            target,
            entry,
            families.for_entry(entry).map_err(R::Family)?,
            selected,
        )
        .map_err(R::Fixed)?;
        retained_sources =
            add(retained_sources, receipt.source.retained_heap_bytes_bound).ok_or(R::Pressure)?;
        ports.push(port);
    }
    plans.push(source.original_plan_owner());
    plans.push(numeric.original_plan_owner());
    let mut selected = limits.mixed;
    selected.other_existing_session_reserved_bytes = add(
        base_reserved,
        add(retained_sources, limits.revision_and_driver_reserved_bytes).ok_or(R::Pressure)?,
    )
    .ok_or(R::Pressure)?;
    selected.maximum_combined_bytes = limits.maximum_combined_bytes;
    let family = families
        .for_values::<LanguageParserV2ChoiceQuery, LanguageParserV2ModelScores>()
        .map_err(R::Family)?;
    let (mixed, mixed_receipt) =
        prepare_mixed_targets(source, numeric, model, family, selected).map_err(R::Mixed)?;
    let queries =
        PreparedParserSessionQueries::prepare(&families, limits.queries).map_err(R::Queries)?;
    let base = ParserSessionHistoricalBase::prepare(
        profile,
        plans,
        static_bytes,
        limits.maximum_historical_base_bytes,
    )
    .map_err(R::Historical)?;
    let targets = ParserSessionTargetRegistry::from_prepared(ports, mixed).map_err(R::Registry)?;
    let combined = add(
        add(base_reserved, retained_sources).ok_or(R::Pressure)?,
        add(
            add(
                mixed_receipt.retained_source_bytes_bound,
                mixed_receipt.retained_canonical_codec_bytes_bound,
            )
            .ok_or(R::Pressure)?,
            add(
                queries.receipt().retained_requested_bytes_bound,
                limits.revision_and_driver_reserved_bytes,
            )
            .ok_or(R::Pressure)?,
        )
        .ok_or(R::Pressure)?,
    )
    .ok_or(R::Pressure)?;
    if combined > limits.maximum_combined_bytes {
        return Err(R::Pressure);
    }
    Ok(PreparedParserSessionOwners {
        targets,
        families,
        base,
        queries,
        combined_reserved_bytes_bound: combined,
    })
}

/// The public target contract is sufficient for numerical ports as well. This
/// private adapter retains the exact original target and delegates its complete
/// canonical frame transport; it supplies no new model or execution authority.
pub(crate) struct NumericTargetBridge<T: ParserSessionPreparedTarget>(pub T);
impl<T: ParserSessionPreparedTarget> ParserCanonicalSourceExecutor for NumericTargetBridge<T> {
    type Error = T::Error;
    fn cancel(&mut self) {
        ParserCanonicalSourceExecutor::cancel(&mut self.0)
    }
    fn entry(&self) -> &str {
        self.0.entry()
    }
    fn input_type_bytes(&self) -> &[u8] {
        self.0.input_type_bytes()
    }
    fn output_type_bytes(&self) -> &[u8] {
        self.0.output_type_bytes()
    }
    fn transact(
        &mut self,
        ordinal: u64,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, Self::Error> {
        ParserCanonicalSourceExecutor::transact(&mut self.0, ordinal, input, output)
    }
}
impl<T: ParserSessionPreparedTarget> crate::parser_session_fixed_ingress::ParserSessionExecutor
    for NumericTargetBridge<T>
{
    fn original_plan(&self) -> &Plan {
        self.0.original_plan()
    }
}
impl<T: ParserSessionPreparedTarget> ParserSessionPreparedTarget for NumericTargetBridge<T> {
    fn checked_source(&self) -> &conduit_plot::CheckedSyntaxDocument {
        self.0.checked_source()
    }
    fn expanded_source(&self) -> &conduit_plot::ExpandedAuthoringPlot {
        self.0.expanded_source()
    }
    fn original_plan_owner(&self) -> Rc<Plan> {
        self.0.original_plan_owner()
    }
    fn storage_contract(
        &self,
    ) -> crate::parser_session_target_contract::ParserSessionTargetStorageContract {
        self.0.storage_contract()
    }
}
impl<T: ParserSessionPreparedTarget> ParserNumericExecutor for NumericTargetBridge<T> {
    type Error = T::Error;
    fn plan(&self) -> &Plan {
        self.0.original_plan()
    }
    fn transact(
        &mut self,
        ordinal: u64,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, Self::Error> {
        ParserCanonicalSourceExecutor::transact(&mut self.0, ordinal, input, output)
    }
    fn cancel(&mut self) {
        ParserCanonicalSourceExecutor::cancel(&mut self.0)
    }
}
