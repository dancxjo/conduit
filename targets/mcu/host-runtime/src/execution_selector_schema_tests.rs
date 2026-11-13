//! Standard Source selector through the actual MCU Driver and fixed scheduler.
use super::*;
use crate::execution_factory::Driver;
use alloc::{boxed::Box, vec};
use conduit_core::{
    kind_id, BorrowedStructuredValueValidator, StructuredFieldType as F, StructuredFieldValue as V,
    StructuredInfoType as T, StructuredInfoValue as I, StructuredSelector,
};
use conduit_kernel::{scheduler::*, *};
use conduit_plan_lowering::schema_value_store::SchemaValueStore;
struct End<'a> {
    source: bool,
    schema: &'a [u8],
    bytes: &'a [u8],
    done: bool,
    staged: bool,
}
impl StepBack<1> for End<'_> {
    fn canonical_input_schema(&self, _: PortId) -> Option<&[u8]> {
        (!self.source).then_some(self.schema)
    }
    fn canonical_output_schema(&self, _: PortId) -> Option<&[u8]> {
        self.source.then_some(self.schema)
    }
    fn prepared_output(&self, _: PortId) -> Option<&[u8]> {
        (self.source && self.staged).then_some(self.bytes)
    }
    fn step(&mut self, io: &mut StepIo<1>, input: &StepInputBytes<'_, 1>) -> StepOutcome {
        self.staged = false;
        if self.done {
            return StepOutcome::Complete;
        }
        if self.source {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.send_prepared(PortId(0), self.bytes.len() as u32)
                .unwrap();
        } else {
            if io.input(PortId(0)).is_none() {
                return StepOutcome::Await;
            }
            let parts = input.canonical_parts(PortId(0)).unwrap();
            assert_eq!(parts.schema, self.schema);
            assert_eq!(parts.node, &self.bytes[self.schema.len()..]);
            assert!(input.input(PortId(0)).is_none());
            io.consume(PortId(0)).unwrap();
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        self.done |= self.staged;
        self.staged = false;
    }
}
#[test]
fn linked_selector_consumes_and_publishes_schema_nodes_without_canonical_pool_space() {
    let u8_type = T::leaf(kind_id("value/u8")).unwrap();
    let boolean = T::leaf(kind_id("value/bool")).unwrap();
    let selected_type = T::record(
        kind_id("test/selected-reading"),
        vec![F::new("reading", u8_type.clone()).unwrap()],
    )
    .unwrap();
    let input_type = T::record(
        kind_id("test/selector-envelope"),
        vec![
            F::new("selected", selected_type.clone()).unwrap(),
            F::new("guard", boolean.clone()).unwrap(),
        ],
    )
    .unwrap();
    let selected = I::record(
        selected_type.clone(),
        vec![V::new("reading", I::leaf(u8_type, vec![42]).unwrap()).unwrap()],
    )
    .unwrap();
    let input = I::record(
        input_type.clone(),
        vec![
            V::new("selected", selected.clone()).unwrap(),
            V::new("guard", I::leaf(boolean, vec![1]).unwrap()).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let expected = selected.canonical_bytes().unwrap();
    let input_schema = input_type.canonical_bytes().unwrap();
    let output_schema = selected_type.canonical_bytes().unwrap();
    let selector = StructuredSelector::field(input_type, "selected").unwrap();
    let material = Box::leak(selector.canonical_bytes().unwrap().into_boxed_slice());
    let selector = SoftwareBack::selector_static(material, input.len(), expected.len()).unwrap();
    let values = SchemaValueStore::try_new(
        FixedValueStore::<2, 1>::new(1).unwrap(),
        &[
            (
                BorrowedStructuredValueValidator::new(&input_schema, input.len()).unwrap(),
                1,
            ),
            (
                BorrowedStructuredValueValidator::new(&output_schema, expected.len()).unwrap(),
                1,
            ),
        ],
        (input.len() + expected.len()) as u32,
        4096,
    )
    .unwrap();
    let drivers: [Driver<'_, End<'_>>; 3] = [
        Driver::Resident(End {
            source: true,
            schema: &input_schema,
            bytes: &input,
            done: false,
            staged: false,
        }),
        Driver::Software(selector),
        Driver::Resident(End {
            source: false,
            schema: &output_schema,
            bytes: &expected,
            done: false,
            staged: false,
        }),
    ];
    let mut routes = FixedRoutes::<3, 2>::new(1);
    for n in 0..2 {
        routes
            .install(
                NodeId(n),
                PortId(0),
                RouteRange { start: n, len: 1 },
                &[RouteTarget {
                    cord: CordId(n),
                    sink: CordEndpoint::local(NodeId(n + 1), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut kernel = FixedScheduler::<_, _, _, 3, 2, 1, 2, 3, 2, 3, 1>::new(
        core::array::from_fn(|n| NodeSpec {
            input_cords: [if n == 0 {
                None
            } else {
                Some(CordId(n as u16 - 1))
            }],
            maximum_step_fuel: 4,
        }),
        core::array::from_fn(|n| {
            CordSpec::local(
                CordId(n as u16),
                (NodeId(n as u16), PortId(0)),
                (NodeId(n as u16 + 1), PortId(0)),
                CordCapacity {
                    slot_start: n as u16,
                    item_capacity: 1,
                    byte_capacity: if n == 0 { input.len() } else { expected.len() } as u32,
                    pressure_policy: Default::default(),
                },
            )
        }),
        routes,
        drivers,
        values,
        FixedSignLog::<128>::new((128 * core::mem::size_of::<KernelEvent>()) as u32).unwrap(),
    )
    .unwrap();
    let (result, cost) =
        crate::member_return_tests::return_allocation_probe::observe(|| kernel.run(64));
    result.unwrap();
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert!(matches!(
        kernel.drivers()[2],
        Driver::Resident(End { done: true, .. })
    ));
    assert_eq!(kernel.values().used_items(), 0);
}
