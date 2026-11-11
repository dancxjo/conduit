//! One checked two-operation service occupying one original inventory slot.
use super::*;
use alloc::boxed::Box;
use conduit_kernel::scheduler::AssignedTerminalTransduction;
use conduit_resource_snapshot::{
    native_append::service_back::RecordServiceBack,
    native_service::{PreparedAssignedRecordService, RecordServiceHostCall},
    nor::NorStorage,
};
/// Indirection is selected only by this service inventory. Its complete inline
/// Back allocation is charged before admission, never every old Driver array.
pub struct BoxedRecordServiceBack<const PORTS: usize, const READ_BYTES: usize>(
    Box<RecordServiceBack<PORTS, READ_BYTES>>,
);
impl<const P: usize, const R: usize> StepBack<P> for BoxedRecordServiceBack<P, R> {
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; P] {
        self.0.terminal_transductions()
    }
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        self.0.step(io, input)
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        self.0.prepared_output(port)
    }
    fn step_committed(&mut self) {
        self.0.step_committed()
    }
    fn cancel(&mut self) {
        self.0.cancel()
    }
}
pub struct CallsWithRecordService<B, S> {
    pub base: B,
    pub service: RecordServiceHostCall<S>,
}
pub struct PreparedWithRecordService<'a, B, S, const PORTS: usize, const READ_BYTES: usize> {
    base: B,
    service: PreparedAssignedRecordService<'a, S>,
    back: BoxedRecordServiceBack<PORTS, READ_BYTES>,
    backing: usize,
}
impl<
        'a,
        'o,
        'f,
        R: PreparedResidents<E>,
        P: PulseReadProvider,
        T: OpenDrain,
        W: MonotonicWaitProvider,
        const E: usize,
        const N: usize,
        const A: usize,
    > PreparedDrivers<'a, 'o, 'f, R, P, T, W, E, N, A>
{
    /// Prepare a complete unique mapping and fund Ready, opaque operations and
    /// the actual boxed Back. This neither binds equipment nor admits resources.
    /// # Safety
    /// Only the original authenticated whole-workset Start may later bind this
    /// inventory, after checking this ticket's exact resource binding/index in
    /// the live ledger. The old appender/Play must not own the same residence.
    /// Inline Calls/provider state, physical backing, preparation temporaries
    /// and allocator overhead require their separate unchanged Root funding.
    pub unsafe fn with_record_service<
        's,
        S: NorStorage,
        const PORTS: usize,
        const READ_BYTES: usize,
    >(
        residents: R,
        resident_nodes: [Option<u16>; E],
        software: PreparedSoftware<'o, 'f, N>,
        native: PreparedCalls<'a, P, T, W, A>,
        service: PreparedAssignedRecordService<'s, S>,
    ) -> Result<PreparedWithRecordService<'s, Self, S, PORTS, READ_BYTES>, &'static str>
    where
        R::Back: StepBack<PORTS>,
    {
        let extra = service.retained_backing_bytes().ok_or("service-backing")?;
        let base = Self::new_with_extra(
            residents,
            resident_nodes,
            software,
            native,
            Some(service.node().0),
        )?;
        let layout = core::alloc::Layout::new::<RecordServiceBack<PORTS, READ_BYTES>>();
        let backing = base
            .backing
            .checked_add(extra)
            .and_then(|n| n.checked_add(layout.size()))
            .ok_or("backing")?;
        let back = service
            .prepare_back::<PORTS, READ_BYTES>()
            .map_err(|_| "service-back")?;
        // SAFETY: RecordServiceBack is nonzero-sized. The unique allocation uses
        // exactly Box's layout; failure returns before any Ready owner transfer.
        let boxed = unsafe {
            let allocation =
                alloc::alloc::alloc(layout).cast::<RecordServiceBack<PORTS, READ_BYTES>>();
            if allocation.is_null() {
                return Err("service-allocation");
            }
            allocation.write(back);
            Box::from_raw(allocation)
        };
        Ok(PreparedWithRecordService {
            base,
            service,
            back: BoxedRecordServiceBack(boxed),
            backing,
        })
    }
}
// SAFETY: constructor checks complete unique mapping, exact actual Back shape
// and all retained backing. Its unsafe caller obligation retains original Start
// admission; bind then consumes the sole service ticket without allocation/IO.
unsafe impl<
        'a,
        'o,
        'f,
        's,
        R: PreparedResidents<E>,
        P: PulseReadProvider,
        T: OpenDrain,
        W: MonotonicWaitProvider,
        S: NorStorage,
        const E: usize,
        const N: usize,
        const A: usize,
        const PORTS: usize,
        const READ_BYTES: usize,
    > PreparedBacks<N, PORTS>
    for PreparedWithRecordService<
        's,
        PreparedDrivers<'a, 'o, 'f, R, P, T, W, E, N, A>,
        S,
        PORTS,
        READ_BYTES,
    >
where
    R::Back: StepBack<PORTS>,
{
    type Driver = Driver<'f, R::Back, BoxedRecordServiceBack<PORTS, READ_BYTES>>;
    type Calls = CallsWithRecordService<Calls<P, T, W, A>, S>;
    fn requires_terminal_gate(&self) -> bool {
        true
    }
    fn terminal_transductions(&self) -> Option<[[Option<AssignedTerminalTransduction>; PORTS]; N]> {
        let mut actual = <PreparedDrivers<'a, 'o, 'f, R, P, T, W, E, N, A> as PreparedBacks<
            N,
            PORTS,
        >>::terminal_transductions(&self.base)?;
        actual[usize::from(self.service.node().0)] = self.back.terminal_transductions();
        Some(actual)
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(self.backing)
    }
    #[inline(always)]
    fn bind(self) -> ([Self::Driver; N], Self::Calls) {
        let node = usize::from(self.service.node().0);
        let (mut drivers, base) = self
            .base
            .bind_slots_with_extra::<PORTS, BoxedRecordServiceBack<PORTS, READ_BYTES>>();
        debug_assert!(drivers[node].is_none());
        drivers[node] = Some(Driver::Extra(self.back));
        // SAFETY: constructor obliges the original Root Start and exact complete
        // resource check before this infallible PreparedBacks transfer boundary.
        let service = unsafe { self.service.bind_after_resource_admission() };
        (
            drivers.map(|driver| driver.expect("complete checked service inventory")),
            CallsWithRecordService { base, service },
        )
    }
}
