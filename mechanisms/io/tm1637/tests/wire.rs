use conduit_tm1637_wire::{transmit, Failure, OpenDrain, Packet};

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Clock(bool),
    Data(bool),
    Read,
    Delay,
}
#[derive(Default)]
struct Pins {
    events: Vec<Event>,
    reads: usize,
    nack: Option<usize>,
    fail: Option<usize>,
}
impl Pins {
    fn record(&mut self, e: Event) -> Result<(), ()> {
        self.events.push(e);
        if self.fail == Some(self.events.len()) {
            Err(())
        } else {
            Ok(())
        }
    }
    fn released(&self) {
        assert_eq!(
            &self.events[self.events.len() - 2..],
            &[Event::Data(true), Event::Clock(true)]
        );
    }
}
impl OpenDrain for Pins {
    type Error = ();
    fn clock(&mut self, value: bool) -> Result<(), ()> {
        self.record(Event::Clock(value))
    }
    fn data(&mut self, value: bool) -> Result<(), ()> {
        self.record(Event::Data(value))
    }
    fn half_period(&mut self) -> Result<(), ()> {
        self.record(Event::Delay)
    }
    fn read_data(&mut self) -> Result<bool, ()> {
        self.record(Event::Read)?;
        let nack = self.nack == Some(self.reads);
        self.reads += 1;
        Ok(nack)
    }
}

#[test]
fn least_significant_bit_first_and_ninth_clock_releases_data() {
    let mut pins = Pins::default();
    transmit(&mut pins, &[Packet { bytes: &[0x96] }], || false).unwrap();
    let e = &pins.events;
    assert_eq!(
        &e[..6],
        &[
            Event::Data(true),
            Event::Clock(true),
            Event::Delay,
            Event::Data(false),
            Event::Delay,
            Event::Clock(false)
        ]
    );
    for (i, high) in [false, true, true, false, true, false, false, true]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            &e[6 + i * 5..11 + i * 5],
            &[
                Event::Data(high),
                Event::Delay,
                Event::Clock(true),
                Event::Delay,
                Event::Clock(false)
            ]
        );
    }
    assert_eq!(
        &e[46..53],
        &[
            Event::Data(true),
            Event::Delay,
            Event::Clock(true),
            Event::Delay,
            Event::Read,
            Event::Clock(false),
            Event::Delay
        ]
    );
    pins.released();
}

#[test]
fn every_byte_acknowledged_and_nack_stops_without_retry() {
    let packets = [
        Packet { bytes: &[0x40] },
        Packet {
            bytes: &[0xc0, 1, 2, 3, 4, 0, 0],
        },
        Packet { bytes: &[0x8a] },
    ];
    let mut pins = Pins::default();
    transmit(&mut pins, &packets, || false).unwrap();
    assert_eq!(pins.reads, 9);
    for failed in 0..9 {
        let mut pins = Pins {
            nack: Some(failed),
            ..Pins::default()
        };
        let location = match failed {
            0 => (0, 0),
            8 => (2, 0),
            n => (1, n - 1),
        };
        assert_eq!(
            transmit(&mut pins, &packets, || false),
            Err(Failure::Nack {
                packet: location.0,
                byte: location.1 as u8
            })
        );
        assert_eq!(pins.reads, failed + 1);
        pins.released();
    }
}

#[test]
fn malformed_geometry_has_no_effect_and_cancellation_releases_lines() {
    for packets in [
        vec![],
        vec![Packet { bytes: &[] }],
        vec![Packet { bytes: &[0; 8] }],
    ] {
        let mut pins = Pins::default();
        assert_eq!(
            transmit(&mut pins, &packets, || false),
            Err(Failure::InvalidGeometry)
        );
        assert!(pins.events.is_empty());
    }
    for cancel_at in 0..4 {
        let mut pins = Pins::default();
        let mut count = 0;
        assert_eq!(
            transmit(&mut pins, &[Packet { bytes: &[1, 2, 3] }], || {
                let cancel = count == cancel_at;
                count += 1;
                cancel
            }),
            Err(Failure::Cancelled)
        );
        pins.released();
    }
}

#[test]
fn provider_failure_at_every_transition_attempts_both_releases() {
    let mut good = Pins::default();
    transmit(&mut good, &[Packet { bytes: &[0x40] }], || false).unwrap();
    for failed in 1..=good.events.len() {
        let mut pins = Pins {
            fail: Some(failed),
            ..Pins::default()
        };
        assert_eq!(
            transmit(&mut pins, &[Packet { bytes: &[0x40] }], || false),
            Err(Failure::Provider(()))
        );
        pins.released();
    }
}
