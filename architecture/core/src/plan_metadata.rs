//! Verified read-only lowering material with separately encoded configuration.
//! This is local verification evidence, never resource possession or Play authority.
use super::*;
#[derive(Debug, PartialEq, Eq)]
pub enum PlanMetadataRefusal<E> {
    UnsupportedActivations,
    RetainedConfiguration,
    Configuration(E),
    InvalidPlan,
}
/// Owns metadata whose full canonical Plan was checked with streamed payloads.
/// No deserialization or unchecked constructor. Configuration bytes are retired;
/// this cannot substitute for a complete executable/serialized Plan.
#[derive(Debug)]
pub struct VerifiedPlanMetadata {
    plan: Plan,
}
/// Read-only metadata borrowed from a verified complete-plan scope.
pub struct VerifiedFragmentMetadata<'a> {
    fragment: &'a PlanFragment,
}
impl VerifiedPlanMetadata {
    /// Read-only configuration-free preparation projection within this verified
    /// scope. Body validators may inspect its identities and graph structure;
    /// it is not a replacement for a complete serialized/executable Plan.
    pub fn preparation_metadata(&self) -> &Plan {
        &self.plan
    }
    pub fn plan_id(&self) -> &PlanId {
        &self.plan.plan_id
    }
    pub fn fragment_count(&self) -> usize {
        self.plan.fragments.len()
    }
    pub fn fragment(&self, id: &FragmentId) -> Option<VerifiedFragmentMetadata<'_>> {
        self.plan
            .fragments
            .iter()
            .find(|f| &f.fragment_id == id)
            .map(|fragment| VerifiedFragmentMetadata { fragment })
    }
}
impl VerifiedFragmentMetadata<'_> {
    /// Configuration-free metadata for preparation consumers only. The IDs bind
    /// the complete canonical records verified by the owning token; this view
    /// is not a substitute for a complete, self-verifying serialized PlanFragment.
    /// Consumers must keep this verification scope and must not infer readiness
    /// or effect authority from it. Full configuration inspection requires the
    /// original immutable Source material.
    pub fn lowering_metadata(&self) -> &PlanFragment {
        self.fragment
    }
}
/// Verify exact fingerprints using one separately supplied configuration at a
/// time, then run the same whole-Plan structural checks as `verify_plan`.
/// Activation-owned child Plans require full configuration and are explicitly
/// unsupported by this flat preparation representation. No effects are issued.
pub fn verify_plan_metadata_with_configurations<E>(
    plan: Plan,
    mut configuration: impl FnMut(&PlannedGear) -> Result<Vec<ConfigurationEntry>, E>,
) -> Result<VerifiedPlanMetadata, PlanMetadataRefusal<E>> {
    if !plan.activations.is_empty() || !plan.activation_preparations.is_empty() {
        return Err(PlanMetadataRefusal::UnsupportedActivations);
    }
    if plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .any(|g| !g.configuration.is_empty())
    {
        return Err(PlanMetadataRefusal::RetainedConfiguration);
    }
    let mut refusal = None;
    let valid = crate::plan_verification::verify_at_depth(&plan, 0, &mut |fragment| {
        let mut hash = FragmentFingerprintBuilder::new(fragment.into(), fragment.placements.len());
        for gear in &fragment.placements {
            let entries = match configuration(gear) {
                Ok(entries) => entries,
                Err(error) => {
                    refusal = Some(error);
                    return None;
                }
            };
            hash.push_placement_with_configuration(gear, &entries)
                .ok()?;
        }
        hash.finish(fragment.into()).ok()
    });
    if let Some(error) = refusal {
        return Err(PlanMetadataRefusal::Configuration(error));
    }
    if !valid {
        return Err(PlanMetadataRefusal::InvalidPlan);
    }
    Ok(VerifiedPlanMetadata { plan })
}
