use conduit_ht16k33::*;
use embedded_hal::i2c::{ErrorKind, ErrorType, Operation};
use embedded_hal_async::i2c::I2c;
use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fault;
impl embedded_hal::i2c::Error for Fault {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}
#[derive(Default)]
struct Bus {
    calls: usize,
    packets: [[u8; 17]; 8],
    lengths: [usize; 8],
    fail: Option<usize>,
    pending: bool,
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
        assert_eq!(address, 0x70);
        if self.pending {
            std::future::pending::<()>().await;
        }
        let [Operation::Write(bytes)] = operations else {
            panic!("write only")
        };
        let at = self.calls;
        self.calls += 1;
        if self.fail == Some(at) {
            return Err(Fault);
        }
        self.packets[at][..bytes.len()].copy_from_slice(bytes);
        self.lengths[at] = bytes.len();
        Ok(())
    }
}
fn run<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("pending"),
    }
}
fn config() -> Configuration {
    Configuration {
        brightness: Brightness::new(15).unwrap(),
        blink: Blink::Off,
        enabled: true,
    }
}
#[test]
fn checked_scalar_bounds() {
    for value in 0..=255 {
        assert_eq!(Address::new(value).is_ok(), (0x70..=0x77).contains(&value));
        assert_eq!(Brightness::new(value).is_ok(), value <= 15);
    }
}
#[test]
fn initialization_and_full_ram_packet_are_exact_and_allocation_free() {
    let mut bus = Bus::default();
    let ((), allocation) = allocation_probe::observe(|| {
        run(initialize(&mut bus, Address::new(0x70).unwrap(), config())).unwrap();
        let ram = core::array::from_fn(|i| i as u8);
        run(write_frame(&mut bus, Address::new(0x70).unwrap(), &ram)).unwrap();
    });
    assert_eq!(allocation.allocations, 0);
    assert_eq!(bus.calls, 6);
    assert_eq!(&bus.packets[0][..1], &[0x21]);
    assert_eq!(&bus.packets[1][..1], &[0x80]);
    assert_eq!(bus.packets[2], [0; 17]);
    assert_eq!(bus.lengths[2], 17);
    assert_eq!(bus.packets[3][0], 0xef);
    assert_eq!(bus.packets[4][0], 0x81);
    assert_eq!(
        bus.packets[5],
        core::array::from_fn(|i| if i == 0 { 0 } else { (i - 1) as u8 })
    );
}
#[test]
fn every_initialization_error_stops_and_cancel_is_not_success() {
    for fail in 0..5 {
        let mut bus = Bus {
            fail: Some(fail),
            ..Bus::default()
        };
        assert_eq!(
            run(initialize(&mut bus, Address::new(0x70).unwrap(), config())),
            Err(Fault)
        );
        assert_eq!(bus.calls, fail + 1);
    }
    let mut bus = Bus {
        pending: true,
        ..Bus::default()
    };
    {
        let future = initialize(&mut bus, Address::new(0x70).unwrap(), config());
        let mut future = pin!(future);
        assert!(future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
    }
    assert_eq!(bus.calls, 0);
    bus.pending = false;
    run(initialize(&mut bus, Address::new(0x70).unwrap(), config())).unwrap();
    assert_eq!(bus.calls, 5);
}
#[test]
fn blink_and_enable_commands_preserve_all_modes() {
    let mut bus = Bus::default();
    for (index, blink) in [Blink::Off, Blink::Hz2, Blink::Hz1, Blink::HzHalf]
        .into_iter()
        .enumerate()
    {
        for enabled in [false, true] {
            run(set_display(
                &mut bus,
                Address::new(0x70).unwrap(),
                blink,
                enabled,
            ))
            .unwrap();
            assert_eq!(
                bus.packets[index * 2 + usize::from(enabled)][0],
                0x80 | ((index as u8) << 1) | u8::from(enabled)
            );
        }
    }
}
#[test]
fn finite_alpha_profile_preserves_positions_decimal_and_refuses_aliases() {
    let p = Profile::adafruit_alpha4();
    let glyphs = ['H', ' ', '4', '4'].map(|c| Glyph::from_char(c).unwrap());
    let ram = p.encode(glyphs, [false, false, false, true]);
    assert_eq!(&ram[..8], &[0xf6, 0, 0, 0, 0xe6, 0, 0xe6, 0x40]);
    assert_eq!(&ram[8..], &[0; 8]);
    assert_eq!(Glyph::from_char('💧'), Err(Refusal::Glyph));
    assert!(Profile::new([0, 0, 2, 3], core::array::from_fn(|i| i as u8), 14).is_err());
    assert!(Profile::new([0, 1, 2, 8], core::array::from_fn(|i| i as u8), 14).is_err());
    assert!(Profile::new([0, 1, 2, 3], [0; 14], 14).is_err());
    assert!(Profile::new([0, 1, 2, 3], core::array::from_fn(|i| i as u8), 0).is_err());
}

#[test]
fn degree_has_complete_upper_loop_in_exact_controller_wire_words() {
    let profile = Profile::adafruit_alpha4();
    let glyphs = ['7', '2', '°', 'F'].map(|c| Glyph::from_char(c).unwrap());
    let frame = profile.encode(glyphs, [false; 4]);
    assert_eq!(
        frame,
        [7, 0, 0xdb, 0, 0xe3, 0, 0x71, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    let degree = u16::from_le_bytes([frame[4], frame[5]]);
    assert_eq!(
        degree & 0x0063,
        0x0063,
        "top and both upper verticals preserved"
    );
    assert_eq!(
        degree & 0x00c0,
        0x00c0,
        "both bottom halves close the upper loop"
    );
    assert_eq!(
        degree & !0x00e3,
        0,
        "no lower verticals or diagonal substitute"
    );
    let mut bus = Bus::default();
    run(write_frame(&mut bus, Address::new(0x70).unwrap(), &frame)).unwrap();
    assert_eq!(bus.lengths[0], 17);
    assert_eq!(
        bus.packets[0],
        [0, 7, 0, 0xdb, 0, 0xe3, 0, 0x71, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}
