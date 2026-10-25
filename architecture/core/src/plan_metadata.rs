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
    configurations: Vec<Vec<[u8; 32]>>,
}
/// Read-only metadata borrowed from a verified complete-plan scope.
pub struct VerifiedFragmentMetadata<'a> {
    fragment: &'a PlanFragment,
    configurations: &'a [[u8; 32]],
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
            .enumerate()
            .find(|(_, f)| &f.fragment_id == id)
            .map(|(index, fragment)| VerifiedFragmentMetadata {
                fragment,
                configurations: &self.configurations[index],
            })
    }
}
impl VerifiedFragmentMetadata<'_> {
    /// Compare independently retained native expectations with the exact
    /// configuration committed during whole-Plan verification. Ordering and
    /// semantic value tags remain significant. This grants no possession.
    pub fn configuration_matches(
        &self,
        placement: &PlacementId,
        expected: &[ConfigurationEntry],
    ) -> bool {
        let mut matches = self
            .fragment
            .placements
            .iter()
            .enumerate()
            .filter(|(_, gear)| &gear.placement_id == placement);
        let Some((index, _)) = matches.next() else {
            return false;
        };
        matches.next().is_none()
            && self.configurations.get(index)
                == Some(&crate::plan_fingerprint::configuration_fingerprint(
                    expected,
                ))
    }
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
    let mut configurations = Vec::with_capacity(plan.fragments.len());
    let valid = crate::plan_verification::verify_at_depth(&plan, 0, &mut |fragment| {
        let mut hash = FragmentFingerprintBuilder::new(fragment.into(), fragment.placements.len());
        let mut commitments = Vec::with_capacity(fragment.placements.len());
        for gear in &fragment.placements {
            let entries = match configuration(gear) {
                Ok(entries) => entries,
                Err(error) => {
                    refusal = Some(error);
                    return None;
                }
            };
            commitments.push(crate::plan_fingerprint::configuration_fingerprint(&entries));
            hash.push_placement_with_configuration(gear, &entries)
                .ok()?;
        }
        let id = hash.finish(fragment.into()).ok()?;
        configurations.push(commitments);
        Some(id)
    });
    if let Some(error) = refusal {
        return Err(PlanMetadataRefusal::Configuration(error));
    }
    if !valid {
        return Err(PlanMetadataRefusal::InvalidPlan);
    }
    Ok(VerifiedPlanMetadata {
        plan,
        configurations,
    })
}
