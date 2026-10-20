//! Exact pre-start binding of prepared child kernels to a parent Play.
use super::*;

impl KernelCompositeHost {
    /// Bind prepared child kernel identities to one admitted containing Play.
    /// Each invocation of a selected child Plan gets a distinct identity even
    /// when the same Host executes every invocation.
    pub fn bind_parent_play(
        &mut self,
        parent: &ActivePlayId,
        activation_id: &str,
        invocation: u16,
    ) -> Result<(), KernelCompositeError> {
        if self.started || self.cancelled || self.parent_play_binding.is_some() {
            return Err(KernelCompositeError::InvalidLifecycle);
        }
        for fragment in &self.definition.internal_plan.fragments {
            let child = child_play_id(
                parent,
                activation_id,
                &self.definition.internal_plan.plan_id,
                fragment,
                invocation,
            );
            let slot = self
                .active_plays
                .get_mut(&fragment.host_id)
                .ok_or_else(|| KernelCompositeError::StaleChild(fragment.host_id.clone()))?;
            *slot = child;
        }
        self.parent_play_binding = Some((parent.clone(), activation_id.into(), invocation));
        Ok(())
    }

    pub fn parent_play_binding(&self) -> Option<(&ActivePlayId, u16)> {
        self.parent_play_binding
            .as_ref()
            .map(|(play, _, invocation)| (play, *invocation))
    }
}

pub(super) fn child_play_id(
    parent: &ActivePlayId,
    activation_id: &str,
    plan: &conduit_core::PlanId,
    fragment: &conduit_core::PlanFragment,
    invocation: u16,
) -> ActivePlayId {
    let mut exact = Vec::new();
    for field in [
        parent.as_str(),
        activation_id,
        plan.as_str(),
        fragment.fragment_id.as_str(),
        fragment.host_id.as_str(),
        fragment.boot_id.as_str(),
    ] {
        exact.extend_from_slice(&(field.len() as u32).to_le_bytes());
        exact.extend_from_slice(field.as_bytes());
    }
    exact.extend_from_slice(&invocation.to_le_bytes());
    let digest = conduit_core::semantic_digest("conduit/parent-bound-child-play@1", &exact);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        use core::fmt::Write;
        write!(&mut encoded, "{byte:02x}").expect("write child Play digest");
    }
    ActivePlayId::from(encoded)
}
