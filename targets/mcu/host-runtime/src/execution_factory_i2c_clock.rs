//! Typed native tickets for ordinary I2C and monotonic-deadline operations.
//! No sensor, display, protocol transition or scheduling policy lives here.
use super::{NativeCallNode, PreparedCallSet};
use conduit_i2c_transaction::{
    contract::I2C_MAXIMUM_BYTES, installation::PreparedAssignedI2c, owner::I2cHostCall, I2cProvider,
};
use conduit_monotonic_wait::clock_at::{
    assigned::PreparedAssignedClockAt,
    contract::CLOCK_MAXIMUM_BYTES,
    owner::{MonotonicClockHostCall, MonotonicDeadlineProvider},
};

pub struct PreparedI2cClockCalls<
    'a,
    I: I2cProvider,
    C: MonotonicDeadlineProvider,
    const B: usize,
    const D: usize,
> {
    pub buses: [PreparedAssignedI2c<'a, I>; B],
    pub clocks: [PreparedAssignedClockAt<'a, C>; D],
}
pub struct I2cClockCalls<I, C, const B: usize, const D: usize> {
    pub buses: [I2cHostCall<I>; B],
    pub clocks: [MonotonicClockHostCall<C>; D],
}
// SAFETY: every mapping is read from the same exclusively retained typed ticket.
// Bounds are the linked exact native contracts, not received metadata. Typed
// binders transfer the original providers without allocating or starting IO.
unsafe impl<I: I2cProvider, C: MonotonicDeadlineProvider, const B: usize, const D: usize>
    PreparedCallSet for PreparedI2cClockCalls<'_, I, C, B, D>
{
    type Calls = I2cClockCalls<I, C, B, D>;
    fn nodes(&self) -> impl Iterator<Item = NativeCallNode> {
        self.buses
            .iter()
            .map(|bus| NativeCallNode {
                node: bus.node().0,
                maximum_input_bytes: I2C_MAXIMUM_BYTES,
            })
            .chain(self.clocks.iter().map(|clock| NativeCallNode {
                node: clock.node().0,
                maximum_input_bytes: CLOCK_MAXIMUM_BYTES,
            }))
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        self.buses
            .iter()
            .map(PreparedAssignedI2c::retained_backing_bytes)
            .chain(
                self.clocks
                    .iter()
                    .map(PreparedAssignedClockAt::retained_backing_bytes),
            )
            .try_fold(0usize, |sum, bytes| sum.checked_add(bytes?))
    }
    #[inline(always)]
    fn bind(self) -> Self::Calls {
        I2cClockCalls {
            buses: self.buses.map(PreparedAssignedI2c::bind),
            clocks: self.clocks.map(PreparedAssignedClockAt::bind),
        }
    }
}
