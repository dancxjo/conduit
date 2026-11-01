//! Worker/owner scheduling fixtures, not network or physical-device evidence.
use conduit_zenoh_pico_embassy::{BridgeAccess, SocketWorker, WorkerRuntime};
use conduit_zenoh_pico_platform::{Endpoint, Fault, NetworkCommand, Phase, Refusal, SocketBridge};
use std::cell::{Cell, RefCell};
const PEER: Endpoint = Endpoint {
    ipv4: [192, 0, 2, 1],
    port: 7447,
};
struct Access(RefCell<SocketBridge<4, 4>>);
impl Access {
    fn new() -> Self {
        Self(RefCell::new(SocketBridge::new()))
    }
}
impl BridgeAccess<4, 4> for Access {
    fn with<R>(&self, f: impl FnOnce(&mut SocketBridge<4, 4>) -> R) -> R {
        f(&mut self.0.borrow_mut())
    }
}
#[derive(Clone, Copy)]
enum Connect {
    Open,
    Refuse,
    Pending,
}
struct Control {
    now: Cell<u64>,
    yields: Cell<usize>,
    connect: Cell<Connect>,
    retire: Cell<bool>,
    connects: Cell<usize>,
    closes: Cell<usize>,
    yield_fault: Cell<bool>,
}
impl Control {
    fn new() -> Self {
        Self {
            now: Cell::new(0),
            yields: Cell::new(0),
            connect: Cell::new(Connect::Open),
            retire: Cell::new(true),
            connects: Cell::new(0),
            closes: Cell::new(0),
            yield_fault: Cell::new(false),
        }
    }
}
struct Runtime<'a> {
    access: &'a Access,
    control: &'a Control,
}
impl WorkerRuntime for Runtime<'_> {
    fn now_ms(&self) -> u64 {
        self.control.now.get()
    }
    fn yield_worker(&self) -> Result<(), Fault> {
        let c = self.control;
        c.yields.set(c.yields.get() + 1);
        c.now.set(c.now.get() + 1);
        if c.yield_fault.get() {
            return Err(Fault::Network);
        }
        self.access.with(|b| match b.take_command(c.now.get()) {
            Some(NetworkCommand::Connect { generation, .. }) => {
                c.connects.set(c.connects.get() + 1);
                match c.connect.get() {
                    Connect::Open => {
                        b.complete_connect(generation, c.now.get(), Ok(())).unwrap();
                    }
                    Connect::Refuse => {
                        b.complete_connect(generation, c.now.get(), Err(Fault::Network))
                            .unwrap();
                    }
                    Connect::Pending => (),
                }
            }
            Some(NetworkCommand::Close { generation }) => {
                c.closes.set(c.closes.get() + 1);
                if c.retire.get() {
                    b.complete_close(generation).unwrap();
                }
            }
            None => (),
        });
        Ok(())
    }
}
#[test]
fn accepted_binary_prefix_returns_without_yield_and_is_never_replayed() {
    let access = Access::new();
    let control = Control::new();
    let mut worker = SocketWorker::new(
        &access,
        Runtime {
            access: &access,
            control: &control,
        },
        5,
    )
    .unwrap();
    let generation = worker.connect(PEER, 10).unwrap();
    let before = control.yields.get();
    assert_eq!(worker.write(generation, &[0, 255, 128, 9, 8, 7], 10), Ok(4));
    assert_eq!(
        control.yields.get(),
        before,
        "accepted prefix must not wait for socket delivery"
    );
    assert_eq!(
        worker.write(generation, &[8, 7], 0),
        Err(Refusal::Fault(Fault::TimedOut))
    );
    let g = access.with(|b| b.generation().unwrap());
    let mut sent = [0; 8];
    let ticket = access.with(|b| b.network_send(g, &mut sent).unwrap().unwrap());
    assert_eq!(&sent[..ticket.len()], &[0, 255, 128, 9]);
    access.with(|b| b.network_sent(ticket, 2).unwrap());
    assert_eq!(worker.write(generation, &[8, 7], 10), Ok(2));
    let ticket = access.with(|b| b.network_send(g, &mut sent).unwrap().unwrap());
    assert_eq!(&sent[..ticket.len()], &[128, 9, 8, 7]);
    access.with(|b| b.network_sent(ticket, 4).unwrap());
    assert_eq!(access.with(|b| b.network_send(g, &mut sent)), Ok(None));
    worker.close(generation).unwrap();
}
#[test]
fn buffered_binary_precedes_fault_and_eof_and_readiness_does_not_consume() {
    for fault in [false, true] {
        let access = Access::new();
        let control = Control::new();
        let mut worker = SocketWorker::new(
            &access,
            Runtime {
                access: &access,
                control: &control,
            },
            5,
        )
        .unwrap();
        let generation = worker.connect(PEER, 10).unwrap();
        access.with(|b| {
            let g = b.generation().unwrap();
            assert_eq!(b.network_receive(g, &[0, 255, 128]), Ok(3));
            if fault {
                b.fail(g, Fault::Network).unwrap();
            } else {
                b.peer_closed(g).unwrap();
            }
        });
        assert_eq!(worker.ready(generation, 0), Ok(true));
        let before = control.yields.get();
        let mut bytes = [0; 2];
        assert_eq!(worker.read(generation, &mut bytes, 10), Ok(2));
        assert_eq!(bytes, [0, 255]);
        assert_eq!(worker.read(generation, &mut bytes, 10), Ok(1));
        assert_eq!(bytes[0], 128);
        if fault {
            assert_eq!(
                worker.read(generation, &mut bytes, 0),
                Err(Refusal::Fault(Fault::Network))
            );
        } else {
            assert_eq!(worker.read(generation, &mut bytes, 0), Ok(0));
        }
        assert_eq!(control.yields.get(), before);
        worker.close(generation).unwrap();
    }
}
#[test]
fn empty_read_and_readiness_deadlines_and_nonblocking_do_not_mutate_payload() {
    let access = Access::new();
    let control = Control::new();
    let mut worker = SocketWorker::new(
        &access,
        Runtime {
            access: &access,
            control: &control,
        },
        5,
    )
    .unwrap();
    let generation = worker.connect(PEER, 10).unwrap();
    let mut bytes = [42; 3];
    let before = control.yields.get();
    assert_eq!(worker.ready(generation, 0), Ok(false));
    assert_eq!(
        worker.read(generation, &mut bytes, 0),
        Err(Refusal::Fault(Fault::TimedOut))
    );
    assert_eq!(bytes, [42; 3]);
    assert_eq!(control.yields.get(), before);
    worker.set_blocking(generation, false).unwrap();
    assert_eq!(
        worker.read(generation, &mut bytes, 100),
        Err(Refusal::Fault(Fault::TimedOut))
    );
    assert_eq!(worker.ready(generation, 100), Ok(false));
    assert_eq!(control.yields.get(), before);
    worker.set_blocking(generation, true).unwrap();
    let start = control.now.get();
    assert_eq!(
        worker.read(generation, &mut bytes, 3),
        Err(Refusal::Fault(Fault::TimedOut))
    );
    assert!(control.now.get() >= start + 3 && control.now.get() <= start + 4);
    assert_eq!(bytes, [42; 3]);
    assert_eq!(access.with(|b| b.phase()), Phase::Open);
    worker.close(generation).unwrap();
}
#[test]
fn close_quarantines_until_actual_retirement_then_old_generation_refuses() {
    let access = Access::new();
    let control = Control::new();
    let mut worker = SocketWorker::new(
        &access,
        Runtime {
            access: &access,
            control: &control,
        },
        3,
    )
    .unwrap();
    let old = worker.connect(PEER, 10).unwrap();
    control.retire.set(false);
    let start = control.now.get();
    assert_eq!(worker.close(old), Err(Refusal::Fault(Fault::TimedOut)));
    assert!(control.now.get() <= start + 4);
    assert_eq!(access.with(|b| b.phase()), Phase::Closing);
    assert_eq!(worker.connect(PEER, 10), Err(Refusal::Busy));
    access.with(|b| b.complete_close(b.generation().unwrap()).unwrap());
    control.retire.set(true);
    let new = worker.connect(PEER, 10).unwrap();
    assert_ne!(old, new);
    let before = control.yields.get();
    let mut bytes = [0; 1];
    assert_eq!(worker.read(old, &mut bytes, 10), Err(Refusal::Stale));
    assert_eq!(worker.write(old, &[1], 10), Err(Refusal::Stale));
    assert_eq!(worker.ready(old, 10), Err(Refusal::Stale));
    assert_eq!(worker.set_blocking(old, false), Err(Refusal::Stale));
    assert_eq!(worker.close(old), Err(Refusal::Stale));
    assert_eq!(control.yields.get(), before);
    worker.close(new).unwrap();
}
#[test]
fn refused_or_unanswered_connect_has_one_attempt_and_bounded_cleanup() {
    for mode in [Connect::Refuse, Connect::Pending] {
        let access = Access::new();
        let control = Control::new();
        control.connect.set(mode);
        let mut worker = SocketWorker::new(
            &access,
            Runtime {
                access: &access,
                control: &control,
            },
            3,
        )
        .unwrap();
        let expected = match mode {
            Connect::Refuse => Fault::Network,
            _ => Fault::TimedOut,
        };
        assert_eq!(worker.connect(PEER, 5), Err(Refusal::Fault(expected)));
        assert_eq!(control.connects.get(), 1);
        assert_eq!(control.closes.get(), 1);
        assert_eq!(access.with(|b| b.phase()), Phase::Idle);
        assert!(control.now.get() <= 9);
        control.connect.set(Connect::Open);
        let generation = worker.connect(PEER, 5).unwrap();
        worker.close(generation).unwrap();
    }
}

#[test]
fn invalid_close_bound_and_poll_connect_refuse_without_queuing_and_yield_fault_propagates() {
    let access = Access::new();
    let control = Control::new();
    assert!(matches!(
        SocketWorker::new(
            &access,
            Runtime {
                access: &access,
                control: &control
            },
            0
        ),
        Err(Refusal::Extent)
    ));
    let mut worker = SocketWorker::new(
        &access,
        Runtime {
            access: &access,
            control: &control,
        },
        3,
    )
    .unwrap();
    assert_eq!(
        worker.connect(PEER, 0),
        Err(Refusal::Fault(Fault::TimedOut))
    );
    assert_eq!(access.with(|b| b.phase()), Phase::Idle);
    assert_eq!(control.yields.get(), 0);
    assert_eq!(control.connects.get(), 0);
    let generation = worker.connect(PEER, 10).unwrap();
    control.yield_fault.set(true);
    let mut bytes = [42; 2];
    assert_eq!(
        worker.read(generation, &mut bytes, 10),
        Err(Refusal::Fault(Fault::Network))
    );
    assert_eq!(bytes, [42; 2]);
    control.yield_fault.set(false);
    worker.close(generation).unwrap();
}
#[test]
fn connect_retirement_timeout_preserves_original_failure_and_quarantines() {
    let access = Access::new();
    let control = Control::new();
    control.connect.set(Connect::Refuse);
    control.retire.set(false);
    let mut worker = SocketWorker::new(
        &access,
        Runtime {
            access: &access,
            control: &control,
        },
        3,
    )
    .unwrap();
    assert_eq!(
        worker.connect(PEER, 10),
        Err(Refusal::Fault(Fault::Network))
    );
    assert_eq!(access.with(|b| b.phase()), Phase::Closing);
    assert_eq!(control.connects.get(), 1);
    assert_eq!(control.closes.get(), 1);
    assert!(control.now.get() <= 5);
    assert_eq!(worker.connect(PEER, 10), Err(Refusal::Busy));
    access.with(|b| b.complete_close(b.generation().unwrap()).unwrap());
    assert_eq!(access.with(|b| b.phase()), Phase::Idle);
}

#[test]
fn a_new_successful_connection_resets_nonblocking_mode() {
    let access = Access::new();
    let control = Control::new();
    let mut worker = SocketWorker::new(
        &access,
        Runtime {
            access: &access,
            control: &control,
        },
        3,
    )
    .unwrap();
    let old = worker.connect(PEER, 10).unwrap();
    worker.set_blocking(old, false).unwrap();
    let before = control.yields.get();
    let mut bytes = [42; 2];
    assert_eq!(
        worker.read(old, &mut bytes, 3),
        Err(Refusal::Fault(Fault::TimedOut))
    );
    assert_eq!(control.yields.get(), before);
    worker.close(old).unwrap();
    let new = worker.connect(PEER, 10).unwrap();
    assert_ne!(old, new);
    let before = control.yields.get();
    let start = control.now.get();
    assert_eq!(
        worker.read(new, &mut bytes, 3),
        Err(Refusal::Fault(Fault::TimedOut))
    );
    assert!(
        control.yields.get() > before,
        "new connection must restore blocking waits"
    );
    assert!(control.now.get() >= start + 3);
    assert_eq!(bytes, [42; 2]);
    worker.close(new).unwrap();
}

#[test]
fn clock_ceiling_close_still_requests_retirement_and_quarantines() {
    let access = Access::new();
    let control = Control::new();
    let mut worker = SocketWorker::new(
        &access,
        Runtime {
            access: &access,
            control: &control,
        },
        3,
    )
    .unwrap();
    let generation = worker.connect(PEER, 10).unwrap();
    control.now.set(u64::MAX);
    let before = control.yields.get();
    assert_eq!(
        worker.close(generation),
        Err(Refusal::Fault(Fault::TimedOut))
    );
    assert_eq!(
        control.yields.get(),
        before,
        "expired close must not attempt overflowing timer progress"
    );
    assert_eq!(access.with(|b| b.phase()), Phase::Closing);
    assert_eq!(
        access.with(|b| b.request_connect(PEER, u64::MAX)),
        Err(Refusal::Busy)
    );
    // The queued close remains available to the actual owner despite worker refusal.
    access.with(|b| {
        let Some(NetworkCommand::Close { generation: actual }) = b.take_command(u64::MAX) else {
            panic!("retirement command was lost at clock ceiling");
        };
        assert_eq!(actual.get(), generation);
        b.complete_close(actual).unwrap();
    });
    assert_eq!(access.with(|b| b.phase()), Phase::Idle);
}
