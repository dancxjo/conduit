//! Host realization of the source-level shared-pool chat proof.
//!
//! The authored source contains only pool, fan, merge, room, and peer
//! semantics. This host module selects RFC6455 as the physical browser line
//! after the exact pool Plan has been checked, planned, and lowered.

use crate::external_websocket::{
    ExternalPeerId, ExternalWebSocketError, ExternalWebSocketListener,
};
use conduit_core::{
    kind_id, AuthorityContractId, AuthorityGrant, AuthorityGrantId, BaseImplementationId, BootId,
    CapabilityId, HostAdvertisement, HostId, HostOperationContractId, HostProfileId,
    OfferGeneration, PlannerCapabilityOffer, PlannerLimits, PlannerProfileId, PoolMemberLimits,
    SharedPoolId, PROTOCOL_VERSION, SHARED_POOL_ADMIT_AUTHORITY_CONTRACT,
    SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT, SHARED_POOL_AUTHORITY_SUBJECT_KIND,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_kernel::shared_flow::{FixedFan, FixedMerge, MergeEvent};
use conduit_kernel::shared_pool::{
    FixedSharedPool, MemberIdentity, MemberKey, MemberPlacement, PoolId,
};
use conduit_kernel::{NodeId, ValueRef};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_shared_pools, PlanningOptions,
    SharedPoolPlanningRequirement,
};
use std::collections::BTreeMap;
use std::net::SocketAddr;

const SOURCE: &str = include_str!("../../../forms/pool-webchat/main.conduit");
const MAXIMUM_PEERS: usize = conduit_chat::POOL_WEBCHAT_MAXIMUM_PEERS as usize;
const MAXIMUM_MESSAGE_BYTES: usize = 256;
const AUTHORITY: u16 = 0;

pub fn run(bind: &str) -> Result<(), String> {
    let (plan_id, lowered_pool) = planned_pool()?;
    let address = bind
        .parse::<SocketAddr>()
        .map_err(|error| format!("invalid bind address: {error}"))?;
    let mut line = ExternalWebSocketListener::bind(
        address,
        conduit_chat::POOL_WEBCHAT_MAXIMUM_PEERS,
        MAXIMUM_MESSAGE_BYTES as u32,
    )
    .map_err(|error| format!("pool chat line bind: {error:?}"))?;
    println!(
        "pool-webchat-ready address={} source={} plan={} pool={} line=websocket",
        line.local_addr().map_err(debug_error)?,
        source_identity()?,
        plan_id,
        lowered_pool.pool_id.as_str(),
    );

    let mut pool = FixedSharedPool::<MAXIMUM_PEERS, 256>::new(
        PoolId(lowered_pool.pool.0),
        lowered_pool.maximum_members,
        AUTHORITY,
        lowered_pool.realizations.len() as u16,
    )
    .map_err(|error| format!("pool initialization: {error:?}"))?;
    let mut members = [None; MAXIMUM_PEERS];
    for _ in 0..2 {
        let peer = line.accept_peer().map_err(debug_error)?;
        members[peer.index()] = Some(admit(&mut pool, peer)?);
    }
    drive_chat(&mut line, &mut pool, &mut members)?;
    println!(
        "pool-webchat-complete plan={} pool={} sign={} population={}",
        plan_id,
        lowered_pool.pool_id.as_str(),
        pool.signs().count(),
        pool.active_population(),
    );
    Ok(())
}

fn planned_pool() -> Result<(String, conduit_plan_lowering::lowering::LoweredSharedPool), String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_chat::install_pool_chat_catalogs(&mut startup, &mut profile)?;
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup)
        .map_err(|error| format!("pool chat check: {error:?}"))?;
    let expanded = expand_canonical_form(&checked, "pool-webchat", &profile)
        .map_err(|error| format!("pool chat expansion: {error:?}"))?;
    let host = advertisement();
    let placements = default_expanded_placements(&expanded, std::slice::from_ref(&host))
        .map_err(|error| error.to_string())?;
    let authority = admission_authority();
    let requirements = BTreeMap::from([(
        SharedPoolId::from("pool-webchat/peers"),
        SharedPoolPlanningRequirement {
            member_limits: PoolMemberLimits {
                queue_item_capacity: 32,
                queue_byte_capacity: 8_192,
                sign_item_capacity: 8,
                sign_byte_capacity: 1_024,
            },
            admission_authority: authority.clone(),
        },
    )]);
    let empty_bases = BTreeMap::new();
    let empty_routes = BTreeMap::new();
    let plan = plan_expanded_canonical_with_shared_pools(
        &expanded,
        std::slice::from_ref(&host),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &empty_bases,
            line_candidates: &empty_routes,
            connection_item_capacity: 32,
            connection_byte_capacity: 8_192,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &requirements,
    )
    .map_err(|error| error.to_string())?;
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0])
        .map_err(|error| format!("pool chat lowering: {error:?}"))?;
    let pool = lowered
        .shared_pools
        .into_iter()
        .next()
        .ok_or_else(|| "pool chat Plan has no lowered pool".to_string())?;
    Ok((plan.plan_id.as_str().to_string(), pool))
}

fn source_identity() -> Result<String, String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_chat::install_pool_chat_catalogs(&mut startup, &mut profile)?;
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup)
        .map_err(|error| format!("pool chat check: {error:?}"))?;
    let expanded = expand_canonical_form(&checked, "pool-webchat", &profile)
        .map_err(|error| format!("pool chat expansion: {error:?}"))?;
    Ok(format!(
        "{}/{}/{}",
        expanded.source_document_id.as_str(),
        expanded.checked_form_id.as_str(),
        expanded.expanded_form_id.as_str()
    ))
}

fn advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("std-pool-webchat"),
        boot_id: BootId::from("std-pool-webchat-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std-pool-webchat-profile"),
        resources: Vec::new(),
        capabilities: conduit_chat::pool_chat_capabilities().into(),
        planner_capabilities: vec![PlannerCapabilityOffer {
            profile_id: PlannerProfileId::from("std/pool-webchat-planner"),
            limits: PlannerLimits {
                maximum_host_advertisements: 1,
                maximum_gears: 8,
                maximum_connections: 8,
                maximum_authority_grants: 1,
                maximum_protected_resource_grants: 0,
                maximum_line_offers: 0,
            },
        }],
    }
}

fn admission_authority() -> AuthorityGrant {
    AuthorityGrant {
        grant_id: AuthorityGrantId::from("grant/pool-webchat-admit"),
        contract_id: AuthorityContractId::from(SHARED_POOL_ADMIT_AUTHORITY_CONTRACT),
        host_operation_contract_id: HostOperationContractId::from(
            SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT,
        ),
        subject_kind: kind_id(SHARED_POOL_AUTHORITY_SUBJECT_KIND),
        host_id: HostId::from("std-pool-webchat"),
        boot_id: BootId::from("std-pool-webchat-boot"),
        capability_id: CapabilityId::from("std/pool-webchat/chat/room"),
    }
}

fn admit(
    pool: &mut FixedSharedPool<MAXIMUM_PEERS, 256>,
    peer: ExternalPeerId,
) -> Result<MemberIdentity, String> {
    let mut key = [0u8; 32];
    key[30..].copy_from_slice(&(peer.index() as u16).to_be_bytes());
    let member = pool
        .admit(
            MemberKey(key),
            MemberPlacement {
                node: NodeId(peer.index() as u16),
                realization: 0,
                play: peer.index() as u16,
            },
            AUTHORITY,
        )
        .map_err(|error| format!("pool admit: {error:?}"))?;
    pool.trigger(member)
        .map_err(|error| format!("pool trigger: {error:?}"))?;
    Ok(member)
}

fn drive_chat(
    line: &mut ExternalWebSocketListener,
    pool: &mut FixedSharedPool<MAXIMUM_PEERS, 256>,
    members: &mut [Option<MemberIdentity>; MAXIMUM_PEERS],
) -> Result<(), String> {
    let mut current = ExternalPeerId::from_index(0);
    let mut sequence = 0u64;
    let mut bytes = [0u8; MAXIMUM_MESSAGE_BYTES];
    let mut snapshot = [empty_member(); MAXIMUM_PEERS];
    let mut fan = FixedFan::<MAXIMUM_PEERS>::new().map_err(debug_error)?;
    let mut merge = FixedMerge::<32>::new().map_err(debug_error)?;
    while pool.active_population() > 0 {
        let source = members[current.index()]
            .ok_or_else(|| "line selected an inactive member".to_string())?;
        match line.receive_binary(current, &mut bytes) {
            Ok(length) => {
                sequence += 1;
                let value = ValueRef {
                    slot: (sequence % u64::from(u16::MAX)) as u16,
                    generation: 1,
                    byte_len: length as u32,
                };
                merge
                    .offer(MergeEvent {
                        sequence,
                        source,
                        value,
                    })
                    .map_err(debug_error)?;
                let merged = merge
                    .pop()
                    .ok_or_else(|| "merge lost an item".to_string())?;
                let recipients = pool.snapshot_active(&mut snapshot).map_err(debug_error)?;
                fan.begin(merged.value, &snapshot[..recipients])
                    .map_err(debug_error)?;
                for recipient in snapshot[..recipients].iter().copied() {
                    let target = ExternalPeerId::from_index(recipient.placement.play);
                    line.send_binary(target, &bytes[..length])
                        .map_err(debug_error)?;
                    fan.deliver(recipient).map_err(debug_error)?;
                }
                fan.take_terminal_value().map_err(debug_error)?;
            }
            Err(ExternalWebSocketError::Disconnected) => release(pool, members, current)?,
            Err(error) => return Err(debug_error(error)),
        }
        let Some(next) = line.next_connected_after(current) else {
            break;
        };
        current = next;
    }
    Ok(())
}

fn release(
    pool: &mut FixedSharedPool<MAXIMUM_PEERS, 256>,
    members: &mut [Option<MemberIdentity>; MAXIMUM_PEERS],
    peer: ExternalPeerId,
) -> Result<(), String> {
    let member = members[peer.index()]
        .take()
        .ok_or_else(|| "disconnect for unknown member".to_string())?;
    pool.request_release(member).map_err(debug_error)?;
    pool.complete_release(member).map_err(debug_error)
}

const fn empty_member() -> MemberIdentity {
    MemberIdentity {
        pool: PoolId(u16::MAX),
        key: MemberKey([0; 32]),
        slot: u16::MAX,
        epoch: 0,
        placement: MemberPlacement {
            node: NodeId(u16::MAX),
            realization: u16::MAX,
            play: u16::MAX,
        },
    }
}

fn debug_error(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
