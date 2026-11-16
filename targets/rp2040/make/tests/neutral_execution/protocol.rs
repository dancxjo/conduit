//! CPU register protocol, never linked into firmware or a physical claim.
use conduit_i2c_transaction::{I2cDisposition, I2cProvider, I2cTransaction};
use conduit_monotonic_wait::clock_at::{codec::ClockDisposition, owner::MonotonicDeadlineProvider};
use core::cell::Cell;
std::thread_local! {
    static SENSOR_STEP: Cell<usize> = const { Cell::new(0) };
    static DISPLAY_STEP: Cell<usize> = const { Cell::new(0) };
}
pub struct CpuBus<'a>(pub &'a Cell<usize>);
impl I2cProvider for CpuBus<'_> {
    fn transact(
        &mut self,
        transaction: &I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        self.0.set(self.0.get() + 1);
        let write = transaction.write();
        match transaction.address() {
            0x76 => {
                let step = SENSOR_STEP.with(|next| {
                    let step = next.get();
                    next.set(step + 1);
                    step
                });
                let register = match step {
                    0 => 0xd0,
                    1 => 0xe0,
                    2 | 39 => 0xf3,
                    3 | 38 => 0xf4,
                    4 => 0xf2,
                    5 => 0xf5,
                    6..=29 => 0x88 + (step - 6) as u8,
                    30 => 0xa1,
                    31..=37 => 0xe1 + (step - 31) as u8,
                    40..=47 => 0xf7 + (step - 40) as u8,
                    _ => panic!("extra sensor transaction"),
                };
                assert_eq!(write[0], register, "sensor transcript order at {step}");
                assert!(!write.is_empty());
                if transaction.read_length() == 0 {
                    assert!(matches!(
                        write,
                        [0xe0, 0xb6] | [0xf4, 0] | [0xf2, 1] | [0xf5, 0] | [0xf4, 0x25]
                    ));
                    return Ok(0);
                }
                assert_eq!(transaction.read_length(), 1);
                let coefficients: [u16; 12] = [
                    27504,
                    26435,
                    (-1000i16) as u16,
                    36477,
                    (-10685i16) as u16,
                    3024,
                    2855,
                    140,
                    (-7i16) as u16,
                    15500,
                    (-14600i16) as u16,
                    6000,
                ];
                input[0] = match write {
                    [0xd0] => 0x60,
                    [0xf3] => 0,
                    [0xa1] => 75,
                    [r @ 0x88..=0x9f] => coefficients[usize::from(r - 0x88) / 2].to_le_bytes()
                        [usize::from(r - 0x88) % 2],
                    [r @ 0xe1..=0xe7] => [0x6a, 1, 0, 0x13, 0x2b, 3, 30][usize::from(r - 0xe1)],
                    [r @ 0xf7..=0xfe] => {
                        [0x65, 0x5a, 0xc0, 0x7e, 0xed, 0, 0x75, 0x30][usize::from(r - 0xf7)]
                    }
                    _ => panic!("unexpected BME register request {write:?}"),
                };
                Ok(1)
            }
            0x70 => {
                let step = DISPLAY_STEP.with(|next| {
                    let step = next.get();
                    next.set(step + 1);
                    step
                });
                let expected: &[u8] = match step {
                    0 => &[0x21],
                    1 => &[0x80],
                    2 => &[0; 17],
                    3 => &[0xe7],
                    4 => &[0x81],
                    5 => &[0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
                    _ => panic!("extra display transaction"),
                };
                assert_eq!(write, expected, "display transcript order at {step}");
                assert_eq!(transaction.read_length(), 0);
                assert!(
                    matches!(write, [0x21] | [0x80] | [0xe7] | [0x81])
                        || write == [0; 17]
                        || write == [0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
                );
                Ok(0)
            }
            address => panic!("unexpected bus address {address}"),
        }
    }
    fn revoke(&mut self) {}
}
std::thread_local! { static TIME: Cell<u64> = const { Cell::new(17) }; }
pub struct CpuClock<'a>(pub &'a Cell<usize>);
impl MonotonicDeadlineProvider for CpuClock<'_> {
    fn poll_until(&mut self, deadline: u64) -> Result<Option<u64>, ClockDisposition> {
        self.0.set(self.0.get() + 1);
        Ok(Some(TIME.with(|time| {
            let observed = deadline.max(time.get());
            time.set(observed);
            observed
        })))
    }
    fn sample_now(&mut self) -> Result<u64, ClockDisposition> {
        self.0.set(self.0.get() + 1);
        Ok(TIME.with(Cell::get))
    }
    fn revoke(&mut self) {}
}
