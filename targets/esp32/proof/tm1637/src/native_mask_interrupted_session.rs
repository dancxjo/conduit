//! Interruption settlement owns the actual returned stores and electrical owner;
//! it never claims the parent Mask completed its entire reserved child range.
use super::*;

pub struct InterruptedMask {
    parts: RetiredMaskParts,
    settled_children: u64,
}
impl InterruptedMask {
    pub fn renderer_identity(&self) -> &r::RendererIdentity {
        self.parts.renderer.identity()
    }
    pub fn settled_children(&self) -> u64 {
        self.settled_children
    }
    pub fn into_parts(self) -> RetiredMaskParts {
        self.parts
    }
    pub(crate) fn matches_current_body(
        &self,
        body: &conduit_body::SealedBodyLifecycleSession,
        initial: &c::ActivePlayIdentity,
    ) -> bool {
        self.parts.matches_current_body(body, initial)
    }
}
impl NativeMaskSession {
    pub(crate) fn interruption_ready(
        &self,
        leases: &MaskLeaseIssuer,
        evidence: &Consumer,
    ) -> Result<(), &'static str> {
        if !(terminal::Interruption {
            facts: self.terminal_facts(leases, evidence)?,
        })
        .ready()
        {
            return Err("mask-interruption-unsettled");
        }
        if self.context.plan_id() != leases.plan_id() {
            return Err("mask-interruption-plan");
        }
        Ok(())
    }
    /// Refusal returns every incoming owner unchanged. Acknowledged withdrawal,
    /// exact accepted terminal evidence and actual drained kernel are required.
    pub(crate) fn retire_interrupted(
        self: Box<Self>,
        leases: Box<MaskLeaseIssuer>,
        evidence: Box<Consumer>,
    ) -> Result<InterruptedMask, CompletionRefusal> {
        if let Err(reason) = self.interruption_ready(&leases, &evidence) {
            return Err((self, leases, evidence, reason));
        }
        let Self {
            ready,
            session,
            context,
            invocation,
            basis,
            producer,
            reader,
            index,
            ..
        } = *self;
        let kernel = session
            .try_retire()
            .unwrap_or_else(|_| unreachable!("exclusive drained interruption preflight"));
        Ok(InterruptedMask {
            parts: RetiredMaskParts {
                renderer: ready.expect("preflighted returned renderer"),
                kernel,
                leases,
                evidence,
                context,
                invocation,
                basis,
                producer,
                reader,
            },
            settled_children: index,
        })
    }
}
