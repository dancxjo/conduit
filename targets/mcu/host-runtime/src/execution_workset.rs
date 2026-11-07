//! One authenticated, atomic admission boundary for heterogeneous local kernels.
//! Each factory prepares the real checked fragment; no fragment binds or starts
//! its providers before the complete set has passed Core's resource admission.
extern crate alloc;
use crate::execution::{Policy, Refusal};
use alloc::vec::Vec;
use conduit_body as b;
use conduit_core as c;

/// Prepared concrete kernels and their actual exclusive provider loans.
/// # Safety
/// Implementations must validate every approved fragment against independently
/// installed Source/configuration, own its complete prepared execution, and bind
/// infallibly without allocations, effects or Start. Dropping returns all loans.
pub unsafe trait PreparedLocalKernels {
    type Bound;
    fn retained_backing_bytes(&self) -> Option<usize>;
    /// # Safety
    /// Original complete resource admission and current authenticated Start
    /// must have succeeded before any initialized provider is withdrawn.
    unsafe fn bind(self) -> Self::Bound;
}
pub struct PreparedLocalWorkset<'w, T, const F: usize> {
    fragments: [b::DelegatedBodyFragment<'w>; F],
    stamp: b::RetainedHostCommandStamp,
    prepared: T,
    maximum: usize,
    retained: usize,
}
pub struct CommittedLocalWorkset<'w, T, const F: usize> {
    fragments: [b::DelegatedBodyFragment<'w>; F],
    stamp: b::RetainedHostCommandStamp,
    prepared: T,
}
pub struct StartedLocalWorkset<T>(T);
impl<T> StartedLocalWorkset<T> {
    pub fn kernels_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
impl<'w, T: PreparedLocalKernels, const F: usize> PreparedLocalWorkset<'w, T, F> {
    /// Profile order is installed Root material. The signed set must contain
    /// exactly that many fragments; no unchecked subset can become Playing.
    pub fn prepare<const I: usize, const PORTS: usize, E>(
        workspace: &'w b::HostCommandInput<I>,
        context: b::NativePreparationContext<'_>,
        policy: Policy<'_>,
        profiles: [b::AssignedBodyPreparationProfile<'_>; F],
        factory: impl FnOnce(
            &[b::DelegatedBodyFragment<'w>; F],
            &[b::ApprovedBodyForePorts<PORTS>; F],
        ) -> Result<T, E>,
    ) -> Result<Self, Refusal<E>> {
        let mut profiles = profiles.into_iter();
        Self::prepare_selected::<I, PORTS, E>(
            workspace,
            context,
            policy,
            move |_, _| Ok(profiles.next().expect("exact finite profile count")),
            factory,
        )
    }
    /// Select an independently installed profile from an authenticated set's
    /// public child metadata. Selection is not admission; every selected child
    /// is still checked below against that profile and the original command.
    pub fn prepare_selected<'p, const I: usize, const PORTS: usize, E>(
        workspace: &'w b::HostCommandInput<I>,
        context: b::NativePreparationContext<'_>,
        policy: Policy<'_>,
        mut profile_for: impl FnMut(usize, &[u8]) -> Result<b::AssignedBodyPreparationProfile<'p>, E>,
        factory: impl FnOnce(
            &[b::DelegatedBodyFragment<'w>; F],
            &[b::ApprovedBodyForePorts<PORTS>; F],
        ) -> Result<T, E>,
    ) -> Result<Self, Refusal<E>> {
        if F == 0 {
            return Err(Refusal::Bounds);
        }
        let command = workspace
            .reborrow(
                context.member,
                context.actual,
                &context.bootstrap.owner,
                b::HOST_PREPARED_WORKSET_CONTRACT,
                context.now_millis,
            )
            .map_err(Refusal::Retention)?;
        let set = b::receive_delegated_body_workset::<F>(policy.preparation, context, command)
            .map_err(Refusal::Workset)?;
        set.check_execution_delegation(
            policy.execution,
            policy.preparation,
            context,
            policy.active_worksets,
        )
        .map_err(Refusal::Execution)?;
        if set.fragment_count() != F {
            return Err(Refusal::Scope);
        }
        // Finite temporary preparation storage, independently funded by Root.
        let mut fragments = Vec::with_capacity(F);
        let mut ports = Vec::with_capacity(F);
        for index in 0..F {
            let profile = profile_for(index, set.document(index).ok_or(Refusal::Scope)?)
                .map_err(Refusal::Back)?;
            let (fragment, fore) = set
                .receive_fragment_with_approved_fore::<PORTS>(
                    index,
                    policy.preparation,
                    context,
                    profile,
                )
                .map_err(Refusal::Workset)?;
            fragments.push(fragment);
            ports.push(fore);
        }
        let fragments: [_; F] = fragments.try_into().map_err(|_| Refusal::Bounds)?;
        let ports: [_; F] = ports.try_into().map_err(|_| Refusal::Bounds)?;
        let prepared = factory(&fragments, &ports).map_err(Refusal::Back)?;
        let retained = prepared.retained_backing_bytes().ok_or(Refusal::Bounds)?;
        if retained > policy.maximum_retained_bytes {
            return Err(Refusal::Bounds);
        }
        let stamp = set.into_retained_stamp().map_err(Refusal::Retention)?;
        Ok(Self {
            fragments,
            stamp,
            prepared,
            maximum: policy.maximum_retained_bytes,
            retained,
        })
    }
    /// The original ledger admits all fragments in one batch. Any refusal leaves
    /// native owners in their Ready slots and the ledger unchanged.
    pub fn admit<E>(
        self,
        owner: &mut c::ResourceAdmissionOwner,
        observations: &[c::ResourceObservation],
        observation_backing_bytes: usize,
    ) -> Result<CommittedLocalWorkset<'w, T, F>, Refusal<E>> {
        let fragments = core::array::from_fn::<_, F, _>(|i| &self.fragments[i]);
        let bridge = b::delegated_plain_resource_preparation_bytes(&fragments, owner, observations)
            .map_err(Refusal::Resources)?;
        if self
            .retained
            .checked_add(bridge)
            .and_then(|n| n.checked_add(observation_backing_bytes))
            .is_none_or(|n| n > self.maximum)
        {
            return Err(Refusal::Bounds);
        }
        b::admit_delegated_plain_resources(&fragments, owner, observations)
            .map_err(Refusal::Resources)?;
        Ok(CommittedLocalWorkset {
            fragments: self.fragments,
            stamp: self.stamp,
            prepared: self.prepared,
        })
    }
}
impl<T: PreparedLocalKernels, const F: usize> CommittedLocalWorkset<'_, T, F> {
    /// Original approved scopes needed to abort this unstarted reservation via
    /// Core and retain its release evidence. Providers remain unbound on refusal.
    pub fn fragments(&self) -> &[b::DelegatedBodyFragment<'_>; F] {
        &self.fragments
    }

    /// Recheck the unchanged original signed command and complete live ledger at
    /// the actual Start transition. Transport acknowledgement grants no Start.
    #[allow(clippy::result_large_err)]
    pub fn start<const I: usize, E>(
        self,
        workspace: &b::HostCommandInput<I>,
        context: b::NativePreparationContext<'_>,
        policy: Policy<'_>,
        resources: &c::ResourceAdmissionOwner,
    ) -> Result<StartedLocalWorkset<T::Bound>, (Self, Refusal<E>)> {
        let check = (|| {
            let command = workspace
                .reborrow(
                    context.member,
                    context.actual,
                    &context.bootstrap.owner,
                    b::HOST_PREPARED_WORKSET_CONTRACT,
                    context.now_millis,
                )
                .map_err(Refusal::Retention)?;
            self.stamp
                .check_command(&command)
                .map_err(Refusal::Retention)?;
            let set = b::receive_delegated_body_workset::<F>(policy.preparation, context, command)
                .map_err(Refusal::Workset)?;
            set.check_execution_delegation(
                policy.execution,
                policy.preparation,
                context,
                policy.active_worksets,
            )
            .map_err(Refusal::Execution)?;
            if set.fragment_count() != F || resources.host() != context.actual {
                return Err(Refusal::Scope);
            }
            let mut entries = 0usize;
            for fragment in &self.fragments {
                let m = fragment.metadata();
                if m.body != context.member.body_id.as_str()
                    || m.host != context.actual.host_id.as_str()
                    || m.boot != context.actual.boot_id.as_str()
                    || m.offer_generation != context.actual.offer_generation.0
                {
                    return Err(Refusal::Scope);
                }
                for item in b::PlainFragmentResources::new(m.resource_bindings) {
                    let item = item.map_err(|_| Refusal::Scope)?;
                    entries += 1;
                    let matches = resources
                        .admissions()
                        .iter()
                        .filter(|a| {
                            a.plan_id.as_str() == m.body_plan
                                && a.placement_id.as_str() == item.placement
                                && a.host_id.as_str() == m.host
                                && a.boot_id.as_str() == m.boot
                                && a.offer_generation.0 == m.offer_generation
                        })
                        .flat_map(|a| &a.items)
                        .filter(|held| {
                            held.binding.pool_id.as_str() == item.pool
                                && held.binding.class_id.as_str() == item.class
                                && held.binding.units == item.units
                                && held.requirement.class_id.as_str() == item.class
                                && held.requirement.units == item.units
                        })
                        .count();
                    if matches != 1 {
                        return Err(Refusal::Scope);
                    }
                }
            }
            if resources
                .admissions()
                .iter()
                .map(|a| a.items.len())
                .sum::<usize>()
                != entries
            {
                return Err(Refusal::Scope);
            }
            Ok(())
        })();
        match check {
            Ok(()) => {
                // SAFETY: original whole-workset admission plus all current
                // authenticated Start checks above succeeded atomically.
                Ok(StartedLocalWorkset(unsafe { self.prepared.bind() }))
            }
            Err(e) => Err((self, e)),
        }
    }
}

/// Different finite kernel shapes retain their own original Plan scopes while
/// sharing one complete-set admission and Start transition.
pub struct PreparedKernelGroup<A, B, C>(pub A, pub B, pub C);
// SAFETY: component tickets already provide infallible effect-free binds. The
// group adds checked accounting and transfers each exactly once after admission.
unsafe impl<A: PreparedLocalKernels, B: PreparedLocalKernels, C: PreparedLocalKernels>
    PreparedLocalKernels for PreparedKernelGroup<A, B, C>
{
    type Bound = (A::Bound, B::Bound, C::Bound);
    fn retained_backing_bytes(&self) -> Option<usize> {
        self.0
            .retained_backing_bytes()?
            .checked_add(self.1.retained_backing_bytes()?)?
            .checked_add(self.2.retained_backing_bytes()?)
    }
    unsafe fn bind(self) -> Self::Bound {
        // SAFETY: caller supplies the same complete authenticated Start for
        // every fragment; these are disjoint original prepared owner loans.
        unsafe { (self.0.bind(), self.1.bind(), self.2.bind()) }
    }
}

// SAFETY: the Mask ticket retains the original initialized routing Backs and
// already preflights the exact assigned Mask storage. Workset Start is the only
// caller of bind, after original complete resource admission and scope checks.
unsafe impl PreparedLocalKernels for super::execution_mask::PreparedMaskRouting<'_> {
    type Bound = conduit_seven_segment_mask::native_kernel::PreparedAssignedMaskKernel;
    fn retained_backing_bytes(&self) -> Option<usize> {
        super::execution_mask::PreparedMaskRouting::retained_backing_bytes(self)
    }
    unsafe fn bind(self) -> Self::Bound {
        // SAFETY: this trait is consumed only by authenticated whole-workset
        // Start; no provider can bind independently through this API.
        unsafe { super::execution_mask::PreparedMaskRouting::bind(self) }
    }
}

/// Additional original services prepared for the same workset. Private fields
/// prevent polling those services through this owner before authenticated Start.
pub struct PreparedServices<K, S> {
    kernels: K,
    services: S,
    shared_backing_bytes: usize,
}
// SAFETY: component tickets own disjoint initialized providers and their bind
// contracts already require effect-free, infallible transfer after Start.
unsafe impl<K: PreparedLocalKernels, S: PreparedLocalKernels> PreparedLocalKernels
    for PreparedServices<K, S>
{
    type Bound = (K::Bound, S::Bound);
    fn retained_backing_bytes(&self) -> Option<usize> {
        self.kernels
            .retained_backing_bytes()?
            .checked_add(self.services.retained_backing_bytes()?)?
            .checked_add(self.shared_backing_bytes)
    }
    unsafe fn bind(self) -> Self::Bound {
        // SAFETY: the enclosing complete original workset authenticated Start;
        // both owned tickets retain their exact initialized providers.
        unsafe { (self.kernels.bind(), self.services.bind()) }
    }
}
impl<'w, T: PreparedLocalKernels, const F: usize> PreparedLocalWorkset<'w, T, F> {
    /// Opaque child approvals for original native/Face service preparation.
    /// This exposes no independent admission or permission to execute a child.
    pub fn fragments(&self) -> &[b::DelegatedBodyFragment<'w>; F] {
        &self.fragments
    }
    /// Include actual services and externally retained, separately owned shared
    /// storage before any resource admission. The original retained-byte ceiling
    /// remains unchanged. `shared_backing_bytes` is accounting, never a grant.
    pub fn with_services<S: PreparedLocalKernels, E>(
        self,
        services: S,
        shared_backing_bytes: usize,
    ) -> Result<PreparedLocalWorkset<'w, PreparedServices<T, S>, F>, Refusal<E>> {
        let prepared = PreparedServices {
            kernels: self.prepared,
            services,
            shared_backing_bytes,
        };
        let retained = prepared.retained_backing_bytes().ok_or(Refusal::Bounds)?;
        if retained > self.maximum {
            return Err(Refusal::Bounds);
        }
        Ok(PreparedLocalWorkset {
            fragments: self.fragments,
            stamp: self.stamp,
            prepared,
            maximum: self.maximum,
            retained,
        })
    }
}
