//! Numeric composition and all runtime storage admission before Play.
use super::*;
use conduitos_runtime_core::admission::{Admission, BackKind};

pub(super) fn prepare(
    prepared: &PreparedNativeWorkset,
    evidence: Option<&conduit_body::BodyBiographyEvidence>,
) -> Result<NativeWorksetPlay, WorksetRefusal> {
    let _native_timing =
        crate::native_observation::Interval::begin(crate::native_observation::Stage::KernelPrepare);
    crate::native_observation::Progress::result(
        crate::native_observation::Phase::KernelPrepare,
        "native-workset",
        || prepare_inner(prepared, evidence),
    )
}

fn prepare_inner(
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
    let mut admission = Admission::new(prepared.lowered.nodes, prepared.lowered.cords);
    let mut bindings = [None; NODES];
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    let mut editors = core::array::from_fn(|_| None);
    let mut keymaps = core::array::from_fn(|_| PreparedKeymap::default_layout());
    let mut admitted_editors = [false; PLOTS];
    let mut admitted_encounters = [false; PLOTS];
    let mut operation_count = 0;
    for (plot, part) in parts.iter().enumerate() {
        let fragment = &prepared.plan.plots[plot].plan.fragments[0];
        if part.identity.plan_id != fragment.plan_id
            || part.identity.fragment_id != fragment.fragment_id
            || part.nodes.len() != fragment.placements.len()
            || !super::super::editor_delivery::partition_shape(fragment)?
        {
            return Err(WorksetRefusal::Plan);
        }
        for node in &part.nodes {
            if usize::from(node.node.0) != operation_count {
                return Err(WorksetRefusal::Lowering);
            }
            let placement = fragment
                .placements
                .iter()
                .find(|placement| placement.placement_id == node.placement_id)
                .ok_or(WorksetRefusal::Plan)?;
            let (effect, operation) = match placement.implementation_id.as_str() {
                super::super::keyboard_delivery::IMPLEMENTATION => {
                    (Effect::Keyboard, BackKind::Keyboard)
                }
                super::super::application_delivery::EVENT_IMPLEMENTATION => {
                    (Effect::ApplicationEvent, BackKind::Keyboard)
                }
                super::super::application_delivery::STATE_IMPLEMENTATION => {
                    (Effect::Application, BackKind::Application)
                }
                super::super::application_delivery::FACE_IMPLEMENTATION => {
                    (Effect::ApplicationFace, BackKind::Face)
                }
                crate::keyboard_text_plan::KEYMAP_IMPLEMENTATION => {
                    // This Back has no selected-layout Host Call. Refuse a
                    // wired selection rather than silently keep the startup layout.
                    if fragment.connections.iter().any(|cord| {
                        cord.sink_placement_id == placement.placement_id
                            && cord.sink_port_id.as_str() == "selection"
                    }) {
                        return Err(WorksetRefusal::Plan);
                    }
                    let layout = placement
                        .configuration
                        .iter()
                        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
                            ("layout", conduit_core::ConfigurationValue::Structured(layout)) => {
                                Some(layout)
                            }
                            _ => None,
                        })
                        .ok_or(WorksetRefusal::Plan)?;
                    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
                    if layout != &conduit_human::default_keymap_configuration() {
                        // The protected domain admits only its source-generated static layout.
                        return Err(WorksetRefusal::Plan);
                    }
                    keymaps[plot] =
                        PreparedKeymap::new(layout).map_err(|_| WorksetRefusal::Plan)?;
                    (Effect::Keymap, BackKind::Keymap)
                }
                crate::offer::TEXT_UPPER_IMPLEMENTATION => (Effect::Upper, BackKind::Upper),
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
                    if admitted_editors[plot] {
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
                    admitted_editors[plot] = true;
                    // Edit is an ordinary one-input/one-output transform; the
                    // installed host binding gives it retained text semantics.
                    (Effect::Edit, BackKind::Edit)
                }
                super::super::document_state::IMPLEMENTATION => {
                    let maximum = super::super::document_state::validate(placement)?;
                    if admitted_editors[plot] {
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
                    let _ = maximum; // Exact protected owner configuration is prepared separately.
                    admitted_editors[plot] = true;
                    (Effect::Document, BackKind::Document)
                }
                super::super::editor_delivery::IMPLEMENTATION => {
                    super::super::editor_delivery::validate(placement)?;
                    if admitted_encounters[plot] {
                        return Err(WorksetRefusal::Plan);
                    }
                    admitted_encounters[plot] = true;
                    (Effect::Editor, BackKind::Editor)
                }
                crate::offer::TEXT_FACE_IMPLEMENTATION => {
                    if conduit_semantic_catalog::text_face_insertion_caret(&placement.configuration)
                    {
                        return Err(WorksetRefusal::Plan);
                    }
                    (Effect::Face, BackKind::Face)
                }
                _ => return Err(WorksetRefusal::Capability),
            };
            bindings[operation_count] = Some(Binding {
                plot: plot as u8,
                effect,
            });
            admission.backs[operation_count] = operation;
            operation_count += 1;
        }
    }
    if operation_count != usize::from(admission.active_nodes) {
        return Err(WorksetRefusal::Lowering);
    }
    for (target, spec) in admission
        .nodes
        .iter_mut()
        .zip(parts.iter().flat_map(|part| &part.node_specs))
    {
        *target = *spec;
    }
    for (target, cord) in admission
        .cords
        .iter_mut()
        .zip(parts.iter().flat_map(|part| &part.cords))
    {
        *target = cord.spec;
    }
    for route in parts.iter().flat_map(|part| &part.routes) {
        admission
            .install_route(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| WorksetRefusal::Kernel)?;
    }
    for operation in parts.iter().flat_map(|part| &part.host_calls) {
        let destination = admission
            .bindings
            .get_mut(usize::from(operation.node.0))
            .ok_or(WorksetRefusal::Kernel)?;
        if destination.replace(operation.binding).is_some() {
            return Err(WorksetRefusal::Kernel);
        }
    }
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    let scheduler =
        crate::protected_runtime::Scheduler::new(admission).map_err(|_| WorksetRefusal::Kernel)?;
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    let scheduler = admission.build().map_err(|_| WorksetRefusal::Kernel)?;
    let initial = super::super::application_delivery::initial(prepared, evidence)?;
    let applications = NativeApplications::new(initial).map_err(WorksetRefusal::Application)?;
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
        keymaps,
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        editors,
        pending: [None; PLOTS],
        held: [None; 256],
        faces: [None; PLOTS],
        editor_observations: core::array::from_fn(|_| None),
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
