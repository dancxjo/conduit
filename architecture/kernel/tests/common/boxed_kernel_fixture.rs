//! Owned-store drop witnesses and a one-node zero-Cord production kernel.
use conduit_kernel::{scheduler::*, *};
use std::{cell::Cell, rc::Rc};
#[derive(Default)]
pub struct Drops {
    pub values: Cell<usize>,
    pub signs: Cell<usize>,
    pub drivers: Cell<usize>,
}
pub struct Driver(pub Rc<Drops>);
impl Drop for Driver {
    fn drop(&mut self) {
        self.0.drivers.set(self.0.drivers.get() + 1);
    }
}
impl StepBack<1> for Driver {
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        StepOutcome::Complete
    }
}
pub struct Values {
    inner: FixedValueStore<1, 1>,
    drops: Rc<Drops>,
}
impl Drop for Values {
    fn drop(&mut self) {
        self.drops.values.set(self.drops.values.get() + 1);
    }
}
impl ValueStorage for Values {
    fn item_capacity(&self) -> u16 {
        self.inner.item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        self.inner.byte_capacity()
    }
    fn used_items(&self) -> u16 {
        self.inner.used_items()
    }
    fn used_bytes(&self) -> u32 {
        self.inner.used_bytes()
    }
    fn store(&mut self, b: &[u8]) -> Result<ValueRef, StorageError> {
        self.inner.store(b)
    }
    fn get(&self, v: ValueRef) -> Result<&[u8], StorageError> {
        self.inner.get(v)
    }
    fn reference_count(&self, v: ValueRef) -> Result<u16, StorageError> {
        self.inner.reference_count(v)
    }
    fn retain(&mut self, v: ValueRef) -> Result<(), StorageError> {
        self.inner.retain(v)
    }
    fn release(&mut self, v: ValueRef) -> Result<(), StorageError> {
        self.inner.release(v)
    }
    fn clear(&mut self) {
        self.inner.clear()
    }
}
pub struct Signs {
    inner: FixedSignLog<16>,
    drops: Rc<Drops>,
}
impl Drop for Signs {
    fn drop(&mut self) {
        self.drops.signs.set(self.drops.signs.get() + 1);
    }
}
impl SignSink for Signs {
    fn record_remote(
        &mut self,
        n: NodeId,
        p: PortId,
        k: KernelEventKind,
        remote: RemoteLifecycleIdentity,
    ) -> Result<KernelEvent, SignError> {
        self.inner.record_remote(n, p, k, remote)
    }
    fn ensure_remote_capacity(&self, additional: u16) -> Result<(), SignError> {
        self.inner.ensure_remote_capacity(additional)
    }
    fn item_capacity(&self) -> u16 {
        self.inner.item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        self.inner.byte_capacity()
    }
    fn len(&self) -> u16 {
        self.inner.len()
    }
    fn used_bytes(&self) -> u32 {
        self.inner.used_bytes()
    }
    fn record(
        &mut self,
        n: NodeId,
        p: Option<PortId>,
        r: Option<RequestId>,
        k: KernelEventKind,
    ) -> Result<KernelEvent, SignError> {
        self.inner.record(n, p, r, k)
    }
}
pub type Kernel = FixedScheduler<Driver, Values, Signs, 1, 1, 1, 1, 1, 0, 0, 1>;
pub type Prepared = PreparedKernelStorage<Driver, Kernel, 1>;
pub fn prepare(
    drops: &Rc<Drops>,
    ceiling: usize,
    active: usize,
) -> Result<Prepared, KernelPreparationError> {
    let mut routes = FixedRoutes::<1, 0>::new(1);
    routes.seal().unwrap();
    let mut calls = FixedHostCallBindings::<0>::new(1);
    calls.seal().unwrap();
    Kernel::prepare_boxed(
        active,
        0,
        [NodeSpec {
            input_cords: [None],
            maximum_step_fuel: 1,
        }],
        [CordSpec::inactive()],
        routes,
        calls,
        Values {
            inner: FixedValueStore::new(1).unwrap(),
            drops: drops.clone(),
        },
        Signs {
            inner: FixedSignLog::new((16 * size_of::<KernelEvent>()) as u32).unwrap(),
            drops: drops.clone(),
        },
        ceiling,
    )
}

pub fn bound(drops: &Rc<Drops>) -> Result<Box<Kernel>, KernelPreparationError> {
    let mut routes = FixedRoutes::<1, 0>::new(1);
    routes.seal().unwrap();
    let mut calls = FixedHostCallBindings::<0>::new(1);
    calls.seal().unwrap();
    Kernel::new_boxed_with_active_counts_and_host_calls(
        1,
        0,
        [NodeSpec {
            input_cords: [None],
            maximum_step_fuel: 1,
        }],
        [CordSpec::inactive()],
        routes,
        calls,
        [Driver(drops.clone())],
        Values {
            inner: FixedValueStore::new(1).unwrap(),
            drops: drops.clone(),
        },
        Signs {
            inner: FixedSignLog::new((16 * size_of::<KernelEvent>()) as u32).unwrap(),
            drops: drops.clone(),
        },
    )
}

#[allow(dead_code)]
pub fn prepare_written(
    drops: &Rc<Drops>,
    ceiling: usize,
    write: impl FnOnce(
        &mut [NodeSpec<1>; 1],
        &mut [CordSpec; 1],
        &mut FixedRoutes<1, 0>,
        &mut FixedHostCallBindings<0>,
    ) -> Result<(usize, usize), KernelPreparationError>,
) -> Result<Prepared, KernelPreparationError> {
    Kernel::prepare_boxed_with_table_writer(
        Values {
            inner: FixedValueStore::new(1).unwrap(),
            drops: drops.clone(),
        },
        Signs {
            inner: FixedSignLog::new((16 * size_of::<KernelEvent>()) as u32).unwrap(),
            drops: drops.clone(),
        },
        ceiling,
        &[[None]],
        &[[None]],
        core::convert::identity,
        write,
    )
}
