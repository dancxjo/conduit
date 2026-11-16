//! Retained pure operation owners consumed together by a native protocol Play.
#[derive(Default)]
pub struct ProtocolOperations {
    pub singletons: crate::flow_singleton::FlowSingletonOperationFactory,
    pub joins: crate::flow_zip::FlowZipOperationFactory,
    pub states: crate::seeded_state::SeededStateOperationFactory,
    pub concats: crate::flow_concat_finite::FlowConcatFiniteOperationFactory,
    pub merges: crate::flow_merge_finite::FlowMergeFiniteOperationFactory,
}

/// A sealed Plan is checked once before the same immutable fragment is handed
/// to each retained operation family. This proof grants no native authority.
#[derive(Debug)]
pub(crate) struct OperationPlanRefusal {
    pub family: &'static str,
    pub reason: alloc::string::String,
}
impl ProtocolOperations {
    pub(crate) fn validate_plan(
        &self,
        plan: &conduit_core::Plan,
    ) -> Result<(), OperationPlanRefusal> {
        if plan.fragments.len() != 1 || !conduit_core::verify_plan(plan) {
            return Err(OperationPlanRefusal {
                family: "plan",
                reason: "native operations require one exact sealed fragment".into(),
            });
        }
        let fragment = &plan.fragments[0];
        let refusal = |family| move |reason| OperationPlanRefusal { family, reason };
        self.joins
            .validate_selections(fragment)
            .map_err(refusal("zip"))?;
        self.states
            .validate_selections(fragment)
            .map_err(refusal("states"))?;
        self.concats
            .validate_selections(fragment)
            .map_err(refusal("concats"))?;
        self.merges
            .validate_selections(fragment)
            .map_err(refusal("merges"))?;
        self.singletons
            .validate_selections(fragment)
            .map_err(refusal("singletons"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
