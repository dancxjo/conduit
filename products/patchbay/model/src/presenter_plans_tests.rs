use crate::{
    patchbay_presenter_plans, portable_demonstration, InteractionDisposition, PatchbayInteraction,
    PatchbayInteractionRequest, PATCHBAY_PRESENTATION_KIND,
};
use conduit_core::{BootId, HostId};
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, FixedRoutes, HostOperationDisposition,
    HostOperationId, HostOperationOutcome, HostedSignLog, HostedValueStore, KernelEventKind,
    Operation, OperationAction, OperationInput, RequestId, ValueRef, ValueStorage,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, FIXED_KERNEL_STORAGE_PORTS_PER_NODE};

const DIRECT_NODES: usize = 2;
const DIRECT_CORDS: usize = 1;
const RECURSIVE_NODES: usize = 25;
const RECURSIVE_CORDS: usize = 18;
const SIGN_ITEMS: u16 = 128;

#[derive(Debug)]
struct PresentLeaf {
    input: ValueRef,
    host_operation: Option<HostOperationId>,
    pending: bool,
}

impl Operation for PresentLeaf {
    fn start(&mut self) -> OperationAction {
        let Some(operation) = self.host_operation else {
            return OperationAction::Complete;
        };
        self.pending = true;
        OperationAction::RequestHostOperation {
            request: RequestId(0),
            operation,
            input: BoundedValueRef::new(self.input, 1).unwrap(),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { outcome, .. }
                if self.pending && outcome.disposition == HostOperationDisposition::Completed =>
            {
                self.pending = false;
                OperationAction::Complete
            }
            _ => OperationAction::Await,
        }
    }

    fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

#[test]
fn unchanged_patchbay_meaning_has_distinct_truthful_direct_and_recursive_plans() {
    let proof = patchbay_presenter_plans().unwrap();
    assert_eq!(
        proof.direct.source_document_id,
        proof.recursive.source_document_id
    );
    assert_eq!(
        proof.direct.checked_form_id,
        proof.recursive.checked_form_id
    );
    assert_ne!(
        proof.direct.expanded_form_id,
        proof.recursive.expanded_form_id
    );
    assert_ne!(proof.direct.plan_id, proof.recursive.plan_id);
    assert!(proof.direct.realization_backs.is_empty());
    assert_eq!(proof.recursive.realization_backs.len(), 4);
    assert_eq!(
        proof.direct.fragments[0].placements[0].kind_id.as_str(),
        PATCHBAY_PRESENTATION_KIND
    );
    assert_eq!(
        proof.direct.fragments[0].placements[0]
            .implementation_id
            .as_str(),
        "patchbay/direct/presentation-patchbay@1"
    );
    let recursive_kinds = proof.recursive.fragments[0]
        .placements
        .iter()
        .map(|placement| placement.kind_id.as_str())
        .collect::<Vec<_>>();
    for expected in [
        "layout/inset",
        "layout/column",
        "layout/align",
        "layout/stack",
        "presentation/frame",
        "graphics/rect",
        "graphics/text",
        "graphics/icon",
        "presentation/graphics",
    ] {
        assert!(recursive_kinds.contains(&expected));
    }
    assert!(proof.recursive.realization_backs.iter().any(|back| {
        back.kind_id.as_str() == PATCHBAY_PRESENTATION_KIND
            && back.invocation_path == "patchbay-capstone/canvas"
    }));
    assert!(conduit_core::verify_plan(&proof.direct));
    assert!(conduit_core::verify_plan(&proof.recursive));

    let mut tampered = proof.recursive.clone();
    tampered.realization_backs[0]
        .invocation_path
        .push_str("/invented");
    assert!(!conduit_core::verify_plan(&tampered));

    let mut tampered_fragment = proof.recursive.clone();
    tampered_fragment.fragments[0].realization_backs[0]
        .invocation_path
        .push_str("/invented");
    assert!(!conduit_core::verify_plan(&tampered_fragment));
}

#[test]
fn production_projection_keeps_recursive_forms_behind_stable_face_gears() {
    let presentation = crate::recursive_form_demonstration().unwrap();
    let recursive = presentation
        .subjects
        .iter()
        .filter(|subject| {
            presentation.properties.iter().any(|property| {
                property.subject == subject.identity
                    && property.name == "reviewed-back"
                    && property.value
                        == conduit_presentation::PresentationPropertyValue::Text("available".into())
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(recursive.len(), 4);
    assert!(recursive.iter().any(|subject| {
        presentation.relationships.iter().any(|relationship| {
            relationship.source == subject.identity
                && presentation.subjects.iter().any(|candidate| {
                    candidate.identity == relationship.target
                        && candidate.role == conduit_presentation::PresentationRole::Gear
                })
        })
    }));
    assert!(presentation.properties.iter().any(|property| {
        property.name == "collapsed-sink-port"
            && matches!(
                property.value,
                conduit_presentation::PresentationPropertyValue::Identity(_)
            )
    }));
    assert_eq!(
        presentation.basis.plan_id.as_ref(),
        Some(&crate::patchbay_presenter_plans().unwrap().recursive.plan_id)
    );
}

#[test]
fn both_shapes_lower_and_execute_through_the_production_kernel_with_bounded_signs() {
    let proof = patchbay_presenter_plans().unwrap();
    let direct = execute::<DIRECT_NODES, DIRECT_CORDS>(&proof.direct).unwrap();
    let recursive = execute::<RECURSIVE_NODES, RECURSIVE_CORDS>(&proof.recursive).unwrap();
    for signs in [&direct, &recursive] {
        assert!(signs.contains(&KernelEventKind::HostOperationRequested));
        assert!(signs.contains(&KernelEventKind::HostOperationCompleted));
        assert!(signs.contains(&KernelEventKind::OperationCompleted));
        assert!(signs.len() <= usize::from(SIGN_ITEMS));
    }
    assert!(recursive.len() > direct.len());
}

#[test]
fn unavailable_direct_renderer_and_missing_recursive_leaf_fail_differently_without_form_mutation() {
    let proof = patchbay_presenter_plans().unwrap();
    let source = proof.direct.source_document_id.clone();
    let checked = proof.direct.checked_form_id.clone();

    let mut unavailable = proof.recursive_host.clone();
    unavailable.capabilities.retain(|offer| {
        matches!(
            offer.kind_id.as_str(),
            "presentation/source" | "presentation/sink"
        )
    });
    let direct_error = conduit_planner::default_expanded_placements(
        &proof.direct_expanded,
        core::slice::from_ref(&unavailable),
    )
    .unwrap_err();
    let recursive_error = conduit_planner::default_expanded_placements(
        &proof.recursive_expanded,
        core::slice::from_ref(&unavailable),
    )
    .unwrap_err();
    assert_ne!(direct_error.to_string(), recursive_error.to_string());
    assert_eq!(proof.recursive.source_document_id, source);
    assert_eq!(proof.recursive.checked_form_id, checked);
}

#[test]
fn both_shapes_use_the_same_portable_selection_seam_and_normalized_subjects() {
    let proof = patchbay_presenter_plans().unwrap();
    let base = portable_demonstration().unwrap();
    let presentation = |plan: &conduit_core::Plan| {
        let mut basis = base.basis.clone();
        basis.source_document_id = Some(plan.source_document_id.clone());
        basis.checked_form_id = Some(plan.checked_form_id.clone());
        basis.expanded_form_id = Some(plan.expanded_form_id.clone());
        basis.plan_id = Some(plan.plan_id.clone());
        basis.active_play_id = None;
        basis.sign_ids.clear();
        conduit_presentation::Presentation::new_with_semantics(
            base.revision,
            basis,
            base.subjects.clone(),
            base.relationships.clone(),
            base.properties.clone(),
            base.text.clone(),
            base.actions.clone(),
            base.disclosures.clone(),
        )
        .unwrap()
    };
    let direct = presentation(&proof.direct);
    let recursive = presentation(&proof.recursive);
    assert_eq!(direct.subjects, recursive.subjects);
    assert_eq!(direct.relationships, recursive.relationships);
    assert_eq!(direct.properties, recursive.properties);
    assert_eq!(direct.text, recursive.text);

    let execute = |presentation: &conduit_presentation::Presentation, suffix: &str| {
        let mut interaction = PatchbayInteraction::new(
            HostId::from(format!("interaction-{suffix}")),
            BootId::from(format!("interaction-{suffix}-boot")),
        );
        let request = PatchbayInteractionRequest::select(
            interaction.next_request_id("select").unwrap(),
            &crate::PatchbaySubjectRef {
                expanded_form_id: presentation.basis.expanded_form_id.clone().unwrap(),
                subject_identity: presentation.subjects[0].identity.clone(),
            },
        )
        .unwrap();
        interaction
            .execute_presentation(presentation, request, |_| unreachable!())
            .unwrap()
    };
    let direct_receipt = execute(&direct, "direct");
    let recursive_receipt = execute(&recursive, "recursive");
    assert_eq!(
        direct_receipt.disposition,
        InteractionDisposition::Succeeded
    );
    assert_eq!(
        recursive_receipt.disposition,
        InteractionDisposition::Succeeded
    );
    assert_eq!(
        direct_receipt
            .signs
            .iter()
            .map(|sign| sign.kind)
            .collect::<Vec<_>>(),
        recursive_receipt
            .signs
            .iter()
            .map(|sign| sign.kind)
            .collect::<Vec<_>>()
    );
}

fn execute<const NODES: usize, const CORDS: usize>(
    plan: &conduit_core::Plan,
) -> Result<Vec<KernelEventKind>, String> {
    let fragment = plan
        .fragments
        .first()
        .ok_or_else(|| "Plan has no fragment".to_string())?;
    let lowered = lower_plan_fragment(fragment).map_err(|error| format!("lower: {error:?}"))?;
    if lowered.nodes.len() != NODES || lowered.cords.len() != CORDS {
        return Err(format!(
            "unexpected capstone shape: nodes={} cords={}",
            lowered.nodes.len(),
            lowered.cords.len()
        ));
    }
    let capacity = u16::try_from(NODES).map_err(|_| "node capacity overflow".to_string())?;
    let byte_capacity = u32::try_from(NODES).map_err(|_| "node bytes overflow".to_string())?;
    let mut values = HostedValueStore::new(capacity, 1, byte_capacity)
        .map_err(|error| format!("value store: {error:?}"))?;
    let mut prepared = Vec::with_capacity(NODES);
    for node in &lowered.nodes {
        let input = values
            .store(&[0])
            .map_err(|error| format!("input: {error:?}"))?;
        let host_operation = lowered
            .host_operations
            .iter()
            .find(|operation| operation.node == node.node)
            .map(|operation| operation.operation);
        prepared.push(
            OperationDriver::new(PresentLeaf {
                input,
                host_operation,
                pending: false,
            })
            .map_err(|error| format!("driver: {error:?}"))?,
        );
    }
    let drivers: [OperationDriver<PresentLeaf, FIXED_KERNEL_STORAGE_PORTS_PER_NODE>; NODES] =
        prepared
            .try_into()
            .map_err(|_| "driver count changed".to_string())?;
    let nodes = lowered
        .node_specs
        .clone()
        .try_into()
        .map_err(|_| "node count changed".to_string())?;
    let cords = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "Cord count changed".to_string())?;
    let mut routes = FixedRoutes::<512, 256>::new(FIXED_KERNEL_STORAGE_PORTS_PER_NODE as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|error| format!("route: {error:?}"))?;
    }
    routes
        .seal()
        .map_err(|error| format!("routes: {error:?}"))?;
    let mut bindings = FixedHostOperationBindings::<NODES>::new(1);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|error| format!("binding: {error:?}"))?;
    }
    bindings
        .seal()
        .map_err(|error| format!("bindings: {error:?}"))?;
    let sign_bytes = u32::from(SIGN_ITEMS)
        * u32::try_from(core::mem::size_of::<conduit_kernel::KernelEvent>()).unwrap();
    let signs =
        HostedSignLog::new(SIGN_ITEMS, sign_bytes).map_err(|error| format!("signs: {error:?}"))?;
    let mut scheduler =
        FixedScheduler::<
            _,
            _,
            _,
            NODES,
            CORDS,
            FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            128,
            512,
            256,
            NODES,
            NODES,
        >::new_with_host_operations(nodes, cords, routes, bindings, drivers, values, signs)
        .map_err(|error| format!("scheduler: {error:?}"))?;
    for _ in 0..64 {
        while let Some(request) = scheduler.next_host_request() {
            scheduler
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                )
                .map_err(|error| format!("complete: {error:?}"))?;
        }
        match scheduler
            .step()
            .map_err(|error| format!("step: {error:?}"))?
        {
            SchedulerStatus::Complete => {
                return Ok(scheduler.signs().events().map(|event| event.kind).collect())
            }
            SchedulerStatus::Progress { .. } | SchedulerStatus::Idle => {}
            SchedulerStatus::Cancelled => return Err("capstone cancelled".into()),
        }
    }
    Err("capstone exceeded decision bound".into())
}
