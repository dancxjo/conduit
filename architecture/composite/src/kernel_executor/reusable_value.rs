//! Pre-admitted identity/evidence transfers for one reused pure child owner.
use super::*;
use alloc::sync::Arc;
use conduit_kernel::HostedSignLog;

/// Built before Play by the exact owner. A binding from another owner cannot
/// be substituted even if its source hashes and Plan identities are equal.
pub struct PreparedValueInvocation {
    definition: Arc<KernelCompositeDefinition>,
    parent: ActivePlayId,
    activation: String,
    invocation: u16,
    active_plays: BTreeMap<HostId, ActivePlayId>,
}
/// Complete immutable prior evidence, moved out without cloning a kernel,
/// discarding mandatory remote lifecycle records or allocating during Play.
pub struct ValueInvocationEvidence {
    pub(crate) definition: Arc<KernelCompositeDefinition>,
    pub(crate) parent: ActivePlayId,
    pub(crate) activation: String,
    pub(crate) invocation: u16,
    pub(crate) active_plays: BTreeMap<HostId, ActivePlayId>,
    pub(crate) signs: HostedSignLog,
    pub decisions: u32,
    pub logical_invocations: u64,
}
impl ValueInvocationEvidence {
    pub fn signs(&self) -> &HostedSignLog {
        &self.signs
    }
    pub fn parent_play_binding(&self) -> (&ActivePlayId, u16) {
        (&self.parent, self.invocation)
    }
    pub fn active_plays(&self) -> &BTreeMap<HostId, ActivePlayId> {
        &self.active_plays
    }
    pub fn definition(&self) -> &KernelCompositeDefinition {
        &self.definition
    }
    pub fn activation_id(&self) -> &str {
        &self.activation
    }
}
impl KernelCompositeHost {
    /// Inspection snapshot; mandatory remote sidecars remain owned until disposal.
    pub fn remote_signs(
        &self,
    ) -> BTreeMap<HostId, Vec<(u32, conduit_kernel::RemoteLifecycleIdentity)>> {
        self.children
            .iter()
            .map(|(host, child)| (host.clone(), child.remote_signs()))
            .collect()
    }
    /// Reserve identity bytes before the containing Play begins. This is not a
    /// fresh Host/Boot observation or permission to invoke the child.
    pub fn prepare_value_invocation(
        &self,
        parent: &ActivePlayId,
        activation: &str,
        invocation: u16,
    ) -> Result<PreparedValueInvocation, KernelCompositeError> {
        if self.started || self.cancelled {
            return Err(KernelCompositeError::InvalidLifecycle);
        }
        let active_plays = self
            .definition
            .internal_plan
            .fragments
            .iter()
            .map(|fragment| {
                (
                    fragment.host_id.clone(),
                    parent_play::child_play_id(
                        parent,
                        activation,
                        &self.definition.internal_plan.plan_id,
                        fragment,
                        invocation,
                    ),
                )
            })
            .collect();
        Ok(PreparedValueInvocation {
            definition: Arc::clone(&self.definition),
            parent: parent.clone(),
            activation: activation.into(),
            invocation,
            active_plays,
        })
    }
    /// Reserve a complete next Sign log before Play, including remote sidecars.
    pub fn empty_value_evidence(&self) -> Result<HostedSignLog, KernelCompositeError> {
        if self.started || self.cancelled || self.children.len() != 1 {
            return Err(KernelCompositeError::InvalidLifecycle);
        }
        self.children
            .values()
            .next()
            .unwrap()
            .empty_value_evidence()
            .map_err(KernelCompositeError::InvalidBoundary)
    }
    /// The containing controller must have accepted the current output. Kernel
    /// drain alone never advances the controller's recurrent accumulator.
    /// A refusal leaves old mandatory evidence owned; there is no cold fallback.
    pub fn rearm_value(
        &mut self,
        next: PreparedValueInvocation,
        signs: HostedSignLog,
    ) -> Result<ValueInvocationEvidence, KernelCompositeError> {
        let Some((parent, activation, invocation)) = self.parent_play_binding.as_ref() else {
            return Err(KernelCompositeError::InvalidLifecycle);
        };
        if !self.started
            || self.cancelled
            || self.children.len() != 1
            || !self.links.is_empty()
            || self.outstanding_host_calls.iter().any(Option::is_some)
            || !Arc::ptr_eq(&next.definition, &self.definition)
            || next.parent != *parent
            || next.activation != *activation
            || invocation.checked_add(1) != Some(next.invocation)
        {
            return Err(KernelCompositeError::InvalidLifecycle);
        }
        let old = self
            .children
            .values_mut()
            .next()
            .unwrap()
            .rearm_value(signs)
            .map_err(|reason| KernelCompositeError::ValueRearm { reason })?;
        let old_binding = self
            .parent_play_binding
            .replace((next.parent, next.activation, next.invocation))
            .unwrap();
        let active_plays = core::mem::replace(&mut self.active_plays, next.active_plays);
        self.started = false;
        Ok(ValueInvocationEvidence {
            definition: next.definition,
            parent: old_binding.0,
            activation: old_binding.1,
            invocation: old_binding.2,
            active_plays,
            signs: old.signs,
            decisions: old.decisions,
            logical_invocations: old.logical_invocations,
        })
    }
}

#[cfg(test)]
mod tests;
