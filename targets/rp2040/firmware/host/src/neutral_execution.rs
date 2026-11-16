//! Original whole-workset admission with caller-installed Fore services.
//! This creates no input, frame policy, carrier, scheduler or autonomous Start.
use crate::{
    neutral_authority::{Issuers, Operations},
    neutral_equipment::Equipment,
    neutral_factory::{self, Bound, Kernel},
    neutral_workset::{layout, N},
};
use conduit_body as b;
use conduit_core as c;
use conduit_i2c_transaction::{contract::I2C_RESULT_VALUE_BYTES, I2cProvider};
use conduit_mcu_host_runtime::{
    execution::{Policy, Refusal},
    execution_factory::SoftwareInventory,
    execution_workset::{CommittedLocalWorkset, PreparedLocalKernels, StartedLocalWorkset},
    native_runner::Observer,
    root::Configuration,
};
use conduit_monotonic_wait::clock_at::owner::MonotonicDeadlineProvider;

/// Disjoint original Root loans. No advertisement copy or received policy.
#[derive(Clone, Copy)]
pub struct RootScope<'w, 'r, 's> {
    pub workspace: &'w b::HostCommandInput<{ crate::config::MAX_ASSIGNMENT_BYTES }>,
    pub bootstrap: &'r b::MemberBootstrap<'s>,
    pub member: &'r b::MembershipCredential,
    pub configuration: &'r Configuration,
    pub now_millis: fn() -> u64,
}
impl RootScope<'_, '_, '_> {
    fn context<'a>(&'a self, actual: &'a c::HostAdvertisement) -> b::NativePreparationContext<'a> {
        b::NativePreparationContext {
            bootstrap: self.bootstrap,
            member: self.member,
            actual,
            image_id: self.configuration.image_id,
            configuration_id: self.configuration.configuration_id,
            now_millis: (self.now_millis)(),
        }
    }
    fn policy(&self, active_worksets: u16, maximum: usize) -> Policy<'_> {
        Policy {
            preparation: self.configuration.preparation,
            execution: self.configuration.execution,
            active_worksets,
            maximum_retained_bytes: maximum,
        }
    }
}
/// Caller-installed typed observer and actual native result storage. Inventory
/// equality checks ABI identity only: it does not prove an arbitrary Observer
/// decodes/delivers values, nor authorize a carrier. The Observer is trusted
/// installed composition, never received metadata. Its existing owner report
/// must cover its complete retained backing; unknown stock refuses cold.
pub struct PreparedFore<'a, O> {
    observer: O,
    output: &'a mut [u8],
}
pub struct BoundFore<'a, O> {
    observer: O,
    output: &'a mut [u8],
}
impl<O> BoundFore<'_, O> {
    pub fn parts_mut(&mut self) -> (&mut O, &mut [u8]) {
        (&mut self.observer, self.output)
    }
}
impl<'a, O: PreparedLocalKernels> PreparedFore<'a, O> {
    pub fn new(
        observer: O,
        pins: &[c::AssignedForeRequirement],
        output: &'a mut [u8],
    ) -> Result<Self, Refusal<&'static str>> {
        if output.len() < I2C_RESULT_VALUE_BYTES {
            return Err(Refusal::Back("native-result-buffer"));
        }
        let expected = layout::fore();
        if pins.len() != expected.len()
            || expected
                .iter()
                .any(|pin| pins.iter().filter(|p| *p == pin).count() != 1)
        {
            return Err(Refusal::Back("complete-typed-fore-required"));
        }
        let ticket = Self { observer, output };
        ticket.retained_backing_bytes().ok_or(Refusal::Bounds)?;
        Ok(ticket)
    }
}
// SAFETY: the constructor checks the complete installed ABI inventory and result
// span. The original observer ticket owns its prepared services. Binding only
// transfers those original owners/loans after enclosing authenticated Start.
unsafe impl<'a, O: PreparedLocalKernels> PreparedLocalKernels for PreparedFore<'a, O> {
    type Bound = BoundFore<'a, O::Bound>;
    fn retained_backing_bytes(&self) -> Option<usize> {
        let prepared_overhead =
            core::mem::size_of::<Self>().checked_sub(core::mem::size_of::<O>())?;
        let bound_overhead =
            core::mem::size_of::<Self::Bound>().checked_sub(core::mem::size_of::<O::Bound>())?;
        self.observer
            .retained_backing_bytes()?
            .checked_add(prepared_overhead.max(bound_overhead))?
            .checked_add(self.output.len())
    }
    unsafe fn bind(self) -> Self::Bound {
        // SAFETY: delegated by the same original complete-workset Start.
        BoundFore {
            observer: unsafe { self.observer.bind() },
            output: self.output,
        }
    }
}
/// Prepare and admit the exact component with its actual external service
/// backing. Providers stay unbound until the returned original owner Starts.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn admit<
    'a,
    'f: 'a,
    'w,
    I: I2cProvider + 'a,
    D: MonotonicDeadlineProvider + 'a,
    O: PreparedLocalKernels + 'a,
>(
    scope: RootScope<'w, '_, '_>,
    resources: &mut c::ResourceAdmissionOwner,
    equipment: &'a mut Equipment<I, D>,
    residents: &'a mut crate::resident::generated::Initialized,
    software: &'a mut SoftwareInventory<'f, N>,
    issuers: &mut Issuers,
    operations: &Operations,
    fore: PreparedFore<'a, O>,
    observations: &[c::ResourceObservation],
    observation_backing_bytes: usize,
    active_worksets: u16,
    maximum: usize,
) -> Result<
    CommittedLocalWorkset<
        'w,
        impl PreparedLocalKernels<Bound = (Bound<'a, 'f, I, D>, BoundFore<'a, O::Bound>)> + 'a,
        1,
    >,
    Refusal<&'static str>,
>
where
    O::Bound: Observer<Kernel<'a, 'f>> + 'a,
{
    let prepared = neutral_factory::prepare(
        scope.workspace,
        scope.context(resources.host()),
        scope.policy(active_worksets, maximum),
        residents,
        software,
        &mut equipment.bus,
        &mut equipment.clock,
        &mut issuers.bus,
        &mut issuers.clock,
        operations,
    )?;
    prepared.with_services::<_, &'static str>(fore, 0)?.admit(
        resources,
        observations,
        observation_backing_bytes,
    )
}
/// Delegate Start to the original committed owner. Repeated Start retains its
/// ordinary refusal; no wrapper flag substitutes for the authenticated checks.
pub fn start<T: PreparedLocalKernels>(
    committed: &mut CommittedLocalWorkset<'_, T, 1>,
    scope: RootScope<'_, '_, '_>,
    resources: &c::ResourceAdmissionOwner,
    active_worksets: u16,
    maximum: usize,
) -> Result<StartedLocalWorkset<T::Bound>, Refusal<&'static str>> {
    committed.start(
        scope.workspace,
        scope.context(resources.host()),
        scope.policy(active_worksets, maximum),
        resources,
    )
}
