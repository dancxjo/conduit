//! Finite browser ABI over the common planned shared-pool controller.

use conduit_core::{
    Plan, PoolOperationId, PoolRealizationObservation, PoolSelectionEvidence, SharedPoolId, SignId,
};
use conduit_kernel::{
    shared_pool::{MemberIdentity, MemberKey, MemberPlacement, PoolId},
    NodeId,
};
use conduit_plan_lowering::shared_pool_runtime::{HostedSharedPool, SharedPoolAdmissionOutcome};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

const CAPACITY: usize = 256 * 1024;
const MEMBER_SLOTS: usize = 16;
const POOL_SIGNS: usize = 256;
const OBSERVATION_SIGNS: usize = 128;
const OK: i32 = 0;
const REFUSED: i32 = -1;

type BrowserSharedPool = HostedSharedPool<MEMBER_SLOTS, POOL_SIGNS, OBSERVATION_SIGNS>;

thread_local! {
    static INPUT: RefCell<Box<[u8; CAPACITY]>> = RefCell::new(Box::new([0; CAPACITY]));
    static OUTPUT: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(CAPACITY));
    static STATE: RefCell<Option<BrowserSharedPool>> = const { RefCell::new(None) };
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    plan: Plan,
    fragment_index: usize,
    pool_id: String,
    first_member_node: u16,
    play: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Admit {
    operation_id: String,
    member_key: Vec<u8>,
    observations: Vec<PoolRealizationObservation>,
    selection_sign_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberAction {
    member: MemberWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderLost {
    operation_id: String,
    member: MemberWire,
    observations: Vec<PoolRealizationObservation>,
    loss_sign_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberWire {
    pool: u16,
    key: Vec<u8>,
    slot: u16,
    epoch: u32,
    node: u16,
    realization: u16,
    play: u16,
}

fn member_wire(member: MemberIdentity) -> MemberWire {
    MemberWire {
        pool: member.pool.0,
        key: member.key.0.to_vec(),
        slot: member.slot,
        epoch: member.epoch,
        node: member.placement.node.0,
        realization: member.placement.realization,
        play: member.placement.play,
    }
}

fn member_identity(member: MemberWire) -> Result<MemberIdentity, String> {
    let key: [u8; 32] = member
        .key
        .try_into()
        .map_err(|_| "pool member key must contain exactly 32 bytes".to_string())?;
    Ok(MemberIdentity {
        pool: PoolId(member.pool),
        key: MemberKey(key),
        slot: member.slot,
        epoch: member.epoch,
        placement: MemberPlacement {
            node: NodeId(member.node),
            realization: member.realization,
            play: member.play,
        },
    })
}

fn input(length: u32) -> Result<Vec<u8>, String> {
    let length = usize::try_from(length).map_err(|_| "shared-pool input length overflow")?;
    if length == 0 || length > CAPACITY {
        return Err("shared-pool input exceeds its finite bound".into());
    }
    Ok(INPUT.with(|input| input.borrow()[..length].to_vec()))
}

fn write<T: Serialize>(value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.is_empty() || bytes.len() > CAPACITY {
        return Err("shared-pool output exceeds its finite bound".into());
    }
    OUTPUT.with(|output| {
        let mut output = output.borrow_mut();
        output.clear();
        output.extend_from_slice(&bytes);
    });
    Ok(())
}

fn refuse(message: impl Into<String>) -> i32 {
    let _ = write(&serde_json::json!({
        "schema": "conduit.browser/shared-pool-refusal@1",
        "message": message.into(),
    }));
    REFUSED
}

fn with_state(action: impl FnOnce(&mut BrowserSharedPool) -> Result<(), String>) -> i32 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(pool) = state.as_mut() else {
            return refuse("shared-pool runtime is not started");
        };
        action(pool).map_or_else(refuse, |()| OK)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_shared_pool_input_ptr() -> *mut u8 {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_shared_pool_input_capacity() -> u32 {
    CAPACITY as u32
}

#[no_mangle]
pub extern "C" fn conduit_browser_shared_pool_output_ptr() -> *const u8 {
    OUTPUT.with(|output| output.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_shared_pool_output_len() -> u32 {
    OUTPUT.with(|output| output.borrow().len() as u32)
}

#[no_mangle]
pub extern "C" fn conduit_browser_shared_pool_start(length: u32) -> i32 {
    STATE.with(|state| *state.borrow_mut() = None);
    OUTPUT.with(|output| output.borrow_mut().clear());
    let result = (|| -> Result<(), String> {
        let mut bytes = input(length)?;
        let start: Start = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode shared-pool start: {error}"))?;
        bytes.fill(0);
        let pool_id = SharedPoolId::from(start.pool_id);
        let pool = BrowserSharedPool::new(
            &start.plan,
            start.fragment_index,
            pool_id.clone(),
            NodeId(start.first_member_node),
            start.play,
        )
        .map_err(|error| format!("prepare shared pool: {error:?}"))?;
        write(&serde_json::json!({
            "schema": "conduit.browser/shared-pool-started@1",
            "plan_id": pool.plan_id(),
            "pool_id": pool_id,
            "population": pool.population(),
        }))?;
        STATE.with(|state| *state.borrow_mut() = Some(pool));
        Ok(())
    })();
    result.map_or_else(refuse, |()| OK)
}

#[no_mangle]
pub extern "C" fn conduit_browser_shared_pool_admit(length: u32) -> i32 {
    let result = (|| -> Result<Admit, String> {
        let mut bytes = input(length)?;
        let request = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode shared-pool admission: {error}"))?;
        bytes.fill(0);
        Ok(request)
    })();
    let request = match result {
        Ok(request) => request,
        Err(error) => return refuse(error),
    };
    with_state(|pool| {
        let key: [u8; 32] = request
            .member_key
            .try_into()
            .map_err(|_| "pool member key must contain exactly 32 bytes".to_string())?;
        let outcome = pool
            .admit_new_operation(
                PoolOperationId::from(request.operation_id.as_str()),
                MemberKey(key),
                &request.observations,
                SignId::from(request.selection_sign_id),
            )
            .map_err(|error| format!("admit shared-pool operation: {error:?}"))?;
        match outcome {
            SharedPoolAdmissionOutcome::Selected { member, evidence } => {
                write(&serde_json::json!({
                    "schema": "conduit.browser/shared-pool-selection@1",
                    "disposition": "selected",
                    "member": member_wire(member),
                    "evidence": evidence,
                    "population": pool.population(),
                }))
            }
            SharedPoolAdmissionOutcome::Refused(evidence) => write(&serde_json::json!({
                "schema": "conduit.browser/shared-pool-selection@1",
                "disposition": "refused",
                "evidence": evidence,
                "population": pool.population(),
            })),
        }
    })
}

fn member_action(
    length: u32,
    action: impl FnOnce(&mut BrowserSharedPool, MemberIdentity) -> Result<(), String>,
) -> i32 {
    let request = match input(length).and_then(|mut bytes| {
        let decoded = serde_json::from_slice::<MemberAction>(&bytes)
            .map_err(|error| format!("decode shared-pool member action: {error}"));
        bytes.fill(0);
        decoded
    }) {
        Ok(request) => request,
        Err(error) => return refuse(error),
    };
    let member = match member_identity(request.member) {
        Ok(member) => member,
        Err(error) => return refuse(error),
    };
    with_state(|pool| {
        action(pool, member)?;
        write(&serde_json::json!({
            "schema": "conduit.browser/shared-pool-member@1",
            "member": member_wire(member),
            "population": pool.population(),
        }))
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_shared_pool_trigger(length: u32) -> i32 {
    member_action(length, |pool, member| {
        pool.trigger(member)
            .map_err(|error| format!("trigger shared-pool member: {error:?}"))
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_shared_pool_fail_preparation(length: u32) -> i32 {
    member_action(length, |pool, member| {
        pool.fail_preparation(member)
            .map_err(|error| format!("fail shared-pool member preparation: {error:?}"))
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_shared_pool_release(length: u32) -> i32 {
    member_action(length, |pool, member| {
        pool.release(member)
            .map_err(|error| format!("release shared-pool member: {error:?}"))
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_shared_pool_provider_lost(length: u32) -> i32 {
    let request = match input(length).and_then(|mut bytes| {
        let decoded = serde_json::from_slice::<ProviderLost>(&bytes)
            .map_err(|error| format!("decode shared-pool provider loss: {error}"));
        bytes.fill(0);
        decoded
    }) {
        Ok(request) => request,
        Err(error) => return refuse(error),
    };
    let member = match member_identity(request.member) {
        Ok(member) => member,
        Err(error) => return refuse(error),
    };
    with_state(|pool| {
        let evidence: PoolSelectionEvidence = pool
            .provider_lost(
                PoolOperationId::from(request.operation_id.as_str()),
                member,
                &request.observations,
                SignId::from(request.loss_sign_id),
            )
            .map_err(|error| format!("record shared-pool provider loss: {error:?}"))?;
        write(&serde_json::json!({
            "schema": "conduit.browser/shared-pool-provider-loss@1",
            "member": member_wire(member),
            "evidence": evidence,
            "population": pool.population(),
        }))
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_shared_pool_close() {
    STATE.with(|state| *state.borrow_mut() = None);
    OUTPUT.with(|output| output.borrow_mut().clear());
}
