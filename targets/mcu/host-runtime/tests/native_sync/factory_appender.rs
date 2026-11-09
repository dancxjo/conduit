//! CPU proof of actual affine append-ticket inventory composition, no media work.
use super::*;
use conduit_mcu_host_runtime::{
    execution::PreparedBacks,
    execution_factory::{
        Driver, PreparedCalls, PreparedDrivers, PreparedResidents, SoftwareInventory,
    },
};
struct Residents<const E: usize> {
    nodes: [Option<u16>; E],
    backing: usize,
}
// SAFETY: CPU fixture binds exactly its declared slots without allocation/effects.
unsafe impl<const E: usize> PreparedResidents<E> for Residents<E> {
    type Back = HostCallBack;
    fn nodes(&self) -> [Option<u16>; E] {
        self.nodes
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(self.backing)
    }
    fn bind(self) -> [Option<(u16, Self::Back)>; E] {
        self.nodes
            .map(|node| node.map(|node| (node, HostCallBack::new(1))))
    }
}
#[test]
fn actual_append_ticket_preserves_base_calls_and_exact_driver_mapping() {
    let f = fixture::Fixture::new();
    f.with_prepared(false, false, 16, |ticket| {
        let extra_bytes = ticket.retained_backing_bytes().unwrap();
        // SAFETY: no software slots are selected or owned in this CPU inventory.
        let mut software =
            unsafe { SoftwareInventory::new([None, None], [None, None], 0, &[]) }.unwrap();
        let selected = unsafe { software.prepare([false, false]) }.unwrap();
        let prepared = PreparedDrivers::with_appender(
            Residents {
                nodes: [Some(1)],
                backing: 37,
            },
            [Some(1)],
            selected,
            PreparedCalls::<NoHardware, NoHardware, NoHardware, 0> {
                pulse: None,
                write: None,
                waits: [],
            },
            ticket,
        )
        .unwrap();
        assert_eq!(
            PreparedBacks::<2, 1>::retained_backing_bytes(&prepared),
            Some(37 + extra_bytes)
        );
        let (mut drivers, calls) = PreparedBacks::<2, 1>::bind(prepared);
        assert!(matches!(drivers[0], Driver::Native(_)));
        assert!(matches!(drivers[1], Driver::Resident(_)));
        assert!(
            calls.base.pulse.is_none() && calls.base.write.is_none() && calls.base.waits.is_empty()
        );
        let input = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 200,
        };
        let mut io = StepIo::test_frame([Some(input)], [false], [Some(1)], None, 4);
        assert_eq!(
            drivers[0].step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert!(io.test_host_request().is_some());
        <Driver<HostCallBack> as StepBack<1>>::cancel(&mut drivers[0]);
        let oversized = ValueRef {
            byte_len: 201,
            ..input
        };
        let mut io = StepIo::test_frame([Some(oversized)], [false], [Some(1)], None, 4);
        assert!(matches!(
            drivers[0].step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Fail(_)
        ));
        assert!(io.test_host_request().is_none());
        assert_eq!(calls.appender.node(), NodeId(0));
        assert!(calls.appender.custody().is_none());
        assert_eq!(f.storage.programs.get(), 0);
    })
    .unwrap();
    assert_eq!(f.storage.programs.get(), 0);
}
#[test]
fn actual_append_ticket_refuses_duplicate_missing_and_backing_overflow() {
    let f = fixture::Fixture::new();
    for (node, backing, expected) in [(0, 0, "duplicate-driver"), (1, usize::MAX, "backing")] {
        f.with_prepared(false, false, 16, |ticket| {
            // SAFETY: no software owner/selection exists in this test inventory.
            let mut software =
                unsafe { SoftwareInventory::new([None, None], [None, None], 0, &[]) }.unwrap();
            let selected = unsafe { software.prepare([false, false]) }.unwrap();
            let result = PreparedDrivers::with_appender(
                Residents {
                    nodes: [Some(node)],
                    backing,
                },
                [Some(node)],
                selected,
                PreparedCalls::<NoHardware, NoHardware, NoHardware, 0> {
                    pulse: None,
                    write: None,
                    waits: [],
                },
                ticket,
            );
            assert_eq!(result.err(), Some(expected));
        })
        .unwrap();
    }
    f.with_prepared(false, false, 16, |ticket| {
        // SAFETY: empty software slots retain no implementation or effect owner.
        let mut software =
            unsafe { SoftwareInventory::new([None, None, None], [None, None, None], 0, &[]) }
                .unwrap();
        let selected = unsafe { software.prepare([false, false, false]) }.unwrap();
        let result = PreparedDrivers::with_appender(
            Residents {
                nodes: [Some(1)],
                backing: 0,
            },
            [Some(1)],
            selected,
            PreparedCalls::<NoHardware, NoHardware, NoHardware, 0> {
                pulse: None,
                write: None,
                waits: [],
            },
            ticket,
        );
        assert_eq!(result.err(), Some("missing-driver"));
    })
    .unwrap();
    assert_eq!(f.storage.programs.get(), 0);
}

#[test]
fn absent_appender_preserves_complete_original_inventory_without_native_placeholder() {
    // SAFETY: no software slot or effect owner exists in this CPU inventory.
    let mut software = unsafe { SoftwareInventory::new([None], [None], 0, &[]) }.unwrap();
    let selected = unsafe { software.prepare([false]) }.unwrap();
    let prepared = PreparedDrivers::with_optional_appender(
        Residents {
            nodes: [Some(0)],
            backing: 37,
        },
        [Some(0)],
        selected,
        PreparedCalls::<NoHardware, NoHardware, NoHardware, 0> {
            pulse: None,
            write: None,
            waits: [],
        },
        None::<
            conduit_resource_snapshot::native_append::PreparedAssignedAppender<
                '_,
                fixture::Storage,
            >,
        >,
    )
    .unwrap();
    assert_eq!(
        PreparedBacks::<1, 1>::retained_backing_bytes(&prepared),
        Some(37)
    );
    let (drivers, calls) = PreparedBacks::<1, 1>::bind(prepared);
    assert!(matches!(drivers[0], Driver::Resident(_)));
    assert!(calls.appender.is_none());
    assert!(
        calls.base.pulse.is_none() && calls.base.write.is_none() && calls.base.waits.is_empty()
    );
}
