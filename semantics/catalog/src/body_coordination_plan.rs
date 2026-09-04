//! Exact two-std-Host realization of the mechanism-free coordination Form.

use crate::{install_text_pipeline_catalogs, TEXT_PRESENTATION_KIND};
use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, present_host_operation_requirement, process_owned_line_offer_with_limits,
    resource_offer, resource_requirement, BaseImplementationId, BootId, CapabilityOffer,
    HostAdvertisement, HostId, HostProfileId, LineId, LineOffer, LineScope, LineSecurity,
    LinkLimits, OfferGeneration, Plan, PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};

pub const BODY_COORDINATION_SOURCE: &str =
    include_str!("../../../proof/fixtures/forms/body-coordination.conduit");
pub const FOREBRAIN_HOST: &str = "pete/forebrain-host";
pub const MOTHERBRAIN_HOST: &str = "pete/motherbrain-host";
pub const FOREBRAIN_TO_MOTHERBRAIN_LINE: &str = "pete/coordination/forebrain-to-motherbrain";
pub const MOTHERBRAIN_TO_FOREBRAIN_LINE: &str = "pete/coordination/motherbrain-to-forebrain";
pub const BODY_COORDINATION_MAXIMUM_ITEMS: u16 = 1;
pub const BODY_COORDINATION_MAXIMUM_FRAME_BYTES: u32 = 1_024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyCoordinationPlan {
    pub plan: Plan,
    pub forebrain: HostAdvertisement,
    pub motherbrain: HostAdvertisement,
    pub outbound_line: LineOffer,
    pub return_line: LineOffer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyCoordinationLineLoss {
    pub plan_id: conduit_core::PlanId,
    pub unavailable_line_id: LineId,
    pub replan_required: bool,
    pub refusal: String,
}

pub fn exact_body_coordination_plan(
    forebrain_boot: BootId,
    motherbrain_boot: BootId,
    base_instance: &str,
) -> Result<BodyCoordinationPlan, String> {
    exact_body_coordination_plan_with_loss(forebrain_boot, motherbrain_boot, base_instance, None)
}

pub fn exact_body_coordination_line_loss(
    forebrain_boot: BootId,
    motherbrain_boot: BootId,
    base_instance: &str,
    unavailable_line: &str,
) -> Result<BodyCoordinationLineLoss, String> {
    if !matches!(
        unavailable_line,
        FOREBRAIN_TO_MOTHERBRAIN_LINE | MOTHERBRAIN_TO_FOREBRAIN_LINE
    ) {
        return Err("unknown coordination Line loss target".into());
    }
    let accepted = exact_body_coordination_plan(
        forebrain_boot.clone(),
        motherbrain_boot.clone(),
        base_instance,
    )?;
    let refusal = exact_body_coordination_plan_with_loss(
        forebrain_boot,
        motherbrain_boot,
        base_instance,
        Some(unavailable_line),
    )
    .err()
    .ok_or("unavailable selected coordination Line still produced a Plan")?;
    Ok(BodyCoordinationLineLoss {
        plan_id: accepted.plan.plan_id,
        unavailable_line_id: LineId::from(unavailable_line),
        replan_required: true,
        refusal,
    })
}

fn exact_body_coordination_plan_with_loss(
    forebrain_boot: BootId,
    motherbrain_boot: BootId,
    base_instance: &str,
    unavailable_line: Option<&str>,
) -> Result<BodyCoordinationPlan, String> {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    install_text_pipeline_catalogs(&mut startup, &mut profile)?;
    let syntax = conduit_form::parse_syntax_document(BODY_COORDINATION_SOURCE);
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("check coordination Form: {error:?}"))?;
    let expanded = conduit_form::expand_canonical_form(&checked, "body-coordination", &profile)
        .map_err(|error| format!("expand coordination Form: {error:?}"))?;

    let forebrain = coordination_host(FOREBRAIN_HOST, forebrain_boot);
    let motherbrain = coordination_host(MOTHERBRAIN_HOST, motherbrain_boot);
    let limits = LinkLimits {
        maximum_in_flight_items: BODY_COORDINATION_MAXIMUM_ITEMS,
        maximum_payload_bytes: conduit_text::MAX_TEXT_BYTES,
        maximum_buffered_bytes: conduit_text::MAX_TEXT_BYTES,
        maximum_frame_bytes: BODY_COORDINATION_MAXIMUM_FRAME_BYTES,
    };
    let mut outbound_line = process_owned_line_offer_with_limits(
        FOREBRAIN_TO_MOTHERBRAIN_LINE,
        "pete/coordination/forebrain-to-motherbrain-binding",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        base_instance,
        &forebrain,
        &motherbrain,
        limits,
    );
    let mut return_line = process_owned_line_offer_with_limits(
        MOTHERBRAIN_TO_FOREBRAIN_LINE,
        "pete/coordination/motherbrain-to-forebrain-binding",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        base_instance,
        &motherbrain,
        &forebrain,
        limits,
    );
    for line in [&mut outbound_line, &mut return_line] {
        line.contract.scope = LineScope::LocalNetwork;
        line.contract.security = LineSecurity::PlaintextNetwork;
    }
    match unavailable_line {
        Some(FOREBRAIN_TO_MOTHERBRAIN_LINE) => {
            outbound_line.availability.availability = conduit_core::LineAvailability::Unavailable;
        }
        Some(MOTHERBRAIN_TO_FOREBRAIN_LINE) => {
            return_line.availability.availability = conduit_core::LineAvailability::Unavailable;
        }
        Some(_) => return Err("unknown coordination Line loss target".into()),
        None => {}
    }
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            placement(
                "body-coordination/message",
                &forebrain,
                conduit_text::TEXT_LITERAL_KIND,
            )?,
            placement(
                "body-coordination/receive-message",
                &motherbrain,
                TEXT_PRESENTATION_KIND,
            )?,
            placement(
                "body-coordination/reply",
                &motherbrain,
                conduit_text::TEXT_LITERAL_KIND,
            )?,
            placement(
                "body-coordination/receive-reply",
                &forebrain,
                TEXT_PRESENTATION_KIND,
            )?,
        ]),
    };
    let line_candidates = BTreeMap::from([
        (
            (
                conduit_core::GearId::from("body-coordination/message"),
                conduit_core::GearId::from("body-coordination/receive-message"),
            ),
            vec![outbound_line.line_id.clone()],
        ),
        (
            (
                conduit_core::GearId::from("body-coordination/reply"),
                conduit_core::GearId::from("body-coordination/receive-reply"),
            ),
            vec![return_line.line_id.clone()],
        ),
    ]);
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        &[forebrain.clone(), motherbrain.clone()],
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: BODY_COORDINATION_MAXIMUM_ITEMS,
            connection_byte_capacity: conduit_text::MAX_TEXT_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[outbound_line.clone(), return_line.clone()],
        },
    )
    .map_err(|error| error.to_string())?;
    Ok(BodyCoordinationPlan {
        plan,
        forebrain,
        motherbrain,
        outbound_line,
        return_line,
    })
}

fn coordination_host(id: &str, boot_id: BootId) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(id),
        boot_id,
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("body-coordination/fixture@1"),
        resources: vec![resource_offer(
            "body-coordination/presentation",
            PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        planner_capabilities: Vec::new(),
        capabilities: vec![
            text_literal_fixture_offer(),
            text_presentation_fixture_offer(),
        ],
    }
}

fn text_literal_fixture_offer() -> CapabilityOffer {
    let mut offer = crate::realization_offer(
        crate::text_literal_contract(),
        conduit_text::TEXT_LITERAL_CONTRACT_REVISION,
        crate::RealizationOfferIdentity {
            capability: "body-coordination/text-literal",
            execution_profile: "body-coordination/fixture@1",
            implementation: "body-coordination/text-literal@1",
            artifact: "body-coordination/fixture@1",
        },
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    offer.startup_parameters[0].has_default = false;
    offer
}

fn text_presentation_fixture_offer() -> CapabilityOffer {
    crate::realization_offer(
        crate::text_presentation_contract(),
        crate::TEXT_PRESENTATION_CONTRACT_REVISION,
        crate::RealizationOfferIdentity {
            capability: "body-coordination/text-presentation",
            execution_profile: "body-coordination/fixture@1",
            implementation: "body-coordination/text-presentation@1",
            artifact: "body-coordination/fixture@1",
        },
        vec![present_host_operation_requirement(
            kind_id("presentation/body-coordination-text"),
            conduit_text::MAX_TEXT_BYTES,
        )],
        vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        Vec::new(),
    )
}

fn placement(
    gear: &str,
    host: &HostAdvertisement,
    kind: &str,
) -> Result<(conduit_core::GearId, PlacementChoice), String> {
    let capability_id = host
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == kind)
        .map(|offer| offer.capability_id.clone())
        .ok_or_else(|| format!("{} lacks {kind}", host.host_id.as_str()))?;
    Ok((
        conduit_core::GearId::from(gear),
        PlacementChoice {
            host_id: host.host_id.clone(),
            capability_id,
        },
    ))
}

pub fn coordination_line_ids(exact: &BodyCoordinationPlan) -> Vec<LineId> {
    vec![
        exact.outbound_line.line_id.clone(),
        exact.return_line.line_id.clone(),
    ]
}

#[cfg(test)]
mod tests;
