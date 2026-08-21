//! Browser-installed identities and execution for the portable presentation nucleus.

use conduit_core::ConnectionBase;
use conduit_kernel::scheduler::{
    FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, ValueStorage,
};
use conduit_planner::{default_placements, plan_with_options, PlanningOptions};
use conduit_presentation::{
    GraphicsScene, LayoutFrame, PresentationComposition, MAX_LAYOUT_FRAME_BYTES,
    MAX_PRESENTATION_COMPOSITION_BYTES,
};
use conduit_runtime::lowering::{lower_plan_fragment, MAXIMUM_KERNEL_PORTS_PER_NODE};
use std::collections::BTreeMap;

mod abi;
pub use abi::*;
mod offers;
pub use offers::offers;
mod operation;
use operation::NucleusOperation;
mod structured_execution;
mod text_execution;
use offers::{advertisement, fixture_catalog, fixture_startup_catalog};
use text_execution::execute_text_form;
pub(crate) use text_execution::uppercase_utf8;
#[cfg(test)]
mod text_lab_tests;

pub use conduit_std_catalog::{BROWSER_PRESENTATION_ARTIFACT, BROWSER_PRESENTATION_PROFILE};
const FIXTURE_GRAPHICS_KIND: &str = "browser-fixture/graphics-present";
const FIXTURE_LAYOUT_KIND: &str = "browser-fixture/layout-present";
const FIXTURE_TEXT_KIND: &str = "browser-fixture/text-source";
const FIXTURE_PRESENT_OPERATION: &str = "browser.host/presentation-nucleus-present@1";
const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const MAX_NODES: usize = 6;
const MAX_CORDS: usize = 5;
const ROUTE_SLOTS: usize = MAX_NODES * PORTS;
const HOST_BINDINGS: usize = MAX_NODES * MAX_NODES;
const VALUE_SLOTS: usize = 16;
const MAX_VALUE_BYTES: usize = MAX_PRESENTATION_COMPOSITION_BYTES;
const VALUE_BYTES: usize = VALUE_SLOTS * MAX_VALUE_BYTES;
const SIGN_ITEMS: usize = 128;

const GRAPHICS_FORM: &str = r#"form browser-graphics-nucleus {
 icon: presentation/icon(icon = "presentation", accessibility-name = "Patchbay")
 frame: presentation/frame(role = "panel", accessibility-name = "Gear Face")
 rect: graphics/rect(style = "stroke")
 text: graphics/text(text = "ready")
 glyph: graphics/icon(icon = "presentation")
 present: browser-fixture/graphics-present
 icon > frame > rect > text > glyph > present
}"#;

const LAYOUT_FORM: &str = r#"form browser-layout-nucleus {
 viewport: layout/viewport(width = 320, height = 200, children = 3, child-width = 40, child-height = 30)
 row: layout/row(gap = 4)
 column: layout/column(gap = 3)
 stack: layout/stack
 align: layout/align(horizontal = "center", vertical = "end")
 present: browser-fixture/layout-present
 viewport > row > column > stack > align > present
}"#;

type NucleusScheduler = FixedScheduler<
    OperationDriver<NucleusOperation, PORTS>,
    FixedValueStore<VALUE_SLOTS, MAX_VALUE_BYTES>,
    FixedSignLog<SIGN_ITEMS>,
    MAX_NODES,
    MAX_CORDS,
    PORTS,
    MAX_CORDS,
    ROUTE_SLOTS,
    MAX_CORDS,
    HOST_BINDINGS,
    MAX_NODES,
>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserNucleusProof {
    pub graphics: GraphicsScene,
    pub layout: LayoutFrame,
    pub text: String,
    pub graphics_plan_id: conduit_core::PlanId,
    pub layout_plan_id: conduit_core::PlanId,
    pub text_plan_id: conduit_core::PlanId,
    pub structured: conduit_presentation::StructuredSignPresentation,
    pub structured_plan_id: conduit_core::PlanId,
}

pub fn execute_browser_nucleus() -> Result<BrowserNucleusProof, String> {
    let (graphics_bytes, graphics_plan_id) = execute_form(GRAPHICS_FORM, FIXTURE_GRAPHICS_KIND)?;
    let (layout_bytes, layout_plan_id) = execute_form(LAYOUT_FORM, FIXTURE_LAYOUT_KIND)?;
    let (text, text_plan_id) = execute_text_form()?;
    let (structured_sign, structured_plan_id) = structured_execution::execute()?;
    let structured = conduit_presentation::StructuredSignPresentation::from_sign(
        1,
        &structured_sign,
        &conduit_std_catalog::education_feedback_type(),
    )
    .map_err(|error| format!("project browser structured presentation: {error:?}"))?;
    Ok(BrowserNucleusProof {
        graphics: GraphicsScene::decode(&graphics_bytes)
            .map_err(|error| format!("decode browser graphics manifestation: {error:?}"))?,
        layout: LayoutFrame::decode(&layout_bytes)
            .map_err(|error| format!("decode browser layout manifestation: {error:?}"))?,
        text,
        graphics_plan_id,
        layout_plan_id,
        text_plan_id,
        structured,
        structured_plan_id,
    })
}

fn execute_form(source: &str, sink_kind: &str) -> Result<(Vec<u8>, conduit_core::PlanId), String> {
    let catalog = fixture_catalog()?;
    let startup = fixture_startup_catalog()?;
    let form = conduit_form::parse_with_startup(source, &startup, &catalog)
        .map_err(|error| format!("parse browser presentation Form: {error:?}"))?;
    let advertisement = advertisement();
    let hosts = [advertisement.clone()];
    let placements = default_placements(&form, &hosts)
        .map_err(|error| format!("place browser presentation Form: {error:?}"))?;
    let connection_byte_capacity = if sink_kind == FIXTURE_LAYOUT_KIND {
        MAX_LAYOUT_FRAME_BYTES
    } else {
        MAX_PRESENTATION_COMPOSITION_BYTES
    } as u32;
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|error| format!("plan browser presentation Form: {error:?}"))?;
    let fragment = plan
        .fragments
        .first()
        .ok_or_else(|| "browser presentation Plan has no fragment".to_string())?;
    let lowered = lower_plan_fragment(fragment)
        .map_err(|error| format!("lower browser presentation Plan: {error:?}"))?;
    let mut scheduler = prepare_scheduler(fragment, &lowered)?;
    let mut manifested = None;
    loop {
        if let Some(request) = scheduler.next_host_request() {
            let placement = fragment
                .placements
                .get(usize::from(request.node.0))
                .ok_or_else(|| "browser host request has no placement".to_string())?;
            let input = scheduler
                .host_value(request.input.value)
                .map_err(|error| format!("read browser host input: {error:?}"))?
                .to_vec();
            if placement.kind_id.as_str() == sink_kind {
                if manifested.replace(input).is_some() {
                    return Err("browser presentation manifested more than once".into());
                }
                complete(&mut scheduler, request, None)?;
            } else {
                let output = transform(placement, &input)?;
                let value = scheduler
                    .store_host_value(&output)
                    .map_err(|error| format!("store browser host output: {error:?}"))?;
                let bounded = BoundedValueRef::new(
                    value,
                    placement
                        .host_operations
                        .first()
                        .ok_or_else(|| {
                            "browser transform has no planned host operation".to_string()
                        })?
                        .maximum_output_bytes,
                )
                .map_err(|_| "browser host output exceeded its admitted bound".to_string())?;
                complete(&mut scheduler, request, Some(bounded))?;
            }
            continue;
        }
        match scheduler
            .step()
            .map_err(|error| format!("run browser presentation kernel: {error:?}"))?
        {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => break,
            SchedulerStatus::Idle => return Err("browser presentation kernel became idle".into()),
            SchedulerStatus::Cancelled => {
                return Err("browser presentation kernel was cancelled".into())
            }
        }
    }
    let output =
        manifested.ok_or_else(|| "browser presentation produced no manifestation".to_string())?;
    Ok((output, plan.plan_id))
}

fn prepare_scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_runtime::lowering::LoweredPlanFragment,
) -> Result<NucleusScheduler, String> {
    if fragment.placements.len() != MAX_NODES
        || fragment.connections.len() != MAX_CORDS
        || lowered.nodes.len() != MAX_NODES
        || lowered.cords.len() != MAX_CORDS
        || !lowered.remote_endpoints.is_empty()
    {
        return Err("browser presentation Plan has an unexpected finite shape".into());
    }
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| "browser presentation node table has the wrong size".to_string())?;
    let cords = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "browser presentation Cord table has the wrong size".to_string())?;
    let mut routes = FixedRoutes::<ROUTE_SLOTS, MAX_CORDS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(debug_error)?;
    }
    routes.seal().map_err(debug_error)?;
    let mut bindings = FixedHostOperationBindings::<HOST_BINDINGS>::new(MAX_NODES as u16);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(debug_error)?;
    }
    bindings.seal().map_err(debug_error)?;
    let mut values = FixedValueStore::<VALUE_SLOTS, MAX_VALUE_BYTES>::new(VALUE_BYTES as u32)
        .map_err(debug_error)?;
    let mut drivers = Vec::with_capacity(MAX_NODES);
    for placement in &fragment.placements {
        let operation = match placement.kind_id.as_str() {
            conduit_std_catalog::LAYOUT_VIEWPORT_KIND => {
                let value = conduit_std_catalog::execute_layout_source(placement)?;
                let encoded = value.encode();
                NucleusOperation::Source {
                    value: values
                        .store(&encoded[..value.encoded_len()])
                        .map_err(debug_error)?,
                    emitted: false,
                }
            }
            conduit_std_catalog::PRESENTATION_ICON_KIND => {
                let value = conduit_std_catalog::execute_presentation_source(placement)?;
                let encoded = value.encode();
                NucleusOperation::Source {
                    value: values
                        .store(&encoded[..value.encoded_len()])
                        .map_err(debug_error)?,
                    emitted: false,
                }
            }
            FIXTURE_GRAPHICS_KIND | FIXTURE_LAYOUT_KIND => NucleusOperation::Sink {
                maximum_input_bytes: placement.host_operations[0].maximum_input_bytes,
                pending: false,
                complete: false,
            },
            _ => NucleusOperation::Transform {
                maximum_input_bytes: placement.host_operations[0].maximum_input_bytes,
                pending: false,
                emitted: false,
            },
        };
        drivers.push(OperationDriver::new(operation).map_err(debug_error)?);
    }
    let drivers = drivers
        .try_into()
        .map_err(|_| "browser presentation driver table has the wrong size".to_string())?;
    let signs = FixedSignLog::<SIGN_ITEMS>::new(
        lowered
            .sign_bytes
            .max((SIGN_ITEMS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32),
    )
    .map_err(debug_error)?;
    FixedScheduler::new_with_host_operations(nodes, cords, routes, bindings, drivers, values, signs)
        .map_err(debug_error)
}

fn transform(placement: &conduit_core::PlannedGear, input: &[u8]) -> Result<Vec<u8>, String> {
    match placement.kind_id.as_str() {
        conduit_std_catalog::LAYOUT_ROW_KIND
        | conduit_std_catalog::LAYOUT_COLUMN_KIND
        | conduit_std_catalog::LAYOUT_STACK_KIND
        | conduit_std_catalog::LAYOUT_INSET_KIND
        | conduit_std_catalog::LAYOUT_ALIGN_KIND => {
            let frame = LayoutFrame::decode(input)
                .map_err(|error| format!("decode browser layout input: {error:?}"))?;
            let output = conduit_std_catalog::execute_layout_transform(placement, frame)?;
            Ok(output.encode()[..output.encoded_len()].to_vec())
        }
        conduit_std_catalog::PRESENTATION_FRAME_KIND
        | conduit_std_catalog::PRESENTATION_BADGE_KIND => {
            let value = PresentationComposition::decode(input)
                .map_err(|error| format!("decode browser presentation input: {error:?}"))?;
            let output = conduit_std_catalog::execute_presentation_transform(placement, value)?;
            Ok(output.encode()[..output.encoded_len()].to_vec())
        }
        conduit_std_catalog::GRAPHICS_RECT_KIND => {
            let value = PresentationComposition::decode(input)
                .map_err(|error| format!("decode browser graphics composition: {error:?}"))?;
            encode_scene(conduit_std_catalog::execute_graphics_transform(
                placement,
                Some(value),
                None,
            )?)
        }
        conduit_std_catalog::GRAPHICS_TEXT_KIND | conduit_std_catalog::GRAPHICS_ICON_KIND => {
            let scene = GraphicsScene::decode(input)
                .map_err(|error| format!("decode browser graphics scene: {error:?}"))?;
            encode_scene(conduit_std_catalog::execute_graphics_transform(
                placement,
                None,
                Some(scene),
            )?)
        }
        _ => Err("browser presentation Host received an unsupported operation".into()),
    }
}

fn encode_scene(scene: GraphicsScene) -> Result<Vec<u8>, String> {
    Ok(scene.encode()[..scene.encoded_len()].to_vec())
}

fn complete(
    scheduler: &mut NucleusScheduler,
    request: HostOperationRequest,
    output: Option<BoundedValueRef>,
) -> Result<(), String> {
    scheduler
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output,
                failure: None,
            },
        )
        .map_err(debug_error)
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("browser presentation kernel: {error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_offers_preserve_semantic_faces_but_own_realization_identity() {
        let offers = offers();
        assert_eq!(offers.len(), 13);
        for offer in offers {
            let canonical = offers::canonical_offer(offer.kind_id.as_str()).unwrap();
            assert_eq!(
                offer.kind_contract_revision,
                canonical.kind_contract_revision
            );
            assert_eq!(offer.inputs, canonical.inputs);
            assert_eq!(offer.outputs, canonical.outputs);
            assert_eq!(offer.host_operations, canonical.host_operations);
            assert_eq!(offer.limits, canonical.limits);
            assert_eq!(
                offer.implementation.execution_profile_id.as_str(),
                BROWSER_PRESENTATION_PROFILE
            );
            assert_eq!(
                offer.implementation.artifact_id.as_str(),
                BROWSER_PRESENTATION_ARTIFACT
            );
        }
        let browser_upper = conduit_std_catalog::browser_text_upper_offer();
        let canonical_upper = conduit_std_catalog::text_upper_offer();
        assert_eq!(
            browser_upper.kind_contract_revision,
            canonical_upper.kind_contract_revision
        );
        assert_eq!(browser_upper.inputs, canonical_upper.inputs);
        assert_eq!(browser_upper.outputs, canonical_upper.outputs);
        assert_eq!(
            browser_upper.host_operations,
            canonical_upper.host_operations
        );
        assert_eq!(browser_upper.limits, canonical_upper.limits);
        assert_eq!(
            browser_upper.implementation.execution_profile_id.as_str(),
            conduit_std_catalog::BROWSER_TEXT_UPPER_PROFILE
        );
        assert_eq!(
            browser_upper.implementation.artifact_id.as_str(),
            conduit_std_catalog::BROWSER_TEXT_UPPER_ARTIFACT
        );
    }

    #[test]
    fn ordinary_browser_plans_execute_layout_and_graphics_through_the_kernel() {
        let proof = execute_browser_nucleus().expect("browser nucleus executes");
        assert_eq!(proof.graphics.commands().len(), 3);
        assert_eq!(proof.layout.child_count, 3);
        assert_eq!(proof.text, "STRASSE");
        assert_ne!(proof.graphics_plan_id, proof.layout_plan_id);
        assert_ne!(proof.layout_plan_id, proof.text_plan_id);
        assert_ne!(proof.text_plan_id, proof.structured_plan_id);
        assert!(proof.structured.presentation.text.is_empty());
        assert!(proof
            .structured
            .presentation
            .properties
            .iter()
            .any(|property| {
                property.name == "record-schema"
                    && property.value
                        == conduit_presentation::PresentationPropertyValue::Identity(
                            "education/feedback@1".into(),
                        )
            }));
        assert!(proof
            .structured
            .presentation
            .properties
            .iter()
            .any(|property| {
                property.name == "quantity-unit"
                    && property.value
                        == conduit_presentation::PresentationPropertyValue::Identity(
                            "ratio/percent".into(),
                        )
            }));
    }
}
