//! Fixed two-owner composition; each owner retains its own original admission.
use super::*;
impl InitializedNativeSource {
    pub(crate) fn owners(&self) -> impl Iterator<Item = &Self> {
        std::iter::once(self).chain(self.second.as_deref())
    }
    pub(crate) fn matching(&self, gear: &PlannedGear) -> Option<&Self> {
        self.owners().find(|owner| owner.matches_own(gear))
    }
    pub(crate) fn select_fragment_mut(
        &mut self,
        fragment: &PlanFragment,
    ) -> Result<&mut Self, String> {
        let first = fragment.placements.iter().any(|g| self.matches_own(g));
        let second = self
            .second
            .as_ref()
            .is_some_and(|s| fragment.placements.iter().any(|g| s.matches_own(g)));
        if first && second {
            return Err("one Play cannot select both retained native Source owners".into());
        }
        if second {
            Ok(self
                .second
                .as_deref_mut()
                .expect("selected retained second owner"))
        } else {
            Ok(self)
        }
    }
}
impl crate::StdHost {
    /// Independently retained grants, never inferred from the checked IMAGE or
    /// received Plans. At most two original admitted native owners are present.
    pub fn native_source_authority_grants(&self) -> impl Iterator<Item = &AuthorityGrant> {
        self.native_source
            .iter()
            .flat_map(InitializedNativeSource::owners)
            .map(|owner| &owner.definition.grant)
    }
}
impl image::InitialNativeSource<'_> {
    pub(crate) fn owners(&self) -> impl Iterator<Item = &Self> {
        std::iter::once(self).chain(self.companion.as_deref())
    }
    pub(crate) fn validate_composition(
        &self,
        image: &conduit_host_make::HostImage,
    ) -> Result<(), String> {
        for owner in self.owners() {
            owner.validate_image(image)?;
        }
        if let Some(second) = self.companion.as_ref() {
            if second.companion.is_some()
                || self.definition.base.base_id == second.definition.base.base_id
                || self.definition.base.provider_instance_id
                    == second.definition.base.provider_instance_id
                || self.definition.base.capabilities[0].capability_id
                    == second.definition.base.capabilities[0].capability_id
                || self.definition.base.resources.iter().any(|r| {
                    second
                        .definition
                        .base
                        .resources
                        .iter()
                        .any(|s| s.pool_id == r.pool_id)
                })
            {
                return Err("initial native Source composition requires two distinct exact owners and pools".into());
            }
        }
        Ok(())
    }
}
