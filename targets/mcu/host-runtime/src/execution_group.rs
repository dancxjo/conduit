//! Concrete sampling/Mask/output assembly under one authenticated admission.
//! All construction inputs belong to installed Source and actual initialized
//! owners. No component can independently admit or Start an incomplete workset.
extern crate alloc;
use super::{
    execution::{Policy, PreparedBacks, PreparedFragment, Refusal},
    execution_mask::{PreparedMaskRouting, ReadyMaskRouting},
    execution_workset::{PreparedKernelGroup, PreparedLocalKernels, PreparedLocalWorkset},
};
use conduit_body as b;
use conduit_core as c;
use conduit_kernel::{assigned::ResidentLayout, FixedSignLog, HostedValueStore};
use conduit_seven_segment_mask::native_kernel::{
    AssignedMaskInput, AssignedMaskProfile, AssignedMaskScope,
};

/// Cold checked definitions prepared at Root's shallow preparation boundary.
/// Consumed before kernel storage is allocated; these grant no native effects.
pub struct NativeContracts {
    pub append: Option<c::Kind>,
    pub pulse: conduit_pulse_capture::PulseReadContract,
    pub write: conduit_tm1637_wire::WriteContract,
}
impl NativeContracts {
    #[inline(never)]
    pub fn prepare() -> Result<Self, &'static str> {
        Self::prepare_with_recording(false)
    }
    /// Only an independently installed native Source node selects this cold Kind.
    #[inline(never)]
    pub fn prepare_with_recording(recording: bool) -> Result<Self, &'static str> {
        Ok(Self {
            append: recording.then(conduit_resource_snapshot::append_contract::definition),
            pulse: conduit_pulse_capture::PulseReadContract::prepare()
                .map_err(|_| "pulse-contract")?,
            write: conduit_tm1637_wire::WriteContract::prepare().map_err(|_| "write-contract")?,
        })
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EntryPin<'a> {
    pub source: &'a str,
    pub checked: &'a str,
    pub expanded: &'a str,
}
impl EntryPin<'_> {
    fn matches(&self, actual: &b::BodyFragmentApprovalMetadata<'_>) -> bool {
        !self.source.is_empty()
            && !self.checked.is_empty()
            && !self.expanded.is_empty()
            && actual.source == self.source
            && actual.checked_plot == self.checked
            && actual.expanded_plot == self.expanded
    }
    fn check(&self, fragment: &b::DelegatedBodyFragment<'_>) -> Result<(), Refusal<&'static str>> {
        let actual = fragment.metadata();
        if self.source.is_empty()
            || self.checked.is_empty()
            || self.expanded.is_empty()
            || actual.source != self.source
            || actual.checked_plot != self.checked
            || actual.expanded_plot != self.expanded
        {
            return Err(Refusal::Scope);
        }
        Ok(())
    }
}
fn check_configuration(
    approved: &b::DelegatedBodyFragment<'_>,
    expected: &[c::AssignedRecordRequirement],
) -> Result<(), Refusal<&'static str>> {
    let mut pins = expected.iter();
    for record in c::AssignedPlanRecords::new(approved.projection()).map_err(|_| Refusal::Scope)? {
        let record = record.map_err(|_| Refusal::Scope)?;
        if record.tag == c::ASSIGNED_CONFIGURATION
            && pins.next() != Some(&c::AssignedRecordRequirement::from_record(record))
        {
            return Err(Refusal::Scope);
        }
    }
    if pins.next().is_some() {
        return Err(Refusal::Scope);
    }
    Ok(())
}
fn copy_profile<'a>(
    p: &b::AssignedBodyPreparationProfile<'a>,
) -> b::AssignedBodyPreparationProfile<'a> {
    b::AssignedBodyPreparationProfile {
        maxima: p.maxima,
        requirements: p.requirements,
        computations: p.computations,
        fore: p.fore,
    }
}
/// An installed concrete constructor, not a factory decoded from a command.
pub trait FragmentFactory<'p> {
    type Prepared: PreparedLocalKernels;
    fn profile(&self) -> b::AssignedBodyPreparationProfile<'p>;
    fn entry(&self) -> EntryPin<'p>;
    fn prepare(
        self,
        approved: &b::DelegatedBodyFragment<'_>,
        fore: &[c::AssignedForeRequirement],
    ) -> Result<Self::Prepared, Refusal<&'static str>>;
}
/// Actual prepared software/resident/native loans plus acquired canonical slots.
/// External stores are counted from their real reservation, not live occupancy.
pub struct FragmentConstruction<
    'p,
    B: PreparedBacks<N, P>,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
    const S: usize,
> {
    pub entry: EntryPin<'p>,
    pub configuration: &'p [c::AssignedRecordRequirement],
    pub profile: b::AssignedBodyPreparationProfile<'p>,
    pub layout: ResidentLayout<'p>,
    pub values: HostedValueStore,
    pub signs: FixedSignLog<S>,
    pub backs: B,
    pub maximum_retained_bytes: usize,
}
impl<
        'p,
        B: PreparedBacks<N, P>,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const S: usize,
    > FragmentFactory<'p> for FragmentConstruction<'p, B, N, C, P, R, T, H, S>
{
    type Prepared = PreparedFragment<B, N, C, P, R, T, H, S>;
    fn profile(&self) -> b::AssignedBodyPreparationProfile<'p> {
        copy_profile(&self.profile)
    }
    fn entry(&self) -> EntryPin<'p> {
        self.entry
    }
    fn prepare(
        self,
        approved: &b::DelegatedBodyFragment<'_>,
        fore: &[c::AssignedForeRequirement],
    ) -> Result<Self::Prepared, Refusal<&'static str>> {
        self.entry.check(approved)?;
        check_configuration(approved, self.configuration)?;
        PreparedFragment::prepare(
            approved,
            self.profile,
            fore,
            &self.layout,
            self.values,
            self.signs,
            self.maximum_retained_bytes,
            || Ok(self.backs),
        )
    }
}
pub struct MaskConstruction<'o, 'p> {
    pub entry: EntryPin<'p>,
    pub configuration: &'p [c::AssignedRecordRequirement],
    pub profile: b::AssignedBodyPreparationProfile<'p>,
    pub layout: AssignedMaskProfile<'p>,
    pub owner: &'o mut ReadyMaskRouting,
}
impl<'o, 'p> FragmentFactory<'p> for MaskConstruction<'o, 'p> {
    type Prepared = PreparedMaskRouting<'o>;
    fn profile(&self) -> b::AssignedBodyPreparationProfile<'p> {
        copy_profile(&self.profile)
    }
    fn entry(&self) -> EntryPin<'p> {
        self.entry
    }
    fn prepare(
        self,
        approved: &b::DelegatedBodyFragment<'_>,
        fore: &[c::AssignedForeRequirement],
    ) -> Result<Self::Prepared, Refusal<&'static str>> {
        self.entry.check(approved)?;
        check_configuration(approved, self.configuration)?;
        if fore != self.layout.fore {
            return Err(Refusal::Scope);
        }
        let bytes = approved.projection();
        let header = bytes
            .get(..c::ASSIGNED_PLAN_HEADER_BYTES)
            .ok_or(Refusal::Scope)?;
        // Fixed small Mask shape. Borrowed records live only through table
        // preparation; no projection copies or decoded offers remain on MCU.
        let mut records = [c::AssignedRecord {
            tag: 0,
            payload: &[],
        }; 64];
        let mut length = 0;
        for record in c::AssignedPlanRecords::new(bytes).map_err(|_| Refusal::Scope)? {
            let slot = records.get_mut(length).ok_or(Refusal::Bounds)?;
            *slot = record.map_err(|_| Refusal::Scope)?;
            length += 1;
        }
        // This scope comes from the opaque, already authenticated Root approval,
        // not from an unauthenticated received descriptor or numeric offer.
        let metadata = approved.metadata();
        let scope = AssignedMaskScope {
            host: c::AssignedIdentity::from_text(metadata.host),
            boot: c::AssignedIdentity::from_text(metadata.boot),
            plan: c::AssignedIdentity::from_text(metadata.plot_plan),
            fragment: c::AssignedIdentity::from_text(metadata.fragment),
        };
        self.owner
            .prepare(
                AssignedMaskInput {
                    header,
                    records: &records[..length],
                    maxima: self.profile.maxima,
                },
                &self.layout,
                scope,
            )
            .map_err(Refusal::Mask)
    }
}
pub type PreparedGroup<'w, S, M, O> = PreparedLocalWorkset<'w, PreparedKernelGroup<S, M, O>, 3>;
/// Caller retains the original ledger and performs the existing `.admit` and
/// `.start` transitions. This function cannot cause an electrical effect.
#[allow(clippy::type_complexity)] // Preserve each fragment factory's concrete affine prepared type.
pub fn prepare<'w, 'p, S, M, O, const INPUT: usize, const PORTS: usize>(
    workspace: &'w b::HostCommandInput<INPUT>,
    context: b::NativePreparationContext<'_>,
    policy: Policy<'_>,
    sampling: S,
    mask: M,
    output: O,
) -> Result<PreparedGroup<'w, S::Prepared, M::Prepared, O::Prepared>, Refusal<Refusal<&'static str>>>
where
    S: FragmentFactory<'p>,
    M: FragmentFactory<'p>,
    O: FragmentFactory<'p>,
{
    let pins = [sampling.entry(), mask.entry(), output.entry()];
    if pins[0] == pins[1] || pins[0] == pins[2] || pins[1] == pins[2] {
        return Err(Refusal::Scope);
    }
    let mut profiles = [
        Some(sampling.profile()),
        Some(mask.profile()),
        Some(output.profile()),
    ];
    PreparedLocalWorkset::prepare_selected::<INPUT, PORTS, _>(
        workspace,
        context,
        policy,
        move |_, document| {
            let (metadata, _) =
                b::decode_body_fragment_document(document).map_err(|_| Refusal::Scope)?;
            let index = pins
                .iter()
                .position(|pin| pin.matches(&metadata))
                .ok_or(Refusal::Scope)?;
            profiles[index].take().ok_or(Refusal::Scope)
        },
        move |fragments, fore| {
            let find = |pin: EntryPin<'_>| {
                fragments
                    .iter()
                    .position(|f| pin.matches(f.metadata()))
                    .ok_or(Refusal::Scope)
            };
            let s = find(pins[0])?;
            let m = find(pins[1])?;
            let o = find(pins[2])?;
            let sampling = sampling.prepare(&fragments[s], fore[s].as_slice())?;
            let mask = mask.prepare(&fragments[m], fore[m].as_slice())?;
            let output = output.prepare(&fragments[o], fore[o].as_slice())?;
            Ok(PreparedKernelGroup(sampling, mask, output))
        },
    )
}

/// Installed storage/topology facts before approved fragments are available.
pub struct FragmentMaterial<'p, const SIGNS: usize> {
    pub entry: EntryPin<'p>,
    pub configuration: &'p [c::AssignedRecordRequirement],
    pub profile: b::AssignedBodyPreparationProfile<'p>,
    pub layout: ResidentLayout<'p>,
    pub values: HostedValueStore,
    pub signs: FixedSignLog<SIGNS>,
    pub maximum_retained_bytes: usize,
}
/// Native/resident selection needs the actual opaque approval. This adapter
/// invokes the installed constructor only inside whole-workset preparation,
/// after checking exact Source identity and independent kernel layout.
pub struct SourceFragmentFactory<
    'p,
    F,
    B: PreparedBacks<N, P>,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
    const S: usize,
> {
    material: FragmentMaterial<'p, S>,
    factory: F,
    marker: core::marker::PhantomData<fn() -> B>,
}
impl<
        'p,
        F,
        B: PreparedBacks<N, P>,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const S: usize,
    > SourceFragmentFactory<'p, F, B, N, C, P, R, T, H, S>
{
    pub fn new(material: FragmentMaterial<'p, S>, factory: F) -> Self {
        Self {
            material,
            factory,
            marker: core::marker::PhantomData,
        }
    }
}
impl<
        'p,
        F,
        B: PreparedBacks<N, P>,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const S: usize,
    > FragmentFactory<'p> for SourceFragmentFactory<'p, F, B, N, C, P, R, T, H, S>
where
    F: FnOnce(
        &b::DelegatedBodyFragment<'_>,
        &[c::AssignedForeRequirement],
    ) -> Result<B, &'static str>,
{
    type Prepared = PreparedFragment<B, N, C, P, R, T, H, S>;
    fn profile(&self) -> b::AssignedBodyPreparationProfile<'p> {
        copy_profile(&self.material.profile)
    }
    fn entry(&self) -> EntryPin<'p> {
        self.material.entry
    }
    fn prepare(
        self,
        approved: &b::DelegatedBodyFragment<'_>,
        fore: &[c::AssignedForeRequirement],
    ) -> Result<Self::Prepared, Refusal<&'static str>> {
        let material = self.material;
        material.entry.check(approved)?;
        check_configuration(approved, material.configuration)?;
        PreparedFragment::prepare(
            approved,
            material.profile,
            fore,
            &material.layout,
            material.values,
            material.signs,
            material.maximum_retained_bytes,
            || (self.factory)(approved, fore),
        )
    }
}

/// Grouped Source construction for inventories whose initialized entries must
/// be checked together and then split into disjoint fragment loans. Signed
/// fragment order is retained; callback roles are independent installed pins.
#[allow(clippy::too_many_arguments)]
#[inline(always)] // Avoid a second complete prepared-group return frame.
pub fn prepare_with_source<'w, 'p, T, E, const INPUT: usize, const PORTS: usize>(
    workspace: &'w b::HostCommandInput<INPUT>,
    context: b::NativePreparationContext<'_>,
    policy: Policy<'_>,
    pins: [EntryPin<'p>; 3],
    configurations: [&'p [c::AssignedRecordRequirement]; 3],
    profiles: [b::AssignedBodyPreparationProfile<'p>; 3],
    factory: impl FnOnce(
        [&b::DelegatedBodyFragment<'w>; 3],
        [&[c::AssignedForeRequirement]; 3],
    ) -> Result<T, E>,
) -> Result<PreparedLocalWorkset<'w, T, 3>, Refusal<Refusal<E>>>
where
    T: PreparedLocalKernels,
{
    if pins[0] == pins[1] || pins[0] == pins[2] || pins[1] == pins[2] {
        return Err(Refusal::Scope);
    }
    let mut profiles = profiles.map(Some);
    PreparedLocalWorkset::prepare_selected::<INPUT, PORTS, _>(
        workspace,
        context,
        policy,
        move |_, document| {
            let (metadata, _) =
                b::decode_body_fragment_document(document).map_err(|_| Refusal::Scope)?;
            let role = pins
                .iter()
                .position(|p| p.matches(&metadata))
                .ok_or(Refusal::Scope)?;
            profiles[role].take().ok_or(Refusal::Scope)
        },
        // Fuse this adapter's affine return storage with its caller instead of
        // retaining another complete prepared-group copy on the MCU stack.
        #[inline(always)]
        move |fragments, fore| {
            let mut indices = [0usize; 3];
            for (role, pin) in pins.iter().enumerate() {
                let index = fragments
                    .iter()
                    .position(|f| pin.matches(f.metadata()))
                    .ok_or(Refusal::Scope)?;
                // Every scope and immutable configuration is checked before a
                // single initialized owner may be borrowed by the callback.
                check_configuration(&fragments[index], configurations[role]).map_err(
                    |e| match e {
                        Refusal::Scope => Refusal::Scope,
                        _ => Refusal::Bounds,
                    },
                )?;
                indices[role] = index;
            }
            factory(
                indices.map(|i| &fragments[i]),
                indices.map(|i| fore[i].as_slice()),
            )
            .map_err(Refusal::Back)
        },
    )
}

#[path = "execution_group_continuation.rs"]
mod continuation;
pub use continuation::prepare_continuation_with_source;
