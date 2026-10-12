//! Initialized identity of the compiled missing-cost coordinator reference.
//! The artifact is the actual executable; this offer claims no known forecasts.
use conduit_core::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Initialize {
    host: HostId,
    boot: BootId,
}
pub(super) fn run() -> Result<(), String> {
    let mut input = std::io::stdin().lock();
    let mut prefix = [0; 4];
    input
        .read_exact(&mut prefix)
        .map_err(|_| "coordinator identity header unavailable")?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > 4096 {
        return Err("coordinator identity extent refused".into());
    }
    let mut bytes = vec![0; length];
    input
        .read_exact(&mut bytes)
        .map_err(|_| "coordinator identity incomplete")?;
    let config: Initialize =
        serde_json::from_slice(&bytes).map_err(|_| "coordinator identity invalid")?;
    if [config.host.as_str(), config.boot.as_str()]
        .iter()
        .any(|v| v.is_empty() || v.len() > 256)
    {
        return Err("coordinator identity subject refused".into());
    }
    let path = std::env::current_exe().map_err(|_| "coordinator executable unavailable")?;
    let mut file = std::fs::File::open(path).map_err(|_| "coordinator artifact unavailable")?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 8192];
    let mut total = 0usize;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| "coordinator artifact read failed")?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count)
            .ok_or("coordinator artifact size overflow")?;
        if total > 256 * 1024 * 1024 {
            return Err("coordinator artifact extent refused".into());
        }
        hash.update(&buffer[..count]);
    }
    let artifact = ArtifactId::from(format!("sha256:{:x}", hash.finalize()));
    let capability = CapabilityId::from("conduit.proof/native-pool-coordinator");
    let advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: config.host.clone(),
        boot_id: config.boot.clone(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduit.proof/native-coordinator@1"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![capability_offer_from_parts! {
            semantic_contract:Default::default(),
            startup_parameters:vec![FrontStartupParameter{name:"workers".into(),value_type:KindId::from("value/pool-reference"),has_default:false}],
            shorthand:None,capability_id:capability.clone(),kind_id:kind_id("conduit-proof/pool-coordinate"),
            kind_contract_revision:KindIdentity::from("conduit-proof/pool-coordinate@1"),
            implementation:ImplementationOffer { execution_profile_id:ExecutionProfileId::from("conduit.proof/native-reference@1"),
                implementation_id:ImplementationId::from("conduit.proof/hosted-pool-reference@1"),artifact_id:artifact },
            inputs:vec![],outputs:vec![],host_calls:vec![],resource_requirements:vec![],authority_requirements:vec![],
            limits:CapabilityLimits{max_active_instances:1,max_queue_items:1,max_queue_bytes:1},
        }],
        planner_capabilities: vec![PlannerCapabilityOffer {
            profile_id: PlannerProfileId::from("conduit.proof/native-pool-planning@1"),
            limits: PlannerLimits {
                maximum_host_advertisements: 3,
                maximum_gears: 4,
                maximum_connections: 4,
                maximum_authority_grants: 4,
                maximum_protected_resource_grants: 0,
                maximum_line_offers: 4,
            },
        }],
    };
    let authority = AuthorityGrant {
        grant_id: AuthorityGrantId::from("conduit.proof/native-pool-admission"),
        contract_id: AuthorityContractId::from(SHARED_POOL_ADMIT_AUTHORITY_CONTRACT),
        host_call_contract_id: HostCallContractId::from(SHARED_POOL_ADMIT_HOST_CALL_CONTRACT),
        subject_kind: kind_id(SHARED_POOL_AUTHORITY_SUBJECT_KIND),
        host_id: config.host,
        boot_id: config.boot,
        capability_id: capability,
    };
    let receipt=serde_json::to_vec(&serde_json::json!({"phase":"reference-coordinator-initialized",
        "cost_mode":"missing-estimates-capacity-fallback","advertisement":advertisement,"authority":authority}))
        .map_err(|_|"coordinator identity serialization failed")?;
    let mut output = std::io::stdout().lock();
    output
        .write_all(&(receipt.len() as u32).to_be_bytes())
        .map_err(|_| "coordinator identity header failed")?;
    output
        .write_all(&receipt)
        .map_err(|_| "coordinator identity output failed")?;
    output
        .flush()
        .map_err(|_| "coordinator identity flush failed".into())
}
