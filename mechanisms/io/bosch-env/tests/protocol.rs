use conduit_bosch_env::*;
use embedded_hal::i2c::{ErrorKind, ErrorType, Operation};
use embedded_hal_async::{delay::DelayNs, i2c::I2c};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    future::Future,
    pin::pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Fault;
impl embedded_hal::i2c::Error for Fault {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}
enum Expect {
    Read(u8, Vec<u8>),
    Write(u8, u8),
    Fail,
}
struct Bus {
    steps: Rc<RefCell<VecDeque<Expect>>>,
    pause: Rc<Cell<bool>>,
    address: u8,
}
impl ErrorType for Bus {
    type Error = Fault;
}
impl I2c for Bus {
    async fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Fault> {
        if self.pause.get() {
            std::future::pending::<()>().await;
        }
        assert_eq!(address, self.address);
        let step = self
            .steps
            .borrow_mut()
            .pop_front()
            .expect("unexpected bus effect");
        match step {
            Expect::Fail => Err(Fault),
            Expect::Write(register, value) => {
                let [Operation::Write(bytes)] = operations else {
                    panic!("expected write")
                };
                assert_eq!(*bytes, &[register, value]);
                Ok(())
            }
            Expect::Read(register, bytes) => {
                let [Operation::Write(request), Operation::Read(out)] = operations else {
                    panic!("expected repeated-start read")
                };
                assert_eq!(*request, &[register]);
                assert_eq!(out.len(), bytes.len());
                out.copy_from_slice(&bytes);
                Ok(())
            }
        }
    }
}
#[derive(Default)]
struct Delay {
    calls: [u32; 32],
    len: usize,
}
impl DelayNs for Delay {
    async fn delay_ns(&mut self, ns: u32) {
        self.calls[self.len] = ns;
        self.len += 1;
    }
}
fn run<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut cx) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("fixture unexpectedly pending"),
    }
}
type ScriptBus = (Bus, Rc<RefCell<VecDeque<Expect>>>, Rc<Cell<bool>>);
fn make_bus(steps: Vec<Expect>, address: u8) -> ScriptBus {
    let steps = Rc::new(RefCell::new(steps.into()));
    let pause = Rc::new(Cell::new(false));
    (
        Bus {
            steps: steps.clone(),
            pause: pause.clone(),
            address,
        },
        steps,
        pause,
    )
}
fn calibration() -> ([u8; 26], [u8; 7]) {
    let words = [
        27504_i32, 26435, -1000, 36477, -10685, 3024, 2855, 140, -7, 15500, -14600, 6000,
    ];
    let mut tp = [0; 26];
    for (i, w) in words.into_iter().enumerate() {
        tp[2 * i..2 * i + 2].copy_from_slice(&(w as u16).to_le_bytes());
    }
    tp[25] = 75;
    (tp, [0x6a, 1, 0, 0x14, 0x25, 3, 30])
}
fn initialize(chip: Chip) -> Vec<Expect> {
    let (tp, h) = calibration();
    let mut steps = vec![
        Expect::Read(0xd0, vec![if chip == Chip::Bme280 { 0x60 } else { 0x58 }]),
        Expect::Write(0xe0, 0xb6),
        Expect::Read(0xf3, vec![0]),
        Expect::Read(
            0x88,
            tp[..if chip == Chip::Bme280 { 26 } else { 24 }].to_vec(),
        ),
    ];
    if chip == Chip::Bme280 {
        steps.push(Expect::Read(0xe1, h.to_vec()));
    }
    steps.push(Expect::Write(0xf5, 0));
    if chip == Chip::Bme280 {
        steps.push(Expect::Write(0xf2, 1));
    }
    steps.push(Expect::Write(0xf4, 0x24));
    steps
}
fn measure(chip: Chip) -> Vec<Expect> {
    let mut steps = Vec::new();
    if chip == Chip::Bme280 {
        steps.push(Expect::Write(0xf2, 1));
    }
    steps.push(Expect::Write(0xf4, 0x25));
    steps.push(Expect::Read(0xf3, vec![0]));
    let (p, t, h) = (415148_u32, 519888_u32, 30000_u16);
    let raw = [
        (p >> 12) as u8,
        (p >> 4) as u8,
        (p << 4) as u8,
        (t >> 12) as u8,
        (t >> 4) as u8,
        (t << 4) as u8,
        (h >> 8) as u8,
        h as u8,
    ];
    steps.push(Expect::Read(
        0xf7,
        raw[..if chip == Chip::Bme280 { 8 } else { 6 }].to_vec(),
    ));
    steps
}
#[test]
fn real_ids_register_order_coherent_bursts_units_and_zero_alloc() {
    for (chip, address) in [
        (Chip::Bme280, Address::Primary),
        (Chip::Bmp280, Address::Secondary),
    ] {
        let mut script = initialize(chip);
        script.extend(measure(chip));
        let (bus, steps, _) = make_bus(script, address as u8);
        let mut sensor = Sensor::new(bus, address);
        let mut delay = Delay::default();
        assert_eq!(sensor.chip(), None);
        let (result, init_alloc) = allocation_probe::observe(|| run(sensor.initialize(&mut delay)));
        assert_eq!(result, Ok(chip));
        assert_eq!(sensor.chip(), Some(chip));
        let (result, read_alloc) = allocation_probe::observe(|| run(sensor.measure(&mut delay)));
        let reading = result.unwrap();
        assert_eq!(reading.temperature_centi_celsius, 2508);
        assert_eq!(reading.pressure_centi_pascal, 10065328);
        match (chip, reading.humidity) {
            (Chip::Bme280, Humidity::PercentQ10(0..=102400))
            | (Chip::Bmp280, Humidity::Unavailable(HumidityUnavailable::UnsupportedChip)) => {}
            _ => panic!("wrong humidity truth"),
        }
        assert!(steps.borrow().is_empty());
        assert_eq!(
            (
                init_alloc.allocations,
                init_alloc.reallocations,
                read_alloc.allocations,
                read_alloc.reallocations
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(&delay.calls[..delay.len], &[2_000_000, 10_000_000]);
    }
}
#[test]
fn unsupported_id_bad_calibration_and_bus_error_never_mark_ready() {
    for id in [0, 0xff, 0x56, 0x57, 0x61] {
        let (bus, steps, _) = make_bus(vec![Expect::Read(0xd0, vec![id])], 0x76);
        let mut sensor = Sensor::new(bus, Address::Primary);
        assert_eq!(
            run(sensor.initialize(&mut Delay::default())),
            Err(Error::UnsupportedChip(id))
        );
        assert_eq!(sensor.chip(), None);
        assert_eq!(
            run(sensor.measure(&mut Delay::default())),
            Err(Error::NotInitialized)
        );
        assert!(steps.borrow().is_empty());
    }
    let mut script = initialize(Chip::Bmp280);
    script[3] = Expect::Read(0x88, vec![0; 24]);
    script.truncate(4);
    let (bus, steps, _) = make_bus(script, 0x76);
    let mut sensor = Sensor::new(bus, Address::Primary);
    assert_eq!(
        run(sensor.initialize(&mut Delay::default())),
        Err(Error::Data(DataRefusal::Calibration))
    );
    assert_eq!(sensor.chip(), None);
    assert!(steps.borrow().is_empty());
    let (bus, steps, _) = make_bus(vec![Expect::Fail], 0x76);
    let mut sensor = Sensor::new(bus, Address::Primary);
    assert_eq!(
        run(sensor.initialize(&mut Delay::default())),
        Err(Error::Bus(Fault))
    );
    assert_eq!(sensor.chip(), None);
    assert!(steps.borrow().is_empty());
    let _retained_bus = sensor.into_inner();
}
#[test]
fn initialization_and_measurement_busy_polls_are_finite() {
    let script = std::iter::once(Expect::Read(0xd0, vec![0x60]))
        .chain(std::iter::once(Expect::Write(0xe0, 0xb6)))
        .chain((0..MAXIMUM_STATUS_POLLS).map(|_| Expect::Read(0xf3, vec![1])))
        .collect();
    let (bus, steps, _) = make_bus(script, 0x76);
    let mut sensor = Sensor::new(bus, Address::Primary);
    let mut delay = Delay::default();
    assert_eq!(run(sensor.initialize(&mut delay)), Err(Error::BusyTimeout));
    assert_eq!(sensor.chip(), None);
    assert!(steps.borrow().is_empty());
    assert_eq!(delay.len, MAXIMUM_STATUS_POLLS);
    let mut script = initialize(Chip::Bmp280);
    script.push(Expect::Write(0xf4, 0x25));
    script.extend((0..MAXIMUM_STATUS_POLLS).map(|_| Expect::Read(0xf3, vec![8])));
    let (bus, steps, _) = make_bus(script, 0x76);
    let mut sensor = Sensor::new(bus, Address::Primary);
    run(sensor.initialize(&mut Delay::default())).unwrap();
    assert_eq!(
        run(sensor.measure(&mut Delay::default())),
        Err(Error::BusyTimeout)
    );
    assert_eq!(sensor.chip(), None);
    assert_eq!(
        run(sensor.measure(&mut Delay::default())),
        Err(Error::NotInitialized)
    );
    assert!(steps.borrow().is_empty());
}
#[test]
fn failed_or_cancelled_read_requires_fresh_initialization_but_retains_bus() {
    let mut script = initialize(Chip::Bme280);
    script.push(Expect::Fail);
    let (bus, steps, pause) = make_bus(script, 0x76);
    let mut sensor = Sensor::new(bus, Address::Primary);
    let mut delay = Delay::default();
    run(sensor.initialize(&mut delay)).unwrap();
    assert_eq!(run(sensor.measure(&mut delay)), Err(Error::Bus(Fault)));
    assert_eq!(sensor.chip(), None);
    steps.borrow_mut().extend(initialize(Chip::Bme280));
    run(sensor.initialize(&mut delay)).unwrap();
    pause.set(true);
    {
        let mut future = pin!(sensor.measure(&mut delay));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(future.as_mut().poll(&mut cx).is_pending());
    }
    assert_eq!(sensor.chip(), None);
    pause.set(false);
    assert_eq!(run(sensor.measure(&mut delay)), Err(Error::NotInitialized));
    steps.borrow_mut().extend(initialize(Chip::Bme280));
    steps.borrow_mut().extend(measure(Chip::Bme280));
    run(sensor.initialize(&mut delay)).unwrap();
    assert!(run(sensor.measure(&mut delay)).is_ok());
    assert!(steps.borrow().is_empty());
}

#[test]
fn exclusive_shared_bus_borrow_preserves_initialized_calibration_and_next_read() {
    let mut steps = initialize(Chip::Bme280);
    steps.extend(measure(Chip::Bme280));
    let (bus, remaining, _) = make_bus(steps, 0x76);
    let mut sensor = Sensor::new(bus, Address::Primary);
    let mut delay = Delay::default();
    run(sensor.initialize(&mut delay)).unwrap();
    let before = sensor.chip();
    // Other device access can borrow this same bus, never clone the peripheral.
    let bus = sensor.bus_mut();
    assert_eq!(bus.address, 0x76);
    assert_eq!(sensor.chip(), before);
    run(sensor.measure(&mut delay)).unwrap();
    assert!(remaining.borrow().is_empty());
}
