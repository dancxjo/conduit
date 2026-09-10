//! Numeric composition and all runtime storage admission before Play.
use super::*;
use crate::keyboard_text_operations::{
    KeyboardOperation, PresentationOperation, StreamTransformOperation,
};
use alloc::vec::Vec;
use conduit_kernel::{
    CordEndpoint, CordId, FixedHostOperationBindings, FixedRoutes, PortId, ValueStorage,
    scheduler::{CordSpec, NodeSpec},
};

pub(super) fn prepare(
    prepared: &PreparedNativeWorkset,
) -> Result<NativeWorksetPlay, WorksetRefusal> {
    let parts = &prepared.lowered.partitions;
    let count = parts.len();
    if count == 0
        || count > FORMS
        || count != prepared.plan.forms.len()
        || usize::from(prepared.lowered.nodes) > NODES
        || usize::from(prepared.lowered.cords) > CORDS
    {
        return Err(WorksetRefusal::Kernel);
    }
    let mut values = FixedValueStore::<64, 256>::new(16_384).map_err(|_| WorksetRefusal::Kernel)?;
    let empty = values.store(&[]).map_err(|_| WorksetRefusal::Kernel)?;
    let mut bindings = [None; NODES];
    let mut editors = core::array::from_fn(|_| None);
    let mut operations = Vec::with_capacity(NODES);
    for (form, part) in parts.iter().enumerate() {
        let fragment = &prepared.plan.forms[form].plan.fragments[0];
        if part.identity.plan_id != fragment.plan_id
            || part.identity.fragment_id != fragment.fragment_id
            || part.nodes.len() != 4
            || fragment.placements.len() != 4
        {
            return Err(WorksetRefusal::Plan);
        }
        for node in &part.nodes {
            if usize::from(node.node.0) != operations.len() {
                return Err(WorksetRefusal::Lowering);
            }
            let placement = fragment
                .placements
                .iter()
                .find(|placement| placement.placement_id == node.placement_id)
                .ok_or(WorksetRefusal::Plan)?;
            let (effect, operation) = match placement.implementation_id.as_str() {
                super::super::keyboard_delivery::IMPLEMENTATION => (
                    Effect::Keyboard,
                    PlannedOperation::Keyboard(KeyboardOperation {
                        empty,
                        pending: None,
                        next: 0,
                        maximum: None,
                    }),
                ),
                crate::keyboard_text_plan::KEYMAP_IMPLEMENTATION => (
                    Effect::Keymap,
                    PlannedOperation::Keymap(StreamTransformOperation::new(true)),
                ),
                crate::offer::TEXT_UPPER_IMPLEMENTATION => (
                    Effect::Upper,
                    PlannedOperation::Upper(StreamTransformOperation::new(false)),
                ),
                super::super::text_state::TEXT_EDIT_IMPLEMENTATION => {
                    let maximum = placement
                        .configuration
                        .iter()
                        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
                            ("maximum-bytes", conduit_core::ConfigurationValue::U64(value)) => {
                                usize::try_from(*value).ok()
                            }
                            _ => None,
                        })
                        .ok_or(WorksetRefusal::Plan)?;
                    if editors[form].is_some() {
                        return Err(WorksetRefusal::Plan);
                    }
                    editors[form] = Some(
                        BoundedTextState::new(
                            conduit_semantic_catalog::TextStateMode::Edit,
                            maximum,
                        )
                        .map_err(|_| WorksetRefusal::Plan)?,
                    );
                    // Edit is an ordinary one-input/one-output transform; the
                    // installed Host binding gives it retained text semantics.
                    (
                        Effect::Edit,
                        PlannedOperation::TextEdit(StreamTransformOperation::new(false)),
                    )
                }
                crate::offer::TEXT_PRESENTATION_IMPLEMENTATION => (
                    Effect::Presentation,
                    PlannedOperation::Presentation(PresentationOperation {
                        pending: None,
                        next: 0,
                    }),
                ),
                _ => return Err(WorksetRefusal::Capability),
            };
            bindings[operations.len()] = Some(Binding {
                form: form as u8,
                effect,
            });
            operations.push(operation);
        }
    }
    while operations.len() < NODES {
        operations.push(PlannedOperation::Upper(StreamTransformOperation::new(
            false,
        )));
    }
    let drivers = operations
        .into_iter()
        .map(|operation| OperationDriver::new(operation).map_err(|_| WorksetRefusal::Kernel))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| WorksetRefusal::Kernel)?;
    let mut nodes = [NodeSpec {
        input_cords: [None; PORTS],
        maximum_step_work: 1,
    }; NODES];
    for (target, spec) in nodes
        .iter_mut()
        .zip(parts.iter().flat_map(|part| &part.node_specs))
    {
        *target = *spec;
    }
    let mut cords = [CordSpec {
        cord: CordId(u16::MAX),
        source: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
        sink: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
        slot_start: u16::MAX,
        item_capacity: 0,
        byte_capacity: 0,
    }; CORDS];
    for (target, cord) in cords
        .iter_mut()
        .zip(parts.iter().flat_map(|part| &part.cords))
    {
        *target = cord.spec;
    }
    let mut routes = FixedRoutes::<{ NODES * PORTS }, CORDS>::new(PORTS as u16);
    for route in parts.iter().flat_map(|part| &part.routes) {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| WorksetRefusal::Kernel)?;
    }
    routes.seal().map_err(|_| WorksetRefusal::Kernel)?;
    let mut host_bindings = FixedHostOperationBindings::<NODES>::new(1);
    for operation in parts.iter().flat_map(|part| &part.host_operations) {
        host_bindings
            .install(operation.node, operation.binding)
            .map_err(|_| WorksetRefusal::Kernel)?;
    }
    host_bindings.seal().map_err(|_| WorksetRefusal::Kernel)?;
    let signs =
        FixedSignLog::<SIGN_ITEMS>::new((SIGN_ITEMS * core::mem::size_of::<KernelEvent>()) as u32)
            .map_err(|_| WorksetRefusal::Kernel)?;
    let scheduler = Scheduler::new_with_active_counts_and_host_operations(
        usize::from(prepared.lowered.nodes),
        usize::from(prepared.lowered.cords),
        nodes,
        cords,
        routes,
        host_bindings,
        drivers,
        values,
        signs,
    )
    .map_err(|_| WorksetRefusal::Kernel)?;
    Ok(NativeWorksetPlay {
        scheduler,
        bindings,
        keymaps: core::array::from_fn(|_| ConduitIntlKeymap::new()),
        editors,
        pending: [None; FORMS],
        held: [None; 256],
        presentations: [None; FORMS],
        form_count: count,
        cancelled: false,
    })
}
