//! One finite executable-book Form running through the ordinary browser Host stack.

mod abi;
mod offers;
mod operation;

pub use abi::*;

use conduit_core::{
    bind_active_play, bind_presentation, bind_sign, BaseImplementationId, ConfigurationValue,
    Plan, PlanFragment, PresentationIdentity, SignIdentity,
};
use conduit_kernel::scheduler::{FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, ValueStorage,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, FIXED_KERNEL_STORAGE_PORTS_PER_NODE};
use conduit_planner::{default_placements, plan_with_options, PlanningOptions};
use operation::BookOperation;
use serde::Serialize;
use std::collections::BTreeMap;

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const NODES: usize = 3;
const CORDS: usize = 2;
const ROUTES: usize = NODES * PORTS;
const VALUES: usize = 6;
const VALUE_BYTES: usize = conduit_text::MAXIMUM_MORSE_PATTERN_BYTES;
const SIGNS: usize = 64;

type BookScheduler = FixedScheduler<
    OperationDriver<BookOperation, PORTS>,
    FixedValueStore<VALUES, VALUE_BYTES>,
    FixedSignLog<SIGNS>,
    NODES,
    CORDS,
    PORTS,
    CORDS,
    ROUTES,
    CORDS,
    4,
    NODES,
>;

#[derive(Debug, Serialize)]
pub(super) struct IndicatorSegment {
    level: bool,
    units: u8,
}

#[derive(Debug, Serialize)]
pub(super) struct BookEffect {
    schema: &'static str,
    source_document_id: String,
    checked_form_id: String,
    expanded_form_id: String,
    plan_id: String,
    fragment_id: String,
    active_play_id: String,
    presentation_id: String,
    placement_id: String,
    host_id: String,
    boot_id: String,
    unit_millis: u16,
    segments: Vec<IndicatorSegment>,
}

#[derive(Debug, Serialize)]
pub(super) struct BookReceipt {
    schema: &'static str,
    disposition: &'static str,
    active_play_id: String,
    presentation_id: String,
    terminal_sign_id: String,
}

pub(super) struct BookSession {
    scheduler: BookScheduler,
    pending: HostOperationRequest,
    active_play_id: conduit_core::ActivePlayId,
    presentation: PresentationIdentity,
    host_id: conduit_core::HostId,
    boot_id: conduit_core::BootId,
}

impl BookSession {
    pub(super) fn prepare(
        host_id: &str,
        boot_id: &str,
        source: &str,
        play_sequence: u64,
    ) -> Result<(Self, BookEffect), String> {
        let (startup, catalog) = offers::catalog()?;
        let form = conduit_form::parse_with_startup(source, &startup, &catalog)
            .map_err(|error| format!("check executable-book Form: {error:?}"))?;
        let advertisement = offers::advertisement(host_id.into(), boot_id.into());
        let hosts = [advertisement];
        let placements = default_placements(&form, &hosts)
            .map_err(|error| format!("place executable-book Form: {error:?}"))?;
        let plan = plan_with_options(
            &form,
            &hosts,
            &placements,
            &[BaseImplementationId::from(offers::BOOK_LOCAL_BASE)],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(|error| format!("plan executable-book Form: {error:?}"))?;
        let fragment = exact_fragment(&plan)?;
        validate_shape(fragment)?;
        let active = bind_active_play(
            &plan.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            play_sequence,
        );
        let indicator = fragment
            .placements
            .iter()
            .find(|placement| {
                placement.kind_id.as_str()
                    == conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND
            })
            .ok_or_else(|| "executable-book Plan has no indicator placement".to_string())?;
        let presentation = bind_presentation(&active.active_play_id, &indicator.placement_id, 0);
        let lowered = lower_plan_fragment(fragment)
            .map_err(|error| format!("lower executable-book Plan: {error:?}"))?;
        let mut scheduler = prepare_scheduler(fragment, &lowered)?;
        let pending = drive_to_indicator(&mut scheduler, fragment)?;
        let encoded = scheduler
            .host_value(pending.input.value)
            .map_err(debug_error)?;
        let pattern = conduit_text::MorsePattern::decode(encoded)
            .map_err(|error| format!("decode planned indicator effect: {error:?}"))?;
        let effect = BookEffect {
            schema: "conduit.book/indicator-effect@1",
            source_document_id: fragment.source_document_id.as_str().into(),
            checked_form_id: fragment.checked_form_id.as_str().into(),
            expanded_form_id: fragment.expanded_form_id.as_str().into(),
            plan_id: fragment.plan_id.as_str().into(),
            fragment_id: fragment.fragment_id.as_str().into(),
            active_play_id: active.active_play_id.as_str().into(),
            presentation_id: presentation.presentation_id.as_str().into(),
            placement_id: indicator.placement_id.as_str().into(),
            host_id: fragment.host_id.as_str().into(),
            boot_id: fragment.boot_id.as_str().into(),
            unit_millis: pattern.unit_millis,
            segments: pattern
                .segments
                .into_iter()
                .map(|segment| IndicatorSegment {
                    level: segment.level,
                    units: segment.units,
                })
                .collect(),
        };
        Ok((
            Self {
                scheduler,
                pending,
                active_play_id: active.active_play_id,
                presentation,
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
            },
            effect,
        ))
    }

    pub(super) fn complete(mut self) -> Result<BookReceipt, String> {
        self.scheduler
            .complete_host_operation(
                self.pending.node,
                self.pending.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )
            .map_err(debug_error)?;
        loop {
            if self.scheduler.next_host_request().is_some() {
                return Err("indicator Play requested an unplanned second effect".into());
            }
            match self.scheduler.step().map_err(debug_error)? {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Complete => break,
                SchedulerStatus::Idle => return Err("indicator Play became idle".into()),
                SchedulerStatus::Cancelled => return Err("indicator Play was cancelled".into()),
            }
        }
        let sign = bind_sign(
            &self.host_id,
            &self.boot_id,
            Some(&self.active_play_id),
            0,
        );
        Ok(receipt("completed", &self.active_play_id, &self.presentation, &sign))
    }

    pub(super) fn cancel(mut self) -> Result<BookReceipt, String> {
        self.scheduler.cancel().map_err(debug_error)?;
        let sign = bind_sign(
            &self.host_id,
            &self.boot_id,
            Some(&self.active_play_id),
            0,
        );
        Ok(receipt("cancelled", &self.active_play_id, &self.presentation, &sign))
    }
}

fn receipt(
    disposition: &'static str,
    active_play_id: &conduit_core::ActivePlayId,
    presentation: &PresentationIdentity,
    sign: &SignIdentity,
) -> BookReceipt {
    BookReceipt {
        schema: "conduit.book/indicator-receipt@1",
        disposition,
        active_play_id: active_play_id.as_str().into(),
        presentation_id: presentation.presentation_id.as_str().into(),
        terminal_sign_id: sign.sign_id.as_str().into(),
    }
}

fn exact_fragment(plan: &Plan) -> Result<&PlanFragment, String> {
    if plan.fragments.len() != 1 {
        return Err("executable-book Plan must contain exactly one browser fragment".into());
    }
    plan.fragments
        .first()
        .ok_or_else(|| "executable-book Plan has no fragment".into())
}

fn validate_shape(fragment: &PlanFragment) -> Result<(), String> {
    let kinds = fragment
        .placements
        .iter()
        .map(|placement| placement.kind_id.as_str())
        .collect::<Vec<_>>();
    if kinds
        != [
            conduit_text::TEXT_LITERAL_KIND,
            conduit_text::TEXT_MORSE_KIND,
            conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND,
        ]
        || fragment.connections.len() != CORDS
    {
        return Err("book runner admits exactly text/literal -> text/morse -> presentation/indicator".into());
    }
    Ok(())
}

fn prepare_scheduler(
    fragment: &PlanFragment,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
) -> Result<BookScheduler, String> {
    if lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || !lowered.remote_endpoints.is_empty()
    {
        return Err("lowered executable-book Plan has an unexpected finite shape".into());
    }
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| "book node table has the wrong size")?;
    let cords = [lowered.cords[0].spec, lowered.cords[1].spec];
    let mut routes = FixedRoutes::<ROUTES, CORDS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(route.source_node, route.source_port, route.range, &route.targets)
            .map_err(debug_error)?;
    }
    routes.seal().map_err(debug_error)?;
    let mut bindings = FixedHostOperationBindings::<4>::new(1);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(debug_error)?;
    }
    bindings.seal().map_err(debug_error)?;
    let mut values = FixedValueStore::<VALUES, VALUE_BYTES>::new((VALUES * VALUE_BYTES) as u32)
        .map_err(debug_error)?;
    let literal = literal_configuration(&fragment.placements[0])?;
    let literal_value = values.store(literal.as_bytes()).map_err(debug_error)?;
    let mut drivers = Vec::with_capacity(NODES);
    for placement in &fragment.placements {
        let operation = match placement.kind_id.as_str() {
            conduit_text::TEXT_LITERAL_KIND => BookOperation::Literal {
                value: literal_value,
                emitted: false,
            },
            conduit_text::TEXT_MORSE_KIND => BookOperation::Morse {
                maximum_input_bytes: placement.host_operations[0].maximum_input_bytes,
                pending: false,
                emitted: false,
            },
            conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND => BookOperation::Indicator {
                maximum_input_bytes: placement.host_operations[0].maximum_input_bytes,
                pending: false,
                complete: false,
            },
            _ => return Err("book Plan selected an unsupported Kind".into()),
        };
        drivers.push(OperationDriver::new(operation).map_err(debug_error)?);
    }
    let drivers = drivers
        .try_into()
        .map_err(|_| "book driver table is incomplete")?;
    let signs = FixedSignLog::<SIGNS>::new(
        lowered
            .sign_bytes
            .max((SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32),
    )
    .map_err(debug_error)?;
    BookScheduler::new_with_host_operations(
        nodes, cords, routes, bindings, drivers, values, signs,
    )
    .map_err(debug_error)
}

fn drive_to_indicator(
    scheduler: &mut BookScheduler,
    fragment: &PlanFragment,
) -> Result<HostOperationRequest, String> {
    loop {
        if let Some(request) = scheduler.next_host_request() {
            let placement = &fragment.placements[usize::from(request.node.0)];
            if placement.kind_id.as_str()
                == conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND
            {
                return Ok(request);
            }
            if placement.kind_id.as_str() != conduit_text::TEXT_MORSE_KIND {
                return Err("book Play requested an unsupported host operation".into());
            }
            let input = scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?
                .to_vec();
            let text = core::str::from_utf8(&input)
                .map_err(|_| "book Morse input is not UTF-8".to_string())?;
            let unit_millis = morse_unit_configuration(placement)?;
            let encoded = conduit_text::MorsePattern::from_text(text, unit_millis)
                .and_then(|pattern| pattern.encode())
                .map_err(|error| format!("encode Morse pattern: {error:?}"))?;
            let output = scheduler.store_host_value(&encoded).map_err(debug_error)?;
            scheduler
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: Some(
                            BoundedValueRef::new(
                                output,
                                placement.host_operations[0].maximum_output_bytes,
                            )
                            .map_err(|_| "Morse output exceeded its planned bound")?,
                        ),
                        failure: None,
                    },
                )
                .map_err(debug_error)?;
            continue;
        }
        match scheduler.step().map_err(debug_error)? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => {
                return Err("book Play completed without an indicator effect".into())
            }
            SchedulerStatus::Idle => return Err("book Play became idle".into()),
            SchedulerStatus::Cancelled => return Err("book Play was cancelled".into()),
        }
    }
}

fn literal_configuration(placement: &conduit_core::PlannedGear) -> Result<&str, String> {
    placement
        .configuration
        .iter()
        .find_map(|(key, value)| match (key.as_str(), value) {
            ("value", ConfigurationValue::Text(value))
                if !value.is_empty()
                    && value.len() <= conduit_text::MAXIMUM_MORSE_INPUT_BYTES =>
            {
                Some(value.as_str())
            }
            _ => None,
        })
        .ok_or_else(|| "book text literal is missing, empty, or oversized".into())
}

fn morse_unit_configuration(placement: &conduit_core::PlannedGear) -> Result<u16, String> {
    placement
        .configuration
        .iter()
        .find_map(|(key, value)| match (key.as_str(), value) {
            (conduit_text::MORSE_UNIT_MILLIS_KEY, ConfigurationValue::U64(value)) => {
                u16::try_from(*value).ok()
            }
            _ => None,
        })
        .filter(|value| {
            (conduit_text::MINIMUM_MORSE_UNIT_MILLIS
                ..=conduit_text::MAXIMUM_MORSE_UNIT_MILLIS)
                .contains(value)
        })
        .ok_or_else(|| "book Morse unit duration is missing or invalid".into())
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO_LIGHT: &str = r#"form hello-light {
    message: text/literal("SOS")
    morse: text/morse(120)
    light: presentation/indicator

    message > morse > light
}
"#;

    #[test]
    fn one_exact_form_reaches_a_browser_indicator_effect_and_can_cancel() {
        let (session, effect) = BookSession::prepare(
            "browser/book-test",
            "browser-boot/book-test",
            HELLO_LIGHT,
            1,
        )
        .unwrap();
        assert_eq!(effect.unit_millis, 120);
        assert_eq!(effect.segments.len(), 17);
        assert_eq!(effect.host_id, "browser/book-test");
        assert_eq!(session.cancel().unwrap().disposition, "cancelled");
    }

    #[test]
    fn unrelated_forms_are_refused_by_the_book_boundary() {
        let wrong = HELLO_LIGHT.replace("presentation/indicator", "text/upper");
        assert!(BookSession::prepare(
            "browser/book-test",
            "browser-boot/book-test",
            &wrong,
            2,
        )
        .is_err());
    }
}
