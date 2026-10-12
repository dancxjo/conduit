//! Ordinary Source checking, member-envelope planning and policy sealing.
//! Initialized coordinator and worker advertisements and owned Lines are
//! supplied by the proof owner. This preparation helper invents none of them.
use conduit_core::{BaseImplementationId, HostAdvertisement, LineOffer, SharedPoolSelectionPolicy};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_shared_pool_policies,
    PlanningOptions, SharedPoolPlanningRequirement, SharedPoolPolicySelection,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, KindProjection,
    KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::{Read, Write};

const MAXIMUM_DOCUMENT: usize = 1024 * 1024;
const COORDINATOR_KIND: &str = "conduit-proof/pool-coordinate";
const SOURCE: &str = r#"
plot llm/generate (
    maximum-input-bytes: Count = 4096
    maximum-context-items: Count = 1
    maximum-output-bytes: Count = 1024
    maximum-work-units: Count = 4096
    maximum-history-items: Count = 0
    request: llm/generation-request@1 >> result: llm/generated-result@1
) {
}
plot consumer (workers: Pool) {
    coordinate: conduit-proof/pool-coordinate(workers)
}
plot model-service {
    pool workers: llm/generate(size = 2)
    client: consumer(workers)
}
"#;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preparation {
    /// Exactly one initialized coordinator advertisement and two initialized
    /// model workers. The coordinator Back is a separate unfinished proof seam.
    hosts: [HostAdvertisement; 3],
    /// Exact four directional Lines owned by the controller's worker pipes.
    lines: [LineOffer; 4],
    mechanism: BaseImplementationId,
    authority: conduit_core::AuthorityGrant,
    policy: SharedPoolSelectionPolicy,
}

fn build(input: Preparation) -> Result<conduit_core::Plan, String> {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: COORDINATOR_KIND.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "workers".into(),
                value_type: "Pool".into(),
                default: None,
            }],
        })
        .map_err(|e| e.to_string())?;
    let mut profiles = ProfileCatalog::new();
    profiles
        .insert(KindProjection {
            kind_id: conduit_core::kind_id(COORDINATOR_KIND),
            kind_contract_revision: conduit_core::KindIdentity::from(
                "conduit-proof/pool-coordinate@1",
            ),
            inputs: vec![],
            outputs: vec![],
            configuration: vec![],
        })
        .map_err(|e| e.to_string())?;
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup)
        .map_err(|e| format!("check pool Source: {e:?}"))?;
    let expanded = expand_canonical_plot(&checked, "model-service", &profiles)
        .map_err(|e| format!("expand pool Source: {e:?}"))?;
    let pool_id = expanded
        .shared_pools
        .first()
        .ok_or("checked pool absent")?
        .pool_id
        .clone();
    let placements =
        default_expanded_placements(&expanded, &input.hosts).map_err(|e| e.to_string())?;
    let requirements = BTreeMap::from([(
        pool_id.clone(),
        SharedPoolPlanningRequirement {
            member_limits: conduit_core::PoolMemberLimits {
                queue_item_capacity: 1,
                queue_byte_capacity: 4096,
                sign_item_capacity: 16,
                sign_byte_capacity: 4096,
            },
            admission_authority: input.authority,
            member_sessions_required: true,
        },
    )]);
    let empty = BTreeMap::new();
    let candidates = BTreeMap::new();
    let plan = plan_expanded_canonical_with_shared_pool_policies(
        &expanded,
        &input.hosts,
        &placements,
        &[input.mechanism],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &input.lines,
        },
        &requirements,
        &[SharedPoolPolicySelection {
            pool_id: &pool_id,
            policy: input.policy,
        }],
    )
    .map_err(|e| format!("plan pool: {e:?}"))?;
    if !conduit_core::verify_plan(&plan) {
        return Err("constructed Plan failed verification".into());
    }
    let pool = plan
        .fragments
        .first()
        .and_then(|f| f.shared_pools.first())
        .ok_or("planned pool absent")?;
    if pool.realization_envelope.len() != 2
        || pool
            .realization_envelope
            .iter()
            .any(|m| m.admitted_lines.len() != 2)
    {
        return Err("proof requires two exact member envelopes and directional Lines".into());
    }
    Ok(plan)
}

pub(super) fn run() -> Result<(), String> {
    let mut input = std::io::stdin().lock();
    let mut prefix = [0; 4];
    input
        .read_exact(&mut prefix)
        .map_err(|_| "preparation header unavailable")?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > MAXIMUM_DOCUMENT {
        return Err("preparation extent refused".into());
    }
    let mut bytes = vec![0; length];
    input
        .read_exact(&mut bytes)
        .map_err(|_| "preparation document incomplete")?;
    let request: Preparation =
        serde_json::from_slice(&bytes).map_err(|_| "preparation document invalid")?;
    let plan = build(request)?;
    let receipt = serde_json::to_vec(&serde_json::json!({
        "phase":"planned", "proof_class":"preparation-only",
        "source":SOURCE, "plan":plan,
    }))
    .map_err(|_| "Plan serialization failed")?;
    if receipt.len() > MAXIMUM_DOCUMENT {
        return Err("Plan document extent refused".into());
    }
    let mut output = std::io::stdout().lock();
    output
        .write_all(&(receipt.len() as u32).to_be_bytes())
        .map_err(|_| "Plan header delivery failed")?;
    output
        .write_all(&receipt)
        .map_err(|_| "Plan delivery failed")?;
    output.flush().map_err(|_| "Plan flush failed".into())
}
