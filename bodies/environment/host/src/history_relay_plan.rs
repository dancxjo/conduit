//! Explicit proof composition: checked environmental Source and ordinary planner.
//! Fixture identities belong to the test Body, never a physical instrument.
use crate::{checked, Result};
use conduit_core::*;
use conduit_environment_runtime::history_exchange::{self as h, installation::*, *};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};
use std::collections::BTreeMap;

pub(crate) struct HistoryProofPlan<P> {
    pub plan: Plan,
    pub provider_placement: PlacementId,
    pub base: ReadyHistoryBase<P>,
    pub scope: BaseCapabilityScope,
    pub authority: BaseCapabilityAuthority,
    pub claim: BaseOperationClaim,
}
fn host(name: &str) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(format!("host/{name}")),
        boot_id: BootId::from(format!("boot/{name}")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("environment/history-relay-proof@1"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
fn endpoint_kind(name: &str, port: PortDescriptor) -> Kind {
    let output = port.direction == PortDirection::Output;
    Kind {
        kind_id: kind_id(name),
        kind_contract_revision: KindIdentity::from(format!("{name}@1")),
        startup_parameters: vec![],
        shorthand: None,
        inputs: if output { vec![] } else { vec![port.clone()] },
        outputs: if output { vec![port] } else { vec![] },
        configuration: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: h::contract::HISTORY_MAXIMUM_BYTES,
        },
    }
}
fn endpoint_offer(kind: &Kind) -> CapabilityOffer {
    conduit_core::capability_offer_from_parts! {
    semantic_contract:kind.semantic_contract(),startup_parameters:vec![],shorthand:None,
    capability_id:CapabilityId::from(kind.kind_id.as_str()),kind_id:kind.kind_id.clone(),kind_contract_revision:kind.kind_contract_revision.clone(),
    implementation:ImplementationOffer{execution_profile_id:ExecutionProfileId::from("environment/history-proof-step@1"),implementation_id:ImplementationId::from(format!("{}@1",kind.kind_id.as_str())),artifact_id:ArtifactId::from("environment/history-proof-client@1")},
    inputs:kind.inputs.clone(),outputs:kind.outputs.clone(),host_calls:vec![],resource_requirements:vec![],authority_requirements:vec![],limits:kind.limits.clone(),
    }
}
/// The trusted proof composition root supplies the actual finite evidence
/// provider and independently observed Line offers. No network claim is inferred
/// from a matching address or from this source document.
pub(crate) fn prepare_proof_plan<P: HistoryProvider>(
    provider: P,
    lines: &[LineOffer],
) -> Result<HistoryProofPlan<P>> {
    let contract = checked(HistoryReadContract::prepare())?;
    let mut producer = host("source");
    let mut collector = host("collector");
    let identity = HistoryNativeIdentity {
        host_id: producer.host_id.clone(),
        boot_id: producer.boot_id.clone(),
        base_id: "history/base".into(),
        provider_instance_id: "history/provider".into(),
        provider_generation: 1,
        resource_pool_id: "history/evidence".into(),
        resource_generation_id: ResourceGenerationId("history/resource-generation/1".into()),
        envelope_id: "history/envelope/1".into(),
        artifact_id: "history/proof-provider@1".into(),
    };
    // SAFETY: the proof composition root transfers its finite provider to this
    // exact test-only identity; physical instrument identities are absent.
    let base = checked(unsafe {
        ReadyHistoryBase::new(
            identity.clone(),
            HistoryAttachment {
                generation: 1,
                resource_bytes: 8192,
            },
            provider,
        )
    })?;
    checked(base.append_to_advertisement(&mut producer, &contract))?;
    let mut request = contract.kind().inputs[0].clone();
    request.direction = PortDirection::Output;
    let mut result = contract.kind().outputs[0].clone();
    result.direction = PortDirection::Input;
    let source = endpoint_kind("environment/proof/history-cursor", request);
    let sink = endpoint_kind("environment/proof/history-batch", result);
    collector.capabilities = vec![endpoint_offer(&source), endpoint_offer(&sink)];
    let (mut startup, mut profile) = contract.catalogs();
    for kind in [&source, &sink] {
        checked(startup.insert(conduit_plot::KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        }))?;
        checked(startup.insert_fore(kind.kind_id.as_str(), kind.checked_front()))?;
        checked(profile.insert_kind(kind.clone()))?;
    }
    let source_text=format!("{}\nplot history-relay-proof {{\n cursor: environment/proof/history-cursor\n evidence: environment/history/read\n batch: environment/proof/history-batch\n cursor.request >> evidence.request\n evidence.result >> batch.result\n}}\n",h::contract::HISTORY_TYPES);
    let syntax = conduit_plot::parse_syntax_document(&source_text);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("history source: {:?}", syntax.diagnostics).into());
    }
    let document = checked(conduit_plot::check_syntax_document(&syntax, &startup))?;
    let expanded = checked(conduit_plot::expand_canonical_plot(
        &document,
        "history-relay-proof",
        &profile,
    ))?;
    let choices = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("history-relay-proof/cursor"),
                PlacementChoice {
                    host_id: collector.host_id.clone(),
                    capability_id: source.kind_id.as_str().into(),
                },
            ),
            (
                GearId::from("history-relay-proof/batch"),
                PlacementChoice {
                    host_id: collector.host_id.clone(),
                    capability_id: sink.kind_id.as_str().into(),
                },
            ),
            (
                GearId::from("history-relay-proof/evidence"),
                PlacementChoice {
                    host_id: producer.host_id.clone(),
                    capability_id: HISTORY_IMPLEMENTATION.into(),
                },
            ),
        ]),
    };
    let grant = AuthorityGrant {
        grant_id: "history/proof-authority/1".into(),
        contract_id: HISTORY_AUTHORITY.into(),
        host_call_contract_id: h::contract::HISTORY_CALL.into(),
        subject_kind: contract.kind().kind_id.clone(),
        host_id: producer.host_id.clone(),
        boot_id: producer.boot_id.clone(),
        capability_id: HISTORY_IMPLEMENTATION.into(),
    };
    let bases: Vec<_> = lines.iter().map(|line| line.binding.base.clone()).collect();
    let empty = BTreeMap::new();
    let plan = checked(plan_expanded_canonical_with_options(
        &expanded,
        &[producer.clone(), collector],
        &choices,
        &bases,
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: h::contract::HISTORY_MAXIMUM_BYTES,
            authority_grants: core::slice::from_ref(&grant),
            protected_resource_grants: &[],
            line_offers: lines,
        },
    ))?;
    let fragment = plan
        .fragments
        .iter()
        .find(|f| f.host_id == producer.host_id)
        .ok_or("history producer fragment missing")?;
    if fragment.connections.len() != 2
        || fragment
            .connections
            .iter()
            .any(|cord| cord.selected_line.is_none())
    {
        return Err("history proof requires exactly two admitted remote Cords".into());
    }
    let [gear] = fragment.placements.as_slice() else {
        return Err("history producer requires exactly one selected operation".into());
    };
    let provider_placement = gear.placement_id.clone();
    let active = bind_active_play(&plan.plan_id, &producer.host_id, &producer.boot_id, 0);
    let scope = BaseCapabilityScope {
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        base_instance_id: identity.provider_instance_id.clone(),
        base_provider_generation: identity.provider_generation,
        plan_id: plan.plan_id.clone(),
        active_play_id: active.active_play_id,
        authority_grant_id: grant.grant_id.clone(),
        authority_contract_id: grant.contract_id.clone(),
        capability_id: HISTORY_IMPLEMENTATION.into(),
        implementation_id: HISTORY_IMPLEMENTATION.into(),
        operation_contract_id: h::contract::HISTORY_CALL.into(),
        subject_kind: contract.kind().kind_id.clone(),
        resource_pool_id: identity.resource_pool_id.clone(),
        resource_generation_id: identity.resource_generation_id.clone(),
        envelope_id: identity.envelope_id.clone(),
        maximum_parameter_bytes: h::contract::HISTORY_MAXIMUM_BYTES,
        maximum_result_bytes: h::contract::HISTORY_MAXIMUM_BYTES,
        maximum_work_units: 240,
        maximum_in_flight: 1,
        maximum_operations: 240,
    };
    let authority = BaseCapabilityAuthority {
        grant,
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: scope.maximum_parameter_bytes,
        maximum_result_bytes: scope.maximum_result_bytes,
        maximum_work_units: scope.maximum_work_units,
        maximum_in_flight: 1,
        maximum_operations: 240,
    };
    let claim = BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: h::contract::HISTORY_MAXIMUM_BYTES,
        work_units: 1,
    };
    Ok(HistoryProofPlan {
        plan,
        provider_placement,
        base,
        scope,
        authority,
        claim,
    })
}
