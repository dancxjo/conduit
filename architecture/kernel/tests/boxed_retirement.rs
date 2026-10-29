//! Boxed retirement returns exact backing without materializing terminal drivers.
#![cfg(feature = "alloc")]
use conduit_kernel::{self as k, scheduler::*, ValueStorage};
use std::{cell::Cell, rc::Rc};

struct Driver {
    drops: Rc<Cell<u32>>,
    _large: [u8; 8192],
}
impl Drop for Driver {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
impl StepBack<1> for Driver {
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        StepOutcome::Complete
    }
}
type Kernel = FixedScheduler<Driver, k::HostedValueStore, k::HostedSignLog, 1, 0, 1, 1, 1, 0, 0, 1>;
fn specimen() -> (Box<Kernel>, Rc<Cell<u32>>, k::ValueRef) {
    let drops = Rc::new(Cell::new(0));
    let mut routes = k::FixedRoutes::<1, 0>::new(1);
    routes.seal().unwrap();
    let mut values = k::HostedValueStore::new(2, 8, 16).unwrap();
    let old = values.store(&[11, 22]).unwrap();
    let signs =
        k::HostedSignLog::new(32, 32 * core::mem::size_of::<k::KernelEvent>() as u32).unwrap();
    let kernel = Kernel::new_with_active_counts(
        1,
        0,
        [NodeSpec {
            input_cords: [None],
            maximum_step_fuel: 8,
        }],
        [],
        routes,
        [Driver {
            drops: drops.clone(),
            _large: [19; 8192],
        }],
        values,
        signs,
    )
    .unwrap();
    (Box::new(kernel), drops, old)
}
#[test]
fn live_refusal_keeps_same_box_drivers_values_and_signs_then_terminal_return_preserves_generations()
{
    let (kernel, drops, old) = specimen();
    let address = (&*kernel) as *const Kernel;
    let before = kernel.signs().events().collect::<Vec<_>>();
    let mut kernel = match kernel.try_retire_backing() {
        Err(kernel) => kernel,
        Ok(_) => panic!("live scheduler retired"),
    };
    assert_eq!((&*kernel) as *const Kernel, address);
    assert_eq!(drops.get(), 0);
    assert_eq!(kernel.values().get(old).unwrap(), [11, 22]);
    assert_eq!(kernel.signs().events().collect::<Vec<_>>(), before);
    assert!(matches!(
        kernel.step(),
        Ok(SchedulerStatus::Progress { .. } | SchedulerStatus::Drained)
    ));
    assert!(kernel.can_retire());
    let expected = kernel.signs().events().collect::<Vec<_>>();
    let (mut values, signs) = match kernel.try_retire_backing() {
        Ok(backing) => backing,
        Err(_) => panic!("terminal retirement refused"),
    };
    assert_eq!(drops.get(), 1);
    assert_eq!(signs.events().collect::<Vec<_>>(), expected);
    assert_eq!(values.get(old).unwrap(), [11, 22]);
    values.clear();
    assert!(values.get(old).is_err());
    let fresh = values.store(&[33]).unwrap();
    assert_eq!(fresh.slot, old.slot);
    assert!(fresh.generation > old.generation);
}
#[test]
fn cancellation_gate_and_backing_match_original_full_retirement() {
    let (mut direct, drops, _) = specimen();
    let (mut full, full_drops, _) = specimen();
    direct.cancel().unwrap();
    full.cancel().unwrap();
    let expected = (*full)
        .try_retire()
        .unwrap_or_else(|_| panic!("cancelled full scheduler refused"));
    let expected_signs = expected.signs.events().collect::<Vec<_>>();
    let (values, signs) = direct
        .try_retire_backing()
        .unwrap_or_else(|_| panic!("cancelled boxed scheduler refused"));
    assert_eq!(drops.get(), 1);
    assert_eq!(full_drops.get(), 0);
    assert_eq!(values.used_items(), expected.values.used_items());
    assert_eq!(signs.events().collect::<Vec<_>>(), expected_signs);
    drop(expected);
    assert_eq!(full_drops.get(), 1);
}
