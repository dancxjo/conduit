//! Original native ownership for the selected neutral Source workset. No
//! diagnostic sensor/display algorithm is part of these retained providers.
use conduit_core::HostAdvertisement;
use embassy_rp::{
    i2c::{self, I2c},
    Peri,
};
pub type Equipment<
    I = crate::i2c_owner::NativeBusLease,
    C = crate::clock_at_installation::ClockLease,
> = crate::neutral_equipment_owners::Equipment<I, C>;
/// Sole physical controller initialization; no transaction or protocol starts.
pub fn initialize_bus(
    bus: Peri<'static, crate::config::Bus>,
    scl: Peri<'static, crate::config::Scl>,
    sda: Peri<'static, crate::config::Sda>,
) {
    let mut configuration = i2c::Config::default();
    configuration.frequency = crate::config::FREQUENCY;
    crate::i2c_owner::initialize(conduit_rp2040_i2c::Rp2040I2c::new(
        I2c::new_async(bus, scl, sda, crate::config::SensorIrqs, configuration),
        crate::config::MAXIMUM_I2C_POLLS,
    ));
}
impl Equipment {
    pub fn prepare(actual: &HostAdvertisement, maximum_bytes: usize) -> Result<Self, &'static str> {
        let (bus, bus_bytes) = crate::i2c_installation::prepare(actual, maximum_bytes)
            .map_err(|_| "original-i2c-unavailable")?;
        let remaining = maximum_bytes
            .checked_sub(bus_bytes)
            .ok_or("native-identity-budget")?;
        let (clock, clock_bytes) = crate::clock_at_installation::prepare(actual, remaining)
            .map_err(|_| "original-clock-unavailable")?;
        Ok(Self {
            bus: Some(bus),
            clock: Some(clock),
            identity_bytes: bus_bytes
                .checked_add(clock_bytes)
                .ok_or("native-identity-budget")?,
        })
    }
}
