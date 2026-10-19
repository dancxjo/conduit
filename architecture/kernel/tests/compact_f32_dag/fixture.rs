use super::*;
pub const N: usize = 6;
pub const C: usize = 6;
pub const SHAPE: CompactF32Layout = CompactF32Layout {
    lanes: 2,
    logical_bytes: 16,
};
pub struct Counted {
    inner: HostedValueStore,
    pub canonical: u64,
    pub compact: u64,
    pub physical: u64,
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
        self.canonical += 1;
        Ok(v)
    }
    fn get(&self, v: ValueRef) -> Result<&[u8], StorageError> {
        self.inner.get(v)
    }
    fn retain(&mut self, v: ValueRef) -> Result<(), StorageError> {
        self.inner.retain(v)
    }
    fn release(&mut self, v: ValueRef) -> Result<(), StorageError> {
        self.inner.release(v)
    }
    fn reference_count(&self, v: ValueRef) -> Result<u16, StorageError> {
        self.inner.reference_count(v)
    }
    fn clear(&mut self) {
        self.inner.clear()
    }
    fn compact_f32_available(&self, n: u32) -> (u16, u32) {
        self.inner.compact_f32_available(n)
    }
    fn compact_f32(&self, v: ValueRef) -> Result<Option<&[u8]>, StorageError> {
        self.inner.compact_f32(v)
    }
    fn store_compact_f32(&mut self, b: &[u8], n: u32) -> Result<ValueRef, StorageError> {
        let v = self.inner.store_compact_f32(b, n)?;
        self.compact += 1;
        self.physical += b.len() as u64;
        Ok(v)
    }
}
pub type Play = FixedScheduler<Driver, Counted, HostedSignLog, N, C, 2, C, 12, C>;
pub struct Driver {
    index: usize,
    source: Option<ValueRef>,
    finished: bool,
    staged: bool,
    bytes: [u8; 16],
    length: usize,
    pub result: Option<[f32; 2]>,
    staged_result: Option<[f32; 2]>,
    delay: usize,
    failure: bool,
    supported: bool,
    input_layout: CompactF32Layout,
    canonical_input: Option<PortId>,
}
impl Driver {
    pub fn compact_staging_witness(&self) -> bool {
        self.length == 8 && &self.bytes[8..] != b"F32CANON"
    }
}
fn decode(input: &StepInputBytes<'_, 2>, port: PortId) -> Option<[f32; 2]> {
    let bytes = if let Some(bytes) = input.compact_f32(port) {
        assert_eq!(bytes.len(), 8);
        bytes
    } else {
        let bytes = input.input(port)?;
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[8..], b"F32CANON");
        bytes
    };
    Some(core::array::from_fn(|i| {
        f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
    }))
}

impl StepBack<2> for Driver {
    fn compact_f32_input(&self, p: PortId) -> Option<CompactF32Layout> {
        (self.supported
            && self.canonical_input != Some(p)
            && (1..=4).contains(&self.index)
            && (p == PortId(0) || self.index == 4 && p == PortId(1)))
        .then_some(self.input_layout)
    }
    fn compact_f32_output(&self) -> Option<(PortId, CompactF32Layout)> {
        (self.supported && (1..=4).contains(&self.index)).then_some((PortId(0), SHAPE))
    }
    fn prepared_output(&self, _: PortId) -> Option<&[u8]> {
        Some(&self.bytes[..self.length])
    }
    fn step(&mut self, io: &mut StepIo<2>, input: &StepInputBytes<'_, 2>) -> StepOutcome {
        self.staged = false;
        if self.finished {
            return StepOutcome::Complete;
        }
        if self.delay > 0 {
            self.delay -= 1;
            io.consume_fuel(io.remaining_fuel()).unwrap();
            return StepOutcome::Yield;
        }
        if self.index == 0 {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.send(PortId(0), self.source.unwrap()).unwrap();
            self.staged = true;
            return StepOutcome::Progress;
        }
        let Some(left) = decode(input, PortId(0)) else {
            return StepOutcome::Await;
        };
        if self.index == 5 {
            self.staged_result = Some(left);
            self.staged = true;
            io.consume(PortId(0)).unwrap();
            return StepOutcome::Progress;
        }
        let right = if self.index == 4 {
            let Some(right) = decode(input, PortId(1)) else {
                return StepOutcome::Await;
            };
            right
        } else {
            [0.; 2]
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if self.failure {
            return StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 77,
            });
        }
        let result: [f32; 2] = core::array::from_fn(|i| match self.index {
            1 => left[i] + 1.,
            2 => left[i] * 2.,
            3 => left[i] + 3.,
            4 => left[i] + right[i],
            _ => unreachable!(),
        });
        for (i, value) in result.iter().enumerate() {
            self.bytes[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        self.length = if io.compact_f32_output(PortId(0)).is_some() {
            io.send_compact_f32(PortId(0)).unwrap();
            8
        } else {
            self.bytes[8..].copy_from_slice(b"F32CANON");
            io.send_prepared(PortId(0), 16).unwrap();
            16
        };
        io.consume(PortId(0)).unwrap();
        if self.index == 4 {
            io.consume(PortId(1)).unwrap();
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.finished = true;
            self.result = self.staged_result.take();
            self.staged = false;
        }
    }
}
pub fn prepare(
    prepared: bool,
    delay: usize,
    failure: Option<usize>,
    unsupported: Option<usize>,
) -> Play {
    prepare_with_layout(prepared, delay, failure, unsupported, None)
}
pub fn prepare_with_layout(
    prepared: bool,
    delay: usize,
    failure: Option<usize>,
    unsupported: Option<usize>,
    mismatch: Option<CompactF32Layout>,
) -> Play {
    prepare_with_inputs(prepared, delay, failure, unsupported, mismatch, None)
}
pub fn prepare_canonical_input(node: usize, port: PortId) -> Play {
    prepare_with_inputs(true, 0, None, None, None, Some((node, port)))
}
fn prepare_with_inputs(
    prepared: bool,
    delay: usize,
    failure: Option<usize>,
    unsupported: Option<usize>,
    mismatch: Option<CompactF32Layout>,
    canonical: Option<(usize, PortId)>,
) -> Play {
    let mut inner = HostedValueStore::new(32, 16, 512).unwrap();
    if prepared {
        inner.prepare_compact_f32();
    }
    let mut values = Counted {
        inner,
        canonical: 0,
        compact: 0,
        physical: 0,
    };
    let mut source = [0; 16];
    source[8..].copy_from_slice(b"F32CANON");
    source[..4].copy_from_slice(&1.0f32.to_le_bytes());
    source[4..8].copy_from_slice(&(-0.0f32).to_le_bytes());
    let value = values.store(&source).unwrap();
    let edges = [
        (0, 0, 1, 0),
        (1, 0, 2, 0),
        (1, 0, 3, 0),
        (2, 0, 4, 0),
        (3, 0, 4, 1),
        (4, 0, 5, 0),
    ];
    let mut routes = FixedRoutes::<12, C>::new(2);
    for (node, start, len) in [(0, 0, 1), (1, 1, 2), (2, 3, 1), (3, 4, 1), (4, 5, 1)] {
        let targets: Vec<_> = (start..start + len)
            .map(|c| RouteTarget {
                cord: CordId(c as u16),
                sink: CordEndpoint::local(NodeId(edges[c].2), PortId(edges[c].3)),
            })
            .collect();
        routes
            .install(
                NodeId(node),
                PortId(0),
                RouteRange {
                    start: start as u16,
                    len: len as u16,
                },
                &targets,
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let inputs = [
        [None, None],
        [Some(CordId(0)), None],
        [Some(CordId(1)), None],
        [Some(CordId(2)), None],
        [Some(CordId(3)), Some(CordId(4))],
        [Some(CordId(5)), None],
    ];
    Play::new(
        core::array::from_fn(|n| NodeSpec {
            input_cords: inputs[n],
            maximum_step_fuel: 8,
        }),
        core::array::from_fn(|c| {
            let e = edges[c];
            CordSpec::local(
                CordId(c as u16),
                (NodeId(e.0), PortId(e.1)),
                (NodeId(e.2), PortId(e.3)),
                CordCapacity {
                    slot_start: c as u16,
                    item_capacity: 1,
                    byte_capacity: 16,
                    pressure_policy: Default::default(),
                },
            )
        }),
        routes,
        core::array::from_fn(|n| Driver {
            index: n,
            source: (n == 0).then_some(value),
            finished: false,
            staged: false,
            bytes: [0; 16],
            length: 0,
            result: None,
            staged_result: None,
            delay: if n == 5 { delay } else { 0 },
            failure: failure == Some(n),
            supported: unsupported != Some(n),
            canonical_input: canonical
                .filter(|(node, _)| *node == n)
                .map(|(_, port)| port),
            input_layout: if n == 2 {
                mismatch.unwrap_or(SHAPE)
            } else {
                SHAPE
            },
        }),
        values,
        HostedSignLog::new(4096, 4096 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
    )
    .unwrap()
}
pub fn finish(play: &mut Play) {
    for _ in 0..1000 {
        if play.step().unwrap() == SchedulerStatus::Drained {
            return;
        }
    }
    panic!("did not drain");
}
pub fn nodes() -> [NodeId; 4] {
    [NodeId(1), NodeId(2), NodeId(3), NodeId(4)]
}
