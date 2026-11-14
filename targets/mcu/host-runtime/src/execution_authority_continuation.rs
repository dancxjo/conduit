//! Distinct owner-intent entrypoints, never reattributed online commands.
use super::*;
impl Issuer {
    pub fn authorize_continuation(
        &mut self,
        context: crate::continuation_native::ContinuationNativeContext<'_>,
        policy: crate::execution::Policy<'_>,
        native: &NativeFacts<'_>,
        operation: &Operation<'_>,
        grant_id: &str,
        maximum_bytes: usize,
    ) -> Result<(), Refusal> {
        context.recheck(policy)?;
        continuation_selected(context, native)?;
        let fingerprint = fingerprint_parts(
            context.image_id,
            context.configuration_id,
            context.verified.intent().body_id,
            native,
            operation,
        );
        self.authorize_checked(fingerprint, native, operation, grant_id, maximum_bytes)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn issue_continuation(
        &mut self,
        context: crate::continuation_native::ContinuationNativeContext<'_>,
        policy: crate::execution::Policy<'_>,
        scope: &b::FreshContinuationScope<'_>,
        approved: &b::DelegatedBodyFragment<'_>,
        native: &NativeFacts<'_>,
        op: &Operation<'_>,
        maximum_bytes: usize,
    ) -> Result<Issued, Refusal> {
        context.recheck(policy)?;
        continuation_selected(context, native)?;
        scope
            .check_actual(context.verified, context.actual)
            .map_err(|_| Refusal::Scope)?;
        if !scope.matches_fragment(approved) {
            return Err(Refusal::Scope);
        }
        self.issue_checked(
            context.bootstrap.admission.secret(),
            context.actual,
            context.verified.intent().body_id,
            fingerprint_parts(
                context.image_id,
                context.configuration_id,
                context.verified.intent().body_id,
                native,
                op,
            ),
            approved,
            native,
            op,
            maximum_bytes,
        )
    }
}
fn continuation_selected(
    context: crate::continuation_native::ContinuationNativeContext<'_>,
    native: &NativeFacts<'_>,
) -> Result<(), Refusal> {
    if native.host != &context.actual.host_id
        || native.boot != &context.actual.boot_id
        || native.artifact.as_str() != context.image_id
    {
        return Err(Refusal::Scope);
    }
    Ok(())
}
