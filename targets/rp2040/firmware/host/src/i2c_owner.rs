//! One configured controller, lent exclusively to diagnostics or native calls.
//! Rust custody is not Base authority, a reservation, readiness or a Play.
use conduit_i2c_transaction::{I2cDisposition, I2cProvider, I2cTransaction};
use core::{
    cell::{Cell, RefCell},
    ops::{Deref, DerefMut},
};
use critical_section::Mutex;
use static_cell::StaticCell;
type Bus = conduit_rp2040_i2c::Rp2040I2c<'static, crate::config::Bus>;
static OWNER: StaticCell<Bus> = StaticCell::new();
static SLOT: Mutex<RefCell<Option<&'static mut Bus>>> = Mutex::new(RefCell::new(None));
// Diagnostic cache invalidation, deliberately distinct from Host/Base generation.
static PROTOCOL_REVISION: Mutex<Cell<u64>> = Mutex::new(Cell::new(0));
pub fn initialize(bus: Bus) {
    let bus = OWNER.init(bus);
    critical_section::with(|cs| *SLOT.borrow(cs).borrow_mut() = Some(bus));
}
/// None means an outstanding loan or no observation, not a Lost provider.
pub fn native_available() -> Option<bool> {
    critical_section::with(|cs| {
        SLOT.borrow(cs)
            .borrow()
            .as_ref()
            .map(|bus| !bus.is_revoked())
    })
}
pub fn protocol_revision() -> u64 {
    critical_section::with(|cs| PROTOCOL_REVISION.borrow(cs).get())
}
pub struct BusLease(Option<&'static mut Bus>);
impl Deref for BusLease {
    type Target = Bus;
    fn deref(&self) -> &Bus {
        self.0.as_deref().expect("live bus lease")
    }
}
impl DerefMut for BusLease {
    fn deref_mut(&mut self) -> &mut Bus {
        self.0.as_deref_mut().expect("live bus lease")
    }
}
impl Drop for BusLease {
    fn drop(&mut self) {
        if let Some(bus) = self.0.take() {
            critical_section::with(|cs| {
                let mut slot = SLOT.borrow(cs).borrow_mut();
                debug_assert!(slot.is_none());
                *slot = Some(bus);
            });
        }
    }
}
pub fn try_lease() -> Option<BusLease> {
    critical_section::with(|cs| {
        SLOT.borrow(cs)
            .borrow_mut()
            .take()
            .map(|bus| BusLease(Some(bus)))
    })
}
/// Actual configured native custody for the existing I2C Host Call factory.
/// Root must independently check its selected attachment, capabilities and Plan.
pub struct NativeBusLease {
    bus: BusLease,
    touched: bool,
}
pub fn try_native_lease() -> Option<NativeBusLease> {
    let bus = try_lease()?;
    if bus.is_revoked() {
        return None;
    }
    Some(NativeBusLease {
        bus,
        touched: false,
    })
}
impl NativeBusLease {
    fn begin(&mut self) -> Result<(), I2cDisposition> {
        if self.touched {
            return Ok(());
        }
        let changed = critical_section::with(|cs| {
            let revision = PROTOCOL_REVISION.borrow(cs);
            revision.get().checked_add(1).map(|next| revision.set(next))
        });
        if changed.is_none() {
            self.bus.revoke();
            return Err(I2cDisposition::ProviderLost);
        }
        self.touched = true;
        Ok(())
    }
}
impl I2cProvider for NativeBusLease {
    fn transact(
        &mut self,
        request: &I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        self.begin()?;
        self.bus.transact(request, input)
    }
    fn revoke(&mut self) {
        if self.begin().is_ok() {
            self.bus.revoke();
        }
    }
}
