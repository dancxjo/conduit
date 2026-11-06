//! Actual firmware controller lease, using a fake configured physical leaf.
extern crate self as conduit_rp2040_i2c;
use conduit_i2c_transaction::{I2cDisposition, I2cProvider, I2cTransaction};
use std::{
    future::Future,
    marker::PhantomData,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;
mod config {
    pub type Bus = ();
}
pub struct Rp2040I2c<'a, T> {
    calls: Arc<AtomicUsize>,
    revoked: bool,
    marker: PhantomData<&'a T>,
}
impl<T> Rp2040I2c<'_, T> {
    pub fn is_revoked(&self) -> bool {
        self.revoked
    }
}
impl<T> I2cProvider for Rp2040I2c<'_, T> {
    fn transact(
        &mut self,
        _: &I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        assert!(owner::protocol_revision() > 0);
        if self.revoked {
            return Err(I2cDisposition::ProviderLost);
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        input.fill(7);
        Ok(input.len())
    }
    fn revoke(&mut self) {
        self.revoked = true;
    }
}
#[path = "../../firmware/host/src/i2c_owner.rs"]
mod owner;

#[test]
fn one_controller_lease_survives_cancellation_and_invalidates_cache_only_on_native_use() {
    critical_section::with(|_| {});
    assert!(owner::try_lease().is_none());
    let calls = Arc::new(AtomicUsize::new(0));
    let bus = Rp2040I2c {
        calls: calls.clone(),
        revoked: false,
        marker: PhantomData,
    };
    let ((), cost) = probe::observe(|| owner::initialize(bus));
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    let lease = owner::try_lease().unwrap();
    let address = (&*lease as *const Rp2040I2c<'_, ()>) as usize;
    assert_eq!(std::mem::size_of_val(&lease), std::mem::size_of::<usize>());
    assert!(owner::try_lease().is_none());
    assert!(owner::try_native_lease().is_none());
    drop(lease);
    let ((), cost) = probe::observe(|| drop(owner::try_native_lease().unwrap()));
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(owner::protocol_revision(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let request = I2cTransaction::new(112, &[0], 1).unwrap();
    let ((), cost) = probe::observe(|| {
        let mut native = owner::try_native_lease().unwrap();
        let mut input = [0];
        assert_eq!(native.transact(&request, &mut input), Ok(1));
        assert_eq!(input, [7]);
        assert_eq!(native.transact(&request, &mut input), Ok(1));
        assert_eq!(owner::protocol_revision(), 1);
        assert!(owner::try_lease().is_none());
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let pending = async {
        let _native = owner::try_native_lease().unwrap();
        core::future::pending::<()>().await;
    };
    assert!(std::mem::size_of_val(&pending) <= 3 * std::mem::size_of::<usize>());
    let mut pending = Box::pin(pending);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(pending.as_mut().poll(&mut context).is_pending());
    let ((), cost) = probe::observe(|| drop(pending));
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(owner::protocol_revision(), 1);
    let lease = owner::try_lease().unwrap();
    assert_eq!((&*lease as *const Rp2040I2c<'_, ()>) as usize, address);
    drop(lease);
    let ((), cost) = probe::observe(|| {
        let mut native = owner::try_native_lease().unwrap();
        native.revoke();
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(owner::protocol_revision(), 2);
    assert!(owner::try_native_lease().is_none());
    assert!(owner::try_lease().unwrap().is_revoked());
}
