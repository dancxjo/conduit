//! Independently retained immutable original model and Plan owners.
//! Every receipt keeps this full base alive; pointer membership avoids charging
//! an equal but separately allocated Plan against another owner's reservation.
use crate::{
    parser_model_selection::PreparedParserModelSelection,
    parser_session_numeric_plan_storage::numeric_plan_retained_bytes,
    parser_session_profile::{ParserSessionCapabilities, PreparedParserSessionProfile},
};
use alloc::{rc::Rc, sync::Arc, vec::Vec};
use conduit_core::Plan;
use core::mem::{align_of, size_of};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoricalBaseRefusal {
    Limits,
    Plan,
    Overflow,
    Pressure,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct HistoricalBaseStorage {
    pub(crate) complete_model_profile_bytes: usize,
    pub(crate) original_plan_bytes: usize,
    pub(crate) static_artifact_bytes: usize,
    pub(crate) complete_base_bytes_bound: usize,
}
pub(crate) struct ParserSessionHistoricalBase {
    pub(crate) model: Arc<PreparedParserModelSelection>,
    pub(crate) capabilities: ParserSessionCapabilities,
    plans: Vec<Rc<Plan>>,
    pub(crate) storage: HistoricalBaseStorage,
}
fn add(a: usize, b: usize) -> Result<usize, HistoricalBaseRefusal> {
    a.checked_add(b).ok_or(HistoricalBaseRefusal::Overflow)
}
impl ParserSessionHistoricalBase {
    /// Source/Plan/Native-law preparation is checked before this assembly. The
    /// exact immutable owners supplied here remain the only admitted Plan set.
    /// Static bytes come from the Session's fixed descriptor/Source inventory.
    pub(crate) fn prepare(
        profile: PreparedParserSessionProfile,
        plans: Vec<Rc<Plan>>,
        static_artifact_bytes: usize,
        maximum_complete_base_bytes: usize,
    ) -> Result<Rc<Self>, HistoricalBaseRefusal> {
        use HistoricalBaseRefusal as R;
        if plans.is_empty() || plans.len() > 64 {
            return Err(R::Limits);
        }
        let owner = add(
            add(size_of::<Self>(), 2 * size_of::<usize>())?,
            4 * align_of::<Self>(),
        )?;
        let slots = plans
            .capacity()
            .checked_mul(size_of::<Rc<Plan>>())
            .ok_or(R::Overflow)?;
        let mut retained = add(owner, slots)?;
        for (index, plan) in plans.iter().enumerate() {
            if plans[..index].iter().any(|prior| Rc::ptr_eq(prior, plan)) {
                continue;
            }
            // The original Plan allocation and Rc header are distinct from all
            // of its owned strings/collections walked by the exhaustive helper.
            let plan_owner = add(
                add(size_of::<Plan>(), 2 * size_of::<usize>())?,
                4 * align_of::<Plan>(),
            )?;
            retained = add(
                retained,
                add(
                    plan_owner,
                    numeric_plan_retained_bytes(plan).map_err(|_| R::Plan)?,
                )?,
            )?;
        }
        let combined = add(
            add(
                retained,
                profile.storage.combined_existing_input_bytes_bound,
            )?,
            static_artifact_bytes,
        )?;
        if combined > maximum_complete_base_bytes {
            return Err(R::Pressure);
        }
        Ok(Rc::new(Self {
            model: profile.selection,
            capabilities: profile.capabilities,
            plans,
            storage: HistoricalBaseStorage {
                complete_model_profile_bytes: profile.storage.combined_existing_input_bytes_bound,
                original_plan_bytes: retained,
                static_artifact_bytes,
                complete_base_bytes_bound: combined,
            },
        }))
    }
    pub(crate) fn contains_plan(&self, plan: &Rc<Plan>) -> bool {
        self.plans.iter().any(|original| Rc::ptr_eq(original, plan))
    }
}
