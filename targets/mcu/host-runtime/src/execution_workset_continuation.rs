//! Explicit installed owner-intent origin through the ordinary whole-set owner.
use super::*;
use crate::continuation_native::ContinuationNativeContext;
use alloc::vec::Vec;

impl<'w, T: PreparedLocalKernels, const F: usize> PreparedLocalWorkset<'w, T, F> {
    /// Receive the complete checked fresh local continuation set. Profiles and
    /// factory remain installed Root material. This grants no Start, membership,
    /// remote execution, or owner-global Playing state.
    pub fn prepare_continuation_selected<'p, const PORTS: usize, E>(
        scope: &b::FreshContinuationScope<'w>,
        context: ContinuationNativeContext<'_>,
        policy: Policy<'_>,
        mut profile_for: impl FnMut(usize, &[u8]) -> Result<b::AssignedBodyPreparationProfile<'p>, E>,
        factory: impl FnOnce(
            &[b::DelegatedBodyFragment<'w>; F],
            &[b::ApprovedBodyForePorts<PORTS>; F],
        ) -> Result<T, E>,
    ) -> Result<Self, Refusal<E>> {
        context.recheck(policy).map_err(|_| Refusal::Scope)?;
        scope
            .check_actual(context.verified, context.actual)
            .map_err(|_| Refusal::Scope)?;
        if F == 0 || scope.fragment_count() != F {
            return Err(Refusal::Scope);
        }
        let mut fragments = Vec::new();
        let mut ports = Vec::new();
        fragments
            .try_reserve_exact(F)
            .map_err(|_| Refusal::Bounds)?;
        ports.try_reserve_exact(F).map_err(|_| Refusal::Bounds)?;
        let preparation = policy.preparation.ok_or(Refusal::Scope)?;
        let mut declared_runtime = 0u32;
        for index in 0..F {
            let document = scope.document(index).ok_or(Refusal::Scope)?;
            let mut profile = profile_for(index, document).map_err(Refusal::Back)?;
            // ASG declared state remains independently bounded per fragment and
            // across the complete local set. Physical backing has its own bound.
            profile.maxima.runtime_state_bytes = profile
                .maxima
                .runtime_state_bytes
                .min(preparation.maximum_runtime_state_bytes);
            let (fragment, fore) = scope
                .approve_fragment::<PORTS>(index, context.verified, context.actual, profile)
                .map_err(|_| Refusal::Scope)?;
            declared_runtime = declared_runtime
                .checked_add(fragment.view().runtime_state_bytes)
                .ok_or(Refusal::Bounds)?;
            if declared_runtime > preparation.maximum_runtime_state_bytes {
                return Err(Refusal::Bounds);
            }
            fragments.push(fragment);
            ports.push(fore);
        }
        let fragments: Box<[_; F]> = fragments
            .into_boxed_slice()
            .try_into()
            .map_err(|_| Refusal::Bounds)?;
        let ports: Box<[_; F]> = ports
            .into_boxed_slice()
            .try_into()
            .map_err(|_| Refusal::Bounds)?;
        let prepared = factory(&fragments, &ports).map_err(Refusal::Back)?;
        let retained = prepared
            .retained_backing_bytes()
            .and_then(|n| n.checked_add(core::mem::size_of::<[b::DelegatedBodyFragment<'w>; F]>()))
            .ok_or(Refusal::Bounds)?;
        let intent_maximum = usize::try_from(context.verified.intent().ceilings.runtime_bytes)
            .map_err(|_| Refusal::Bounds)?;
        let maximum = policy.maximum_retained_bytes.min(intent_maximum);
        if retained > maximum {
            return Err(Refusal::Capacity {
                required: retained,
                maximum,
            });
        }
        Ok(Self {
            fragments,
            origin: StartOrigin::Continuation(scope.custody_identity()),
            prepared,
            maximum,
            retained,
        })
    }
}
impl<T: PreparedLocalKernels, const F: usize> CommittedLocalWorkset<'_, T, F> {
    /// Recheck the same owner-signed installation, fresh canonical scope and exact
    /// admitted ledger. Root retains its one-attempt current-Boot custody; this
    /// owner independently consumes its original kernel tickets at most once.
    pub fn start_continuation<E>(
        &mut self,
        scope: &b::FreshContinuationScope<'_>,
        context: ContinuationNativeContext<'_>,
        policy: Policy<'_>,
        resources: &c::ResourceAdmissionOwner,
    ) -> Result<StartedLocalWorkset<T::Bound>, Refusal<E>> {
        let mut target = None;
        self.start_continuation_into(scope, context, policy, resources, &mut target)?;
        Ok(target.expect("successful original continuation Start output"))
    }

    /// The same checked continuation transition with no fat result return.
    /// An occupied output refuses before checking or consuming any owner.
    #[inline(always)]
    pub fn start_continuation_into<E>(
        &mut self,
        scope: &b::FreshContinuationScope<'_>,
        context: ContinuationNativeContext<'_>,
        policy: Policy<'_>,
        resources: &c::ResourceAdmissionOwner,
        target: &mut Option<StartedLocalWorkset<T::Bound>>,
    ) -> Result<(), Refusal<E>> {
        if target.is_some() {
            return Err(Refusal::Scope);
        }
        if self.prepared.is_none() {
            return Err(Refusal::Scope);
        }
        let StartOrigin::Continuation(custody) = &self.origin else {
            return Err(Refusal::Scope);
        };
        context.recheck(policy).map_err(|_| Refusal::Scope)?;
        scope
            .check_actual(context.verified, context.actual)
            .map_err(|_| Refusal::Scope)?;
        if *custody != scope.custody_identity()
            || scope.fragment_count() != F
            || self
                .fragments
                .iter()
                .any(|fragment| !scope.matches_fragment(fragment))
        {
            return Err(Refusal::Scope);
        }
        super::start::check_resources(
            &self.fragments,
            context.actual,
            context.verified.intent().body_id,
            resources,
        )?;
        // SAFETY: the original complete admission and same checked fresh scope
        // succeeded. The affine ticket is taken once, only after every check.
        *target = Some(StartedLocalWorkset {
            custody_identity: *custody,
            kernels: unsafe {
                self.prepared
                    .take()
                    .expect("original unstarted continuation kernels")
                    .bind()
            },
        });
        Ok(())
    }
}
