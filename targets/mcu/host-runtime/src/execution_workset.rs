//! One authenticated, atomic admission boundary for heterogeneous local kernels.
//! Each factory prepares the real checked fragment; no fragment binds or starts
//! its providers before the complete set has passed Core's resource admission.
extern crate alloc;
use crate::execution::{Policy, Refusal};
use alloc::boxed::Box;
#[path = "execution_workset_continuation.rs"]
mod continuation;
#[path = "execution_workset_receiving.rs"]
mod receiving;
#[path = "execution_workset_start.rs"]
mod start;

// Authority origins are intentionally disjoint; a continuation never invents a
// current-Boot online command stamp or membership acknowledgment.
enum StartOrigin {
    Online(b::RetainedHostCommandStamp),
    Continuation([u8; 32]),
}
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
    fragments: Box<[b::DelegatedBodyFragment<'w>; F]>,
    origin: StartOrigin,
    prepared: T,
    maximum: usize,
    retained: usize,
}
pub struct CommittedLocalWorkset<'w, T, const F: usize> {
    fragments: Box<[b::DelegatedBodyFragment<'w>; F]>,
    origin: StartOrigin,
    prepared: Option<T>,
}
/// Read-only causal evidence produced only by successful authenticated Start.
/// This identity does not grant execution or allow another kernel to Start.
#[derive(Clone, Copy)]
pub struct StartedEvidence {
    custody_identity: [u8; 32],
}
impl StartedEvidence {
    pub fn custody_identity(&self) -> &[u8; 32] {
        &self.custody_identity
    }
}
pub struct StartedLocalWorkset<T> {
    kernels: T,
    custody_identity: [u8; 32],
}
impl<T> StartedLocalWorkset<T> {
    /// Original authenticated attempt retained through successful Start.
    /// Inspection of this identity cannot confer authority on another attempt.
    pub fn custody_identity(&self) -> &[u8; 32] {
        &self.custody_identity
    }

    /// Preserve causal evidence while transferring affine kernels to sessions.
    pub fn evidence(&self) -> StartedEvidence {
        StartedEvidence {
            custody_identity: self.custody_identity,
        }
    }
    pub fn kernels_mut(&mut self) -> &mut T {
        &mut self.kernels
    }
    /// Transfer the already Started affine owners into their ordinary execution
    /// sessions. This exposes no preparation or independent Start transition.
    pub fn into_kernels(self) -> T {
        self.kernels
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
    #[inline(always)] // Fuse affine return storage with the installed factory.
    pub fn prepare_selected<'p, const I: usize, const PORTS: usize, E>(
        workspace: &'w b::HostCommandInput<I>,
        context: b::NativePreparationContext<'_>,
        policy: Policy<'_>,
        profile_for: impl FnMut(usize, &[u8]) -> Result<b::AssignedBodyPreparationProfile<'p>, E>,
        factory: impl FnOnce(
            &[b::DelegatedBodyFragment<'w>; F],
            &[b::ApprovedBodyForePorts<PORTS>; F],
        ) -> Result<T, E>,
    ) -> Result<Self, Refusal<E>> {
        let (fragments, ports, stamp) =
            receiving::receive::<I, F, PORTS, E>(workspace, context, policy, profile_for)?;
        let prepared = factory(&fragments, &ports).map_err(Refusal::Back)?;
        let retained = prepared
            .retained_backing_bytes()
            .and_then(|bytes| {
                bytes.checked_add(core::mem::size_of::<[b::DelegatedBodyFragment<'w>; F]>())
            })
            .ok_or(Refusal::Bounds)?;
        if retained > policy.maximum_retained_bytes {
            return Err(Refusal::Capacity {
                required: retained,
                maximum: policy.maximum_retained_bytes,
            });
        }
        Ok(Self {
            fragments,
            origin: StartOrigin::Online(stamp),
            prepared,
            maximum: policy.maximum_retained_bytes,
            retained,
        })
    }
    /// The original ledger admits all fragments in one batch. Any refusal leaves
    /// native owners in their Ready slots and the ledger unchanged.
    #[inline(always)]
    pub fn admit<E>(
        self,
        owner: &mut c::ResourceAdmissionOwner,
        observations: &[c::ResourceObservation],
        observation_backing_bytes: usize,
    ) -> Result<CommittedLocalWorkset<'w, T, F>, Refusal<E>> {
        let fragments = core::array::from_fn::<_, F, _>(|i| &self.fragments[i]);
        let bridge = b::delegated_resource_preparation_bytes(&fragments, owner, observations)
            .map_err(Refusal::Resources)?;
        let required = self
            .retained
            .checked_add(bridge)
            .and_then(|n| n.checked_add(observation_backing_bytes))
            .ok_or(Refusal::Bounds)?;
        if required > self.maximum {
            return Err(Refusal::Capacity {
                required,
                maximum: self.maximum,
            });
        }
        b::admit_delegated_resources(&fragments, owner, observations)
            .map_err(Refusal::Resources)?;
        Ok(CommittedLocalWorkset {
            fragments: self.fragments,
            origin: self.origin,
            prepared: Some(self.prepared),
        })
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
    #[inline(always)]
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
        let retained = prepared
            .retained_backing_bytes()
            .and_then(|bytes| {
                bytes.checked_add(core::mem::size_of::<[b::DelegatedBodyFragment<'w>; F]>())
            })
            .ok_or(Refusal::Bounds)?;
        if retained > self.maximum {
            return Err(Refusal::Capacity {
                required: retained,
                maximum: self.maximum,
            });
        }
        Ok(PreparedLocalWorkset {
            fragments: self.fragments,
            origin: self.origin,
            prepared,
            maximum: self.maximum,
            retained,
        })
    }
}
