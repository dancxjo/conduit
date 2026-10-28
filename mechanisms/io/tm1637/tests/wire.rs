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
    assert_eq!(
        &pins.events[..8],
        &[
            Event::Clock(false),
            Event::Delay,
            Event::Data(false),
            Event::Delay,
            Event::Clock(true),
            Event::Delay,
            Event::Data(true),
            Event::Delay,
        ]
    );
    let e = &pins.events[8..];
    assert_eq!(
        &e[..7],
        &[
            Event::Data(true),
            Event::Clock(true),
            Event::Delay,
            Event::Data(false),
            Event::Delay,
            Event::Clock(false),
            Event::Delay
        ]
    );
    for (i, high) in [false, true, true, false, true, false, false, true]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            &e[7 + i * 6..13 + i * 6],
            &[
                Event::Data(high),
                Event::Delay,
                Event::Clock(true),
                Event::Delay,
                Event::Clock(false),
                Event::Delay
            ]
        );
    }
    assert_eq!(
        &e[55..62],
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

#[test]
fn interrupted_transfer_is_stopped_before_any_new_command_and_cancel_does_not_start_recovery() {
    struct Interrupted {
        clock: bool,
        data: bool,
        stopped: bool,
        reads: u32,
    }
    impl OpenDrain for Interrupted {
        type Error = ();
        fn clock(&mut self, released: bool) -> Result<(), ()> {
            self.clock = released;
            Ok(())
        }
        fn data(&mut self, released: bool) -> Result<(), ()> {
            if self.clock && !self.data && released {
                self.stopped = true;
            }
            self.data = released;
            Ok(())
        }
        fn half_period(&mut self) -> Result<(), ()> {
            Ok(())
        }
        fn read_data(&mut self) -> Result<bool, ()> {
            self.reads += 1;
            // Model a peripheral refusing new commands until its documented
            // STOP condition has discarded the interrupted transfer.
            Ok(!self.stopped)
        }
    }
    let mut pins = Interrupted {
        clock: false,
        data: true,
        stopped: false,
        reads: 0,
    };
    transmit(&mut pins, &[Packet { bytes: &[0x40] }], || false).unwrap();
    assert!(pins.stopped);
    assert_eq!(pins.reads, 1);
    let mut cancelled = Pins::default();
    assert_eq!(
        transmit(&mut cancelled, &[Packet { bytes: &[0x40] }], || true),
        Err(Failure::Cancelled)
    );
    assert_eq!(cancelled.events, [Event::Data(true), Event::Clock(true)]);
}

#[test]
fn data_changes_wait_for_a_driven_clock_falling_edge_to_settle() {
    struct Propagating {
        high: bool,
        falling: bool,
        released_data: bool,
        acknowledgments: usize,
    }
    impl Propagating {
        fn released() -> Self {
            Self {
                high: true,
                falling: false,
                released_data: true,
                acknowledgments: 0,
            }
        }
    }
    impl OpenDrain for Propagating {
        type Error = &'static str;
        fn clock(&mut self, released: bool) -> Result<(), Self::Error> {
            if released {
                if self.falling {
                    return Err("clock never settled low");
                }
                self.high = true;
            } else if self.high {
                // The GPIO write is ordered, but the physical falling edge is
                // not instantaneous. One admitted interval makes it observable.
                self.falling = true;
            }
            Ok(())
        }
        fn data(&mut self, released: bool) -> Result<(), Self::Error> {
            if self.falling {
                return Err("data changed before clock settled low");
            }
            self.released_data = released;
            Ok(())
        }
        fn half_period(&mut self) -> Result<(), Self::Error> {
            if self.falling {
                self.high = false;
                self.falling = false;
            }
            Ok(())
        }
        fn read_data(&mut self) -> Result<bool, Self::Error> {
            if !self.high || !self.released_data {
                return Err("invalid ninth-clock sampling");
            }
            self.acknowledgments += 1;
            Ok(false)
        }
    }
    // Validate that the model distinguishes an unstaged transition; it is not
    // an always-ACK fixture that would also accept an immediate DIO change.
    let mut unsettled = Propagating::released();
    unsettled.clock(false).unwrap();
    assert!(unsettled.data(false).is_err());
    let mut actual = Propagating::released();
    transmit(
        &mut actual,
        &[
            Packet { bytes: &[0x40] },
            Packet {
                bytes: &[0xc0, 0x55, 0xaa, 0, 0xff],
            },
            Packet { bytes: &[0x8b] },
        ],
        || false,
    )
    .unwrap();
    assert_eq!(actual.acknowledgments, 7);
    assert!(actual.high);
    assert!(actual.released_data);
}

#[test]
fn admitted_half_period_accounting_covers_complete_and_maximum_transactions() {
    for (packets, bytes) in [
        (vec![Packet { bytes: &[0x40] }], 1),
        (
            vec![
                Packet { bytes: &[0x40] },
                Packet {
                    bytes: &[0xc0, 1, 2, 3, 4, 0, 0],
                },
                Packet { bytes: &[0x8b] },
            ],
            9,
        ),
        (
            vec![
                Packet { bytes: &[0; 7] },
                Packet { bytes: &[0; 7] },
                Packet { bytes: &[0; 7] },
            ],
            21,
        ),
    ] {
        let mut pins = Pins::default();
        transmit(&mut pins, &packets, || false).unwrap();
        let intervals = pins
            .events
            .iter()
            .filter(|event| matches!(event, Event::Delay))
            .count();
        assert_eq!(intervals, 4 + 7 * packets.len() + 27 * bytes);
        assert!(intervals <= 592);
    }
}
