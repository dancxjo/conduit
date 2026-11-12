//! Complete Source inventories with independently prepared unary native calls.
use super::*;
use conduit_kernel::scheduler::AssignedTerminalTransduction;

#[derive(Clone, Copy)]
pub struct NativeCallNode {
    pub node: u16,
    pub maximum_input_bytes: u32,
}

/// Original typed native tickets, not offers or newly minted possession.
/// Each node uses the kernel's one-input/one-output HostCallBack with call 0.
/// # Safety
/// Node mappings and bounds must describe the exact independently checked
/// tickets returned by bind, remain immutable, and contain no duplicate loans.
/// Binding transfers every ticket once without allocation or provider effects.
/// Dropping before bind leaves the same original initialized owners available.
/// Backing includes all owned allocations retained by these exact tickets;
/// borrowed providers, inline storage and allocator overhead are funded by Root.
pub unsafe trait PreparedCallSet {
    type Calls;
    fn nodes(&self) -> impl Iterator<Item = NativeCallNode>;
    fn retained_backing_bytes(&self) -> Option<usize>;
    fn bind(self) -> Self::Calls;
}

/// Generic hardware-independent composition. Native adapters own the typed
/// possession and providers; this owner validates complete, unique node custody.
pub struct PreparedNativeDrivers<'o, 'f, R, Q, const E: usize, const N: usize> {
    residents: R,
    resident_nodes: [Option<u16>; E],
    software: PreparedSoftware<'o, 'f, N>,
    native: Q,
    backing: usize,
}
impl<'o, 'f, R: PreparedResidents<E>, Q: PreparedCallSet, const E: usize, const N: usize>
    PreparedNativeDrivers<'o, 'f, R, Q, E, N>
{
    /// Checks custody only. Resource admission and lawful Start still belong to
    /// Root, which must retain the actual tickets through the whole workset.
    pub fn new(
        residents: R,
        resident_nodes: [Option<u16>; E],
        software: PreparedSoftware<'o, 'f, N>,
        native: Q,
    ) -> Result<Self, &'static str> {
        if residents.nodes() != resident_nodes {
            return Err("resident-mapping");
        }
        let mut seen = [false; N];
        for node in resident_nodes.iter().flatten().copied() {
            claim_node(&mut seen, node)?;
        }
        // A malicious trusted adapter cannot make constructor work unbounded.
        // N+1 observations suffice to refuse an oversized inventory.
        for node in native.nodes().take(N.saturating_add(1)) {
            if node.maximum_input_bytes == 0 {
                return Err("native-input-bound");
            }
            claim_node(&mut seen, node.node)?;
        }
        for (index, selected) in software.selected.iter().enumerate() {
            if *selected {
                claim_node(&mut seen, u16::try_from(index).map_err(|_| "driver-node")?)?;
            }
        }
        if seen.iter().any(|seen| !*seen) {
            return Err("missing-driver");
        }
        let backing = residents
            .retained_backing_bytes()
            .ok_or("resident-backing")?
            .checked_add(software.owner.backing)
            .ok_or("backing")?
            .checked_add(native.retained_backing_bytes().ok_or("native-backing")?)
            .ok_or("backing")?;
        Ok(Self {
            residents,
            resident_nodes,
            software,
            native,
            backing,
        })
    }
}
fn claim_node<const N: usize>(seen: &mut [bool; N], node: u16) -> Result<(), &'static str> {
    let slot = seen.get_mut(usize::from(node)).ok_or("driver-node")?;
    if *slot {
        return Err("duplicate-driver");
    }
    *slot = true;
    Ok(())
}

// SAFETY: the constructor validates complete unique mapping before any owner
// transfer. PreparedResidents and PreparedCallSet retain the exact affine loans;
// bind performs only infallible moves into the same checked driver positions.
unsafe impl<
        'o,
        'f,
        R: PreparedResidents<E>,
        Q: PreparedCallSet,
        const E: usize,
        const N: usize,
        const P: usize,
    > PreparedBacks<N, P> for PreparedNativeDrivers<'o, 'f, R, Q, E, N>
where
    R::Back: StepBack<P>,
{
    type Driver = Driver<'f, R::Back>;
    type Calls = Q::Calls;
    fn requires_terminal_gate(&self) -> bool {
        true
    }
    fn terminal_transductions(&self) -> Option<[[Option<AssignedTerminalTransduction>; P]; N]> {
        let mut actual = [[None; P]; N];
        self.write_terminal_transductions(&mut actual)?;
        Some(actual)
    }
    fn write_terminal_transductions(
        &self,
        target: &mut [[Option<AssignedTerminalTransduction>; P]; N],
    ) -> Option<()> {
        target.fill([None; P]);
        for (index, selected) in self.software.selected.iter().enumerate() {
            if *selected {
                target[index] = <SoftwareBack as StepBack<P>>::terminal_transductions(
                    self.software.owner.slots[index].as_ref()?,
                );
            }
        }
        if self.resident_nodes.iter().all(Option::is_none) {
            return Some(());
        }
        for (slot, node) in self.resident_nodes.iter().enumerate() {
            match (node, self.residents.terminal_transductions_at::<P>(slot)?) {
                (Some(node), Some(report)) => *target.get_mut(usize::from(*node))? = report,
                (None, None) => {}
                _ => return None,
            }
        }
        Some(())
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(self.backing)
    }
    #[inline(always)]
    fn bind(self) -> ([Self::Driver; N], Self::Calls) {
        let mut drivers: [Option<Self::Driver>; N] = core::array::from_fn(|_| None);
        for (expected, selected) in self.resident_nodes.into_iter().zip(self.residents.bind()) {
            match (expected, selected) {
                (Some(node), Some((actual, back))) if node == actual => {
                    drivers[usize::from(node)] = Some(Driver::Resident(back))
                }
                (None, None) => {}
                _ => unreachable!("checked resident mapping"),
            }
        }
        for (index, back) in self.software.bind().into_iter().enumerate() {
            if let Some(back) = back {
                drivers[index] = Some(Driver::Software(back));
            }
        }
        for node in self.native.nodes() {
            drivers[usize::from(node.node)] =
                Some(Driver::Native(HostCallBack::new(node.maximum_input_bytes)));
        }
        (
            drivers.map(|d| d.expect("complete checked native inventory")),
            self.native.bind(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::member_return_tests::return_allocation_probe;
    use core::cell::Cell;
    struct Residents<'a> {
        node: Option<u16>,
        available: &'a Cell<bool>,
    }
    // SAFETY: these inert fixtures report the exact HostCallBack and one slot.
    // Only bind withdraws the slot; there is no provider or execution authority.
    unsafe impl PreparedResidents<1> for Residents<'_> {
        type Back = HostCallBack;
        fn nodes(&self) -> [Option<u16>; 1] {
            [self.node]
        }
        fn terminal_transductions_at<const P: usize>(
            &self,
            i: usize,
        ) -> Option<Option<[Option<AssignedTerminalTransduction>; P]>> {
            (i == 0).then(|| self.node.map(|_| [None; P]))
        }
        fn retained_backing_bytes(&self) -> Option<usize> {
            Some(0)
        }
        fn bind(self) -> [Option<(u16, Self::Back)>; 1] {
            self.available.set(false);
            [self.node.map(|n| (n, HostCallBack::new(32)))]
        }
    }
    struct Calls<'a> {
        nodes: &'a [u16],
        available: &'a Cell<bool>,
    }
    // SAFETY: immutable borrowed fixture mappings, no authority or provider.
    unsafe impl PreparedCallSet for Calls<'_> {
        type Calls = ();
        fn nodes(&self) -> impl Iterator<Item = NativeCallNode> {
            self.nodes.iter().map(|&node| NativeCallNode {
                node,
                maximum_input_bytes: 32,
            })
        }
        fn retained_backing_bytes(&self) -> Option<usize> {
            Some(0)
        }
        fn bind(self) {
            self.available.set(false);
        }
    }
    fn empty<const N: usize>() -> SoftwareInventory<'static, N> {
        SoftwareInventory {
            slots: core::array::from_fn(|_| None),
            identities: [None; N],
            backing: 0,
            configuration: &[],
        }
    }
    #[test]
    fn invalid_native_mappings_preserve_every_original_loan() {
        for (nodes, reason) in [
            (&[0, 0][..], "duplicate-driver"),
            (&[2][..], "driver-node"),
            (&[][..], "missing-driver"),
            (&[1][..], "duplicate-driver"),
        ] {
            let ready = Cell::new(true);
            let native_ready = Cell::new(true);
            let mut software = empty::<2>();
            let result = PreparedNativeDrivers::new(
                Residents {
                    node: Some(1),
                    available: &ready,
                },
                [Some(1)],
                PreparedSoftware {
                    owner: &mut software,
                    selected: [false; 2],
                },
                Calls {
                    nodes,
                    available: &native_ready,
                },
            );
            assert!(matches!(result, Err(actual) if actual == reason));
            assert!(ready.get() && native_ready.get());
        }
    }
    #[test]
    fn complete_preflight_and_bind_are_effect_and_allocation_free() {
        let ready = Cell::new(true);
        let native_ready = Cell::new(true);
        let mut software = empty::<2>();
        let probe = return_allocation_probe::ObservationInterval::begin();
        let prepared = PreparedNativeDrivers::new(
            Residents {
                node: Some(1),
                available: &ready,
            },
            [Some(1)],
            PreparedSoftware {
                owner: &mut software,
                selected: [false; 2],
            },
            Calls {
                nodes: &[0],
                available: &native_ready,
            },
        )
        .unwrap();
        assert!(ready.get() && native_ready.get());
        assert_eq!(
            <_ as PreparedBacks<2, 1>>::retained_backing_bytes(&prepared),
            Some(0)
        );
        let (drivers, ()) = <_ as PreparedBacks<2, 1>>::bind(prepared);
        assert!(matches!(drivers[0], Driver::Native(_)));
        assert!(matches!(drivers[1], Driver::Resident(_)));
        let observed = probe.finish();
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
        assert!(!ready.get() && !native_ready.get());
    }
    #[test]
    fn terminal_software_law_matches_the_bound_inventory() {
        let ready = Cell::new(true);
        let native_ready = Cell::new(true);
        let schema =
            conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap();
        let mut software = empty::<2>();
        software.slots[1] = Some(SoftwareBack::merge(&schema, 8).unwrap());
        let prepared = PreparedNativeDrivers::new(
            Residents {
                node: None,
                available: &ready,
            },
            [None],
            PreparedSoftware {
                owner: &mut software,
                selected: [false, true],
            },
            Calls {
                nodes: &[0],
                available: &native_ready,
            },
        )
        .unwrap();
        assert!(<_ as PreparedBacks<2, 2>>::requires_terminal_gate(
            &prepared
        ));
        let reports = <_ as PreparedBacks<2, 2>>::terminal_transductions(&prepared).unwrap();
        assert_eq!(reports[0], [None; 2]);
        assert!(reports[1].iter().all(Option::is_some));
        let (drivers, ()) = <_ as PreparedBacks<2, 2>>::bind(prepared);
        for (driver, report) in drivers.iter().zip(reports) {
            assert_eq!(
                <Driver<HostCallBack> as StepBack<2>>::terminal_transductions(driver),
                report
            );
        }
    }
}
