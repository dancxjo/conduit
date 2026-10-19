use super::*;
pub const N: usize = 34;
const C: usize = 33;
pub struct Counted {
    inner: FixedValueStore<64, 8>,
    pub stores: u64,
}
impl ValueStorage for Counted {
    fn item_capacity(&self) -> u16 {
        self.inner.item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        self.inner.byte_capacity()
    }
    fn used_items(&self) -> u16 {
        self.inner.used_items()
    }
    fn used_bytes(&self) -> u32 {
        self.inner.used_bytes()
    }
    fn store(&mut self, b: &[u8]) -> Result<ValueRef, StorageError> {
        let v = self.inner.store(b)?;
        self.stores += 1;
        Ok(v)
    }
    fn get(&self, v: ValueRef) -> Result<&[u8], StorageError> {
        self.inner.get(v)
    }
    fn reference_count(&self, v: ValueRef) -> Result<u16, StorageError> {
        self.inner.reference_count(v)
    }
    fn retain(&mut self, v: ValueRef) -> Result<(), StorageError> {
        self.inner.retain(v)
    }
    fn release(&mut self, v: ValueRef) -> Result<(), StorageError> {
        self.inner.release(v)
    }
    fn clear(&mut self) {
        self.inner.clear();
    }
}
pub type GenericPlay<E> =
    FixedScheduler<Driver, NativeU64ValueStore<Counted, 32>, E, N, C, 1, C, N, C>;
pub type Play = GenericPlay<HostedSignLog>;
pub enum Driver {
    Source {
        values: [Option<ValueRef>; 4],
        index: usize,
        count: usize,
        staged: bool,
    },
    Add {
        bytes: [u8; 8],
        fail: bool,
    },
    Sink {
        values: [u64; 4],
        count: usize,
        delay: usize,
        staged: Option<u64>,
    },
}
impl Driver {
    pub fn result(&self) -> (&[u64; 4], usize) {
        match self {
            Self::Sink { values, count, .. } => (values, *count),
            _ => panic!("not a sink"),
        }
    }
}
impl StepBack<1> for Driver {
    fn native_u64_ports(&self) -> Option<(PortId, PortId)> {
        matches!(self, Self::Add { .. }).then_some((PortId(0), PortId(0)))
    }
    fn step(&mut self, io: &mut StepIo<1>, input: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Source {
                values,
                index,
                count,
                staged,
            } => {
                if *index == *count {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), values[*index].unwrap()).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Add { bytes, fail } => {
                let value = input.native_u64(PortId(0)).or_else(|| {
                    input
                        .input(PortId(0))
                        .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
                });
                if let Some(value) = value {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if *fail {
                        return StepOutcome::Fail(Failure {
                            code: FailureCode::InvalidInput,
                            detail: 77,
                        });
                    }
                    let next = value.checked_add(1).unwrap();
                    if io.native_u64_output(PortId(0)) {
                        io.send_native_u64(PortId(0), next).unwrap();
                    } else {
                        *bytes = next.to_le_bytes();
                        io.send_prepared(PortId(0), 8).unwrap();
                    }
                    io.consume(PortId(0)).unwrap();
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::Sink { delay, staged, .. } => {
                if *delay > 0 {
                    *delay -= 1;
                    io.consume_fuel(io.remaining_fuel()).unwrap();
                    return StepOutcome::Yield;
                }
                if let Some(bytes) = input.input(PortId(0)) {
                    *staged = Some(u64::from_le_bytes(bytes.try_into().unwrap()));
                    io.consume(PortId(0)).unwrap();
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }
    fn prepared_output(&self, _: PortId) -> Option<&[u8]> {
        match self {
            Self::Add { bytes, .. } => Some(bytes),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source { index, staged, .. } if *staged => {
                *index += 1;
                *staged = false;
            }
            Self::Sink {
                values,
                count,
                staged: Some(value),
                ..
            } => {
                values[*count] = *value;
                *count += 1;
                if let Self::Sink { staged, .. } = self {
                    *staged = None;
                }
            }
            _ => {}
        }
    }
}
pub fn prepare_unbound<E: SignSink>(
    count: usize,
    delay: usize,
    failure: Option<usize>,
    capacity: u32,
    signs: E,
) -> GenericPlay<E> {
    let mut values = NativeU64ValueStore::new(
        Counted {
            inner: FixedValueStore::new(512).unwrap(),
            stores: 0,
        },
        capacity,
    )
    .unwrap();
    let inputs = core::array::from_fn(|i| {
        (i < count).then(|| values.store(&(i as u64).to_le_bytes()).unwrap())
    });
    let mut routes = FixedRoutes::<N, C>::new(1);
    for i in 0..C {
        routes
            .install(
                NodeId(i as u16),
                PortId(0),
                RouteRange {
                    start: i as u16,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(i as u16),
                    sink: CordEndpoint::local(NodeId(i as u16 + 1), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let drivers = core::array::from_fn(|i| {
        if i == 0 {
            Driver::Source {
                values: inputs,
                index: 0,
                count,
                staged: false,
            }
        } else if i == N - 1 {
            Driver::Sink {
                values: [0; 4],
                count: 0,
                delay,
                staged: None,
            }
        } else {
            Driver::Add {
                bytes: [0; 8],
                fail: failure == Some(i),
            }
        }
    });
    GenericPlay::new(
        core::array::from_fn(|i| NodeSpec {
            input_cords: [(i > 0).then(|| CordId(i as u16 - 1))],
            maximum_step_fuel: 8,
        }),
        core::array::from_fn(|i| {
            CordSpec::local(
                CordId(i as u16),
                (NodeId(i as u16), PortId(0)),
                (NodeId(i as u16 + 1), PortId(0)),
                CordCapacity {
                    slot_start: i as u16,
                    item_capacity: 1,
                    byte_capacity: 8,
                    pressure_policy: Default::default(),
                },
            )
        }),
        routes,
        drivers,
        values,
        signs,
    )
    .unwrap()
}
pub fn evidence() -> HostedSignLog {
    HostedSignLog::new(4096, 4096 * core::mem::size_of::<KernelEvent>() as u32).unwrap()
}
pub fn prepare(native: bool, count: usize, delay: usize, failure: Option<usize>) -> Play {
    let mut play = prepare_unbound(count, delay, failure, 256, evidence());
    let chain: Vec<_> = (1..=32).map(NodeId).collect();
    if native {
        play.bind_native_u64_chain(&chain).unwrap();
    } else {
        play.bind_execution_chain(&chain).unwrap();
    }
    play
}
pub fn finish<E: SignSink>(play: &mut GenericPlay<E>) {
    for _ in 0..10000 {
        if play.step().unwrap() == SchedulerStatus::Drained {
            return;
        }
    }
    panic!("did not drain");
}
