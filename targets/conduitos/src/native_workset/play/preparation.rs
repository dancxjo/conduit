//! Numeric composition and all runtime storage admission before Play.
use super::*;
use crate::keyboard_text_backs::{
    ApplicationBack, KeyboardBack, PresentationBack, StreamTransformBack,
};
use alloc::vec::Vec;
use conduit_kernel::{
    FixedHostCallBindings, FixedRoutes, ValueStorage,
    scheduler::{CordSpec, NodeSpec},
};

pub(super) fn prepare(
    prepared: &PreparedNativeWorkset,
    evidence: Option<&conduit_body::BodyBiographyEvidence>,
) -> Result<NativeWorksetPlay, WorksetRefusal> {
    let parts = &prepared.lowered.partitions;
    let count = parts.len();
    if count == 0
        || count > PLOTS
        || count != prepared.plan.plots.len()
        || usize::from(prepared.lowered.nodes) > NODES
        || usize::from(prepared.lowered.cords) > CORDS
    {
        return Err(WorksetRefusal::Kernel);
    }
    let mut values =
        FixedValueStore::<10, 3072>::new(24_576).map_err(|_| WorksetRefusal::Kernel)?;
    let empty = values.store(&[]).map_err(|_| WorksetRefusal::Kernel)?;
    let mut bindings = [None; NODES];
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    let mut editors = core::array::from_fn(|_| None);
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    let editors = core::array::from_fn(|_| None);
    let mut operations = Vec::with_capacity(NODES);
    for (plot, part) in parts.iter().enumerate() {
        let fragment = &prepared.plan.plots[plot].plan.fragments[0];
        if part.identity.plan_id != fragment.plan_id
            || part.identity.fragment_id != fragment.fragment_id
            || part.nodes.len() != fragment.placements.len()
            || !matches!(part.nodes.len(), 3 | 4)
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
                    PlannedBack::Keyboard(KeyboardBack {
                        empty,
                        pending: None,
                        next: 0,
                        maximum: None,
                    }),
                ),
                super::super::application_delivery::EVENT_IMPLEMENTATION => (
                    Effect::ApplicationEvent,
                    PlannedBack::Keyboard(KeyboardBack {
                        empty,
                        pending: None,
                        next: 0,
                        maximum: None,
                    }),
                ),
                super::super::application_delivery::STATE_IMPLEMENTATION => (
                    Effect::Application,
                    PlannedBack::Application(ApplicationBack {
                        empty: values.store(&[]).map_err(|_| WorksetRefusal::Kernel)?,
                        pending: None,
                        next: 0,
                        initial: true,
                    }),
                ),
                super::super::application_delivery::PRESENTATION_IMPLEMENTATION => (
                    Effect::ApplicationPresentation,
                    PlannedBack::Presentation(PresentationBack {
                        pending: None,
                        next: 0,
                    }),
                ),
                crate::keyboard_text_plan::KEYMAP_IMPLEMENTATION => (
                    Effect::Keymap,
                    PlannedBack::Keymap(StreamTransformBack::new(true)),
                ),
                crate::offer::TEXT_UPPER_IMPLEMENTATION => (
                    Effect::Upper,
                    PlannedBack::Upper(StreamTransformBack::new(false)),
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
                    if editors[plot].is_some() {
                        return Err(WorksetRefusal::Plan);
                    }
                    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
                    {
                        editors[plot] = Some(
                            BoundedTextState::new(
                                conduit_semantic_catalog::TextStateMode::Edit,
                                maximum,
                            )
                            .map_err(|_| WorksetRefusal::Plan)?,
                        );
                    }
                    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
                    if maximum == 0 || maximum > 256 {
                        return Err(WorksetRefusal::Plan);
                    }
                    // Edit is an ordinary one-input/one-output transform; the
                    // installed host binding gives it retained text semantics.
                    (
                        Effect::Edit,
                        PlannedBack::TextEdit(StreamTransformBack::new(false)),
                    )
                }
                crate::offer::TEXT_PRESENTATION_IMPLEMENTATION => (
                    Effect::Presentation,
                    PlannedBack::Presentation(PresentationBack {
                        pending: None,
                        next: 0,
                    }),
                ),
                _ => return Err(WorksetRefusal::Capability),
            };
            bindings[operations.len()] = Some(Binding {
                plot: plot as u8,
                effect,
            });
            operations.push(operation);
        }
    }
    while operations.len() < NODES {
        operations.push(PlannedBack::Upper(StreamTransformBack::new(false)));
    }
    let drivers = operations
        .into_iter()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| WorksetRefusal::Kernel)?;
    let mut nodes = [NodeSpec {
        input_cords: [None; PORTS],
        maximum_step_fuel: 1,
    }; NODES];
    for (target, spec) in nodes
        .iter_mut()
        .zip(parts.iter().flat_map(|part| &part.node_specs))
    {
        *target = *spec;
    }
    let mut cords = [CordSpec::inactive(); CORDS];
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
    let mut host_bindings = FixedHostCallBindings::<NODES>::new(1);
    for operation in parts.iter().flat_map(|part| &part.host_calls) {
        host_bindings
            .install(operation.node, operation.binding)
            .map_err(|_| WorksetRefusal::Kernel)?;
    }
    host_bindings.seal().map_err(|_| WorksetRefusal::Kernel)?;
    let signs =
        FixedSignLog::<SIGN_ITEMS>::new((SIGN_ITEMS * core::mem::size_of::<KernelEvent>()) as u32)
            .map_err(|_| WorksetRefusal::Kernel)?;
    let scheduler = Scheduler::new_with_active_counts_and_host_calls(
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
    let mut applications = core::array::from_fn(|_| None);
    for (plot, application) in applications.iter_mut().enumerate().take(count) {
        let planned = &prepared.plan.plots[plot];
        if !planned.plan.fragments[0]
            .placements
            .iter()
            .any(|placement| {
                placement.implementation_id.as_str()
                    == super::super::application_delivery::STATE_IMPLEMENTATION
            })
        {
            continue;
        }
        let placement = planned.plan.fragments[0]
            .placements
            .iter()
            .find(|placement| {
                placement.implementation_id.as_str()
                    == super::super::application_delivery::STATE_IMPLEMENTATION
            })
            .ok_or(WorksetRefusal::Plan)?;
        let selected = super::super::tutorial_application::selected_application(placement)?;
        *application = Some(match selected {
            "tutorial" => NativeApplication::Tutorial(
                super::super::tutorial_application::TutorialApplication::prepare(
                    evidence.ok_or(WorksetRefusal::Plan)?,
                    conduit_tutorial_plot::TutorialPlayback::Playing,
                )?,
            ),
            "tour" => NativeApplication::Tour(Box::new(
                conduit_tour_model::TourApplicationPort::canonical(),
            )),
            "patchbay" => NativeApplication::Patchbay(
                super::super::application_delivery::PatchbayTargets::prepare(prepared, plot)?,
            ),
            _ => return Err(WorksetRefusal::Plan),
        });
    }
    Ok(NativeWorksetPlay {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        pure_results: core::array::from_fn(|_| None),
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        protected: core::array::from_fn(|_| None),
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        protection_admissions: core::array::from_fn(|index| {
            prepared.protected_text.get(index).cloned().flatten()
        }),
        scheduler: Box::new(scheduler),
        bindings,
        keymaps: core::array::from_fn(|_| ConduitIntlKeymap::new()),
        editors,
        pending: [None; PLOTS],
        held: [None; 256],
        presentations: [None; PLOTS],
        application_views: core::array::from_fn(|_| None),
        applications,
        application_requests: core::array::from_fn(|_| None),
        input_owners: core::array::from_fn(|index| prepared.input_owners.get(index).cloned()),
        plot_count: count,
        cancelled: false,
        admitted_plan: prepared.plan.plan_id.clone(),
        active_body_play: None,
    })
}
