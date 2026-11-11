//! Complete unique driver mapping and allocation-free binding.
use super::*;

pub(super) type BoundDriverSlots<'f, B, P, T, W, const N: usize, const A: usize> =
    BoundDriverSlotsWithExtra<'f, B, NoExtraBack, P, T, W, N, A>;
pub(super) type BoundDriverSlotsWithExtra<'f, B, X, P, T, W, const N: usize, const A: usize> =
    ([Option<Driver<'f, B, X>>; N], Calls<P, T, W, A>);

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
    pub fn new(
        residents: R,
        resident_nodes: [Option<u16>; E],
        software: PreparedSoftware<'o, 'f, N>,
        native: PreparedCalls<'a, P, T, W, A>,
    ) -> Result<Self, &'static str> {
        Self::new_with_extra(residents, resident_nodes, software, native, None)
    }

    pub(super) fn new_with_extra(
        residents: R,
        resident_nodes: [Option<u16>; E],
        software: PreparedSoftware<'o, 'f, N>,
        native: PreparedCalls<'a, P, T, W, A>,
        extra_node: Option<u16>,
    ) -> Result<Self, &'static str> {
        let pulse_node = native.pulse.as_ref().map(|ticket| ticket.node().0);
        let write_node = native.write.as_ref().map(|ticket| ticket.node().0);
        if residents.nodes() != resident_nodes {
            return Err("resident-mapping");
        }
        let mut seen = [false; N];
        for node in resident_nodes
            .iter()
            .flatten()
            .copied()
            .chain(pulse_node.into_iter().chain(write_node).chain(extra_node))
        {
            let slot = seen.get_mut(usize::from(node)).ok_or("driver-node")?;
            if *slot {
                return Err("duplicate-driver");
            }
            *slot = true;
        }
        for (index, selected) in software.selected.iter().enumerate() {
            if *selected {
                if seen[index] {
                    return Err("duplicate-software");
                }
                seen[index] = true;
            }
        }
        if seen.iter().any(|seen| !*seen) {
            return Err("missing-driver");
        }
        let mut backing = residents
            .retained_backing_bytes()
            .ok_or("resident-backing")?
            .checked_add(software.owner.backing)
            .ok_or("backing")?;
        if let Some(pulse) = &native.pulse {
            backing = backing
                .checked_add(pulse.retained_backing_bytes().ok_or("pulse-backing")?)
                .ok_or("backing")?;
        }
        if let Some(write) = &native.write {
            backing = backing
                .checked_add(write.retained_backing_bytes().ok_or("write-backing")?)
                .ok_or("backing")?;
        }
        let mut clock_nodes = [false; N];
        for wait in &native.waits {
            let clock_node = usize::from(wait.node().0);
            if clock_node >= N || clock_nodes[clock_node] || !software.selected[clock_node] {
                return Err("clock-node");
            }
            clock_nodes[clock_node] = true;
            backing = backing
                .checked_add(wait.retained_backing_bytes().ok_or("clock-backing")?)
                .ok_or("backing")?;
        }
        Ok(Self {
            residents,
            software,
            native,
            resident_nodes,
            pulse_node,
            write_node,
            backing,
        })
    }
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
    #[inline(always)] // Fuse the affine owner transfer with its final destination.
    pub(super) fn bind_slots<const PORTS: usize>(
        self,
    ) -> BoundDriverSlots<'f, R::Back, P, T, W, N, A>
    where
        R::Back: StepBack<PORTS>,
    {
        self.bind_slots_with_extra::<PORTS, NoExtraBack>()
    }
    #[inline(always)]
    pub(super) fn bind_slots_with_extra<const PORTS: usize, X: StepBack<PORTS>>(
        self,
    ) -> BoundDriverSlotsWithExtra<'f, R::Back, X, P, T, W, N, A>
    where
        R::Back: StepBack<PORTS>,
    {
        let mut drivers: [Option<Driver<'f, R::Back, X>>; N] = core::array::from_fn(|_| None);
        for (expected, selected) in self.resident_nodes.into_iter().zip(self.residents.bind()) {
            match (expected, selected) {
                (Some(node), Some((actual, back))) if node == actual => {
                    drivers[usize::from(node)] = Some(Driver::Resident(back))
                }
                (None, None) => {}
                _ => unreachable!("exclusively checked resident mapping"),
            }
        }
        for (index, back) in self.software.bind().into_iter().enumerate() {
            if let Some(back) = back {
                drivers[index] = Some(Driver::Software(back));
            }
        }
        if let Some(node) = self.pulse_node {
            drivers[usize::from(node)] = Some(Driver::Native(HostCallBack::new(
                conduit_pulse_capture::PULSE_READ_MAXIMUM_BYTES,
            )));
        }
        let calls = Calls {
            pulse: self.native.pulse.map(PreparedAssignedPulse::bind),
            write: self.native.write.map(PreparedAssignedWrite::bind),
            waits: self.native.waits.map(PreparedAssignedWait::bind),
        };
        if let Some(node) = self.write_node {
            // This checked Gear is an effect-only sink. Its electrical success
            // completes without a value; retain the admitted operation ceiling
            // and the sink's commit-scoped request generations.
            let maximum_operations = calls
                .write
                .as_ref()
                .expect("checked write owner")
                .inspection()
                .scope
                .maximum_operations;
            drivers[usize::from(node)] = Some(Driver::Write(WriteBack::new(maximum_operations)));
        }
        (drivers, calls)
    }
}
// SAFETY: constructor validates complete unique Source mapping, individual
// opaque tickets retain exact initialized owners and all bind methods are
// allocation-free. Root's generic preparation validates the matching layout.
unsafe impl<
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
        const PORTS: usize,
    > PreparedBacks<N, PORTS> for PreparedDrivers<'a, 'o, 'f, R, P, T, W, E, N, A>
where
    R::Back: StepBack<PORTS>,
{
    type Driver = Driver<'f, R::Back>;
    type Calls = Calls<P, T, W, A>;
    fn terminal_transductions(
        &self,
    ) -> Option<[[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; PORTS]; N]> {
        // Arbitrary generated resident owners have no inspection entrance yet.
        // Software/native/write variants use StepBack's empty contract.
        if self.residents.nodes().iter().any(Option::is_some) {
            return None;
        }
        Some([[None; PORTS]; N])
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(self.backing)
    }
    #[inline(always)] // Fuse the affine owner transfer with its final destination.
    fn bind(self) -> ([Self::Driver; N], Self::Calls) {
        let (drivers, calls) = self.bind_slots::<PORTS>();
        (
            drivers.map(|driver| driver.expect("complete checked driver inventory")),
            calls,
        )
    }
}
