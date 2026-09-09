//! Exact keyboard Plan lowering and fixed storage admission before Play.
use super::*;

pub(super) fn prepare(
    prepared: &PreparedKeyboardTextPlay,
    event_count: Option<usize>,
) -> Result<KeyboardTextKernel, PreparationError> {
    if event_count.is_some_and(|count| count == 0 || count > MAXIMUM_INPUT_EVENTS) {
        return Err(PreparationError::PlanRejected);
    }
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(PreparationError::PlanRejected)?;
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment)
        .map_err(|_| PreparationError::LoweringRejected)?;
    validate_shape(fragment, &lowered)?;
    let keyboard_node = node_for(fragment, conduit_semantic_catalog::KEYBOARD_KIND)?;
    let keymap_node = node_for(fragment, conduit_semantic_catalog::KEYMAP_KIND)?;
    let upper_node = node_for(fragment, conduit_text::TEXT_UPPER_KIND)?;
    let presentation_node = node_for(fragment, conduit_semantic_catalog::TEXT_PRESENTATION_KIND)?;
    let mut values =
        FixedValueStore::<VALUE_SLOTS, MAX_VALUE_BYTES>::new(VALUE_BYTE_CAPACITY as u32)
            .map_err(|_| PreparationError::KernelRejected)?;
    let empty = values
        .store(&[])
        .map_err(|_| PreparationError::KernelRejected)?;
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| PreparationError::KernelRejected)?;
    let cords = [
        lowered.cords[0].spec,
        lowered.cords[1].spec,
        lowered.cords[2].spec,
    ];
    let mut routes = FixedRoutes::<ROUTE_SLOTS, ROUTE_TARGETS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| PreparationError::KernelRejected)?;
    }
    routes
        .seal()
        .map_err(|_| PreparationError::KernelRejected)?;
    let mut bindings = FixedHostOperationBindings::<HOST_BINDING_SLOTS>::new(MAX_NODES as u16);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|_| PreparationError::KernelRejected)?;
    }
    bindings
        .seal()
        .map_err(|_| PreparationError::KernelRejected)?;
    let mut drivers = [None, None, None, None];
    drivers[usize::from(keyboard_node.0)] = Some(
        OperationDriver::new(PlannedOperation::Keyboard(KeyboardOperation {
            empty,
            pending: None,
            next: 0,
            maximum: event_count.map(|count| count as u32),
        }))
        .map_err(|_| PreparationError::KernelRejected)?,
    );
    drivers[usize::from(keymap_node.0)] = Some(
        OperationDriver::new(PlannedOperation::Keymap(StreamTransformOperation::new(
            true,
        )))
        .map_err(|_| PreparationError::KernelRejected)?,
    );
    drivers[usize::from(upper_node.0)] = Some(
        OperationDriver::new(PlannedOperation::Upper(StreamTransformOperation::new(
            false,
        )))
        .map_err(|_| PreparationError::KernelRejected)?,
    );
    drivers[usize::from(presentation_node.0)] = Some(
        OperationDriver::new(PlannedOperation::Presentation(PresentationOperation {
            pending: None,
            next: 0,
        }))
        .map_err(|_| PreparationError::KernelRejected)?,
    );
    let [Some(first), Some(second), Some(third), Some(fourth)] = drivers else {
        return Err(PreparationError::KernelRejected);
    };
    let minimum_sign_bytes = (SIGN_CAPACITY * core::mem::size_of::<KernelEvent>()) as u32;
    let signs = FixedSignLog::<SIGN_CAPACITY>::new(lowered.sign_bytes.max(minimum_sign_bytes))
        .map_err(|_| PreparationError::KernelRejected)?;
    let scheduler = FixedScheduler::new_with_host_operations(
        nodes,
        cords,
        routes,
        bindings,
        [first, second, third, fourth],
        values,
        signs,
    )
    .map_err(|_| PreparationError::KernelRejected)?;
    Ok(KeyboardTextKernel {
        scheduler,
        keyboard_node,
        keymap_node,
        upper_node,
        presentation_node,
        keymap: ConduitIntlKeymap::new(),
    })
}
