//! Immutable runtime profile ownership without authoring catalogs.
//!
//! This container retains already prepared original components. It does not
//! declare their combination selected/admitted, resource-complete or runnable:
//! the concrete driver must bind the exact Source/Plan and use each bounded
//! factory entrance before Load. Startup/Profile catalogs, checker documents,
//! import text and planning lookup maps belong to preparation and can be dropped.
use alloc::{sync::Arc, vec::Vec};
use conduit_ai::{
    closing_structured_pair::ClosingStructuredPairProfile, fixed_numeric_guard::FixedGuardProfile,
    native_profile::PreparedNativeProfile, nominal_weakening::PreparedNominalWeakening,
};
use conduit_core::Plan;

pub struct PreparedRuntimeProfiles {
    source: Arc<str>,
    plan: Arc<Plan>,
    native: Vec<Arc<PreparedNativeProfile>>,
    weakening: Vec<Arc<PreparedNominalWeakening>>,
    guards: Vec<FixedGuardProfile>,
    zip: crate::flow_zip::FlowZipOperationFactory,
    pairs: Vec<ClosingStructuredPairProfile>,
}
impl PreparedRuntimeProfiles {
    #[allow(clippy::too_many_arguments)]
    pub fn retain(
        source: Arc<str>,
        plan: Arc<Plan>,
        native: Vec<Arc<PreparedNativeProfile>>,
        weakening: Vec<Arc<PreparedNominalWeakening>>,
        guards: Vec<FixedGuardProfile>,
        zip: crate::flow_zip::FlowZipOperationFactory,
        pairs: Vec<ClosingStructuredPairProfile>,
    ) -> Self {
        Self {
            source,
            plan,
            native,
            weakening,
            guards,
            zip,
            pairs,
        }
    }
    pub fn source(&self) -> &Arc<str> {
        &self.source
    }
    pub fn plan(&self) -> &Arc<Plan> {
        &self.plan
    }
    pub fn native(&self) -> &[Arc<PreparedNativeProfile>] {
        &self.native
    }
    pub fn weakening(&self) -> &[Arc<PreparedNominalWeakening>] {
        &self.weakening
    }
    pub fn guards(&self) -> &[FixedGuardProfile] {
        &self.guards
    }
    pub fn zip(&self) -> &crate::flow_zip::FlowZipOperationFactory {
        &self.zip
    }
    pub fn pairs(&self) -> &[ClosingStructuredPairProfile] {
        &self.pairs
    }
}
