//! CPU-only local Fore execution, using the actual assigned tables and Back.
use conduit_core as c;
use conduit_kernel::{scheduler::*, *};
use conduit_plot as p;

pub fn run<B: StepBack<1>>(back: B, assignment: crate::support::Assignment) {
    let (_, program, _) = crate::support::context();
    let input = frame(&program);
    let expected = program.evaluate(&input).unwrap();
    let ingress = assignment
        .fore
        .iter()
        .find(|f| f.direction() == conduit_core::AssignedForeDirection::Input)
        .unwrap();
    let egress = assignment
        .fore
        .iter()
        .find(|f| f.direction() == conduit_core::AssignedForeDirection::Output)
        .unwrap();
    let (_, cost) = crate::probe::observe(|| {
        let mut slot = Some(back);
        let backs = assignment
            .tables
            .prepare_backs(|node| {
                assert_eq!(node, NodeId(0));
                slot.take().ok_or(())
            })
            .unwrap();
        let signs = FixedSignLog::<128>::new_with_remote_storage(
            (128 * core::mem::size_of::<KernelEvent>()) as u32,
            64,
            remote_sign_storage_bytes(64).unwrap(),
        )
        .unwrap();
        let mut kernel: FixedScheduler<_, _, _, 1, 2, 1, 2, 1, 1> = FixedScheduler::new(
            assignment.tables.nodes,
            assignment.tables.cords,
            assignment.tables.routes,
            backs,
            FixedValueStore::<2, 2048>::new(4096).unwrap(),
            signs,
        )
        .unwrap();
        for sequence in 0..2 {
            kernel
                .admit_remote_input(
                    RemoteEndpointId(ingress.endpoint()),
                    CordId(ingress.cord()),
                    sequence,
                    &input,
                )
                .unwrap();
            kernel.step().unwrap();
            let offered = kernel
                .remote_egress_offer(RemoteEndpointId(egress.endpoint()), CordId(egress.cord()))
                .unwrap()
                .unwrap();
            assert_eq!(kernel.host_value(offered.value).unwrap(), expected);
            kernel
                .remote_egress_accept(
                    RemoteEndpointId(egress.endpoint()),
                    CordId(egress.cord()),
                    offered.sequence,
                )
                .unwrap();
            kernel
                .remote_egress_delivered(
                    RemoteEndpointId(egress.endpoint()),
                    CordId(egress.cord()),
                    offered.sequence,
                )
                .unwrap();
        }
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}
fn field(ty: &c::StructuredInfoType, name: &str) -> c::StructuredInfoType {
    let c::StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
pub fn frame(program: &p::PortableExpressionProgram) -> Vec<u8> {
    let ram = field(&program.input_type, "ram");
    let c::StructuredInfoTypeShape::Collection { element, .. } = ram.shape() else {
        panic!("collection")
    };
    c::StructuredInfoValue::record(
        program.input_type.clone(),
        vec![
            c::StructuredFieldValue::new(
                "address",
                c::StructuredInfoValue::leaf(field(&program.input_type, "address"), vec![112])
                    .unwrap(),
            )
            .unwrap(),
            c::StructuredFieldValue::new(
                "ram",
                c::StructuredInfoValue::collection(
                    ram.clone(),
                    (0..16)
                        .map(|n| c::StructuredInfoValue::leaf(element.clone(), vec![n]).unwrap())
                        .collect(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
