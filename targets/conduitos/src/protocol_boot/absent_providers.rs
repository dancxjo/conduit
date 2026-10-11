//! Uninhabited generic slots: this USB-only Root cannot construct I2C or clock owners.
pub(super) enum AbsentI2c {}
pub(super) enum AbsentClock {}
impl crate::i2c_base::I2cProvider for AbsentI2c {
    fn transact(
        &mut self,
        _: &crate::i2c_base::transaction::I2cTransaction<'_>,
        _: &mut [u8],
    ) -> Result<usize, crate::i2c_base::transaction::I2cDisposition> {
        match *self {}
    }
    fn revoke(&mut self) {
        match *self {}
    }
}
impl crate::monotonic_clock::owner::MonotonicDeadlineProvider for AbsentClock {
    fn poll_until(
        &mut self,
        _: u64,
    ) -> Result<Option<u64>, crate::monotonic_clock::codec::ClockDisposition> {
        match *self {}
    }
    fn revoke(&mut self) {
        match *self {}
    }
}
