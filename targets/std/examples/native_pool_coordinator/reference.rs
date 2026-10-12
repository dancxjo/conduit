//! Persistent missing-cost reference for the live proof. All eligibility,
//! choice, occupation and lifecycle remain owned by HostedSharedPool. This
//! mode is explicitly not the unfinished known-forecast dispatch path.
use conduit_core::{Plan, PoolOperationId, PoolRealizationObservation, SharedPoolId, SignId};
use conduit_kernel::{
    shared_pool::{MemberIdentity, MemberKey},
    NodeId,
};
use conduit_plan_lowering::shared_pool_runtime::{HostedSharedPool, SharedPoolAdmissionOutcome};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::json;
use std::io::{Read, Write};

const MAXIMUM_DOCUMENT: usize = 1024 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Initialize {
    plan: Plan,
    fragment: usize,
    pool: SharedPoolId,
}
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Admit {
        operation: PoolOperationId,
        key: [u8; 32],
        demand: [u8; 32],
        current: Vec<PoolRealizationObservation>,
        sign: SignId,
    },
    Trigger {
        slot: usize,
    },
    FailPreparation {
        slot: usize,
    },
    Release {
        slot: usize,
    },
}
fn read<T: DeserializeOwned>(input: &mut impl Read) -> Result<T, String> {
    let mut prefix = [0; 4];
    input
        .read_exact(&mut prefix)
        .map_err(|_| "reference header unavailable")?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > MAXIMUM_DOCUMENT {
        return Err("reference document extent refused".into());
    }
    let mut bytes = vec![0; length];
    input
        .read_exact(&mut bytes)
        .map_err(|_| "reference document incomplete")?;
    serde_json::from_slice(&bytes).map_err(|_| "reference document invalid".into())
}
fn send(output: &mut impl Write, value: &serde_json::Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|_| "reference receipt invalid")?;
    if bytes.len() > MAXIMUM_DOCUMENT {
        return Err("reference receipt extent refused".into());
    }
    output
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .map_err(|_| "reference receipt header failed")?;
    output
        .write_all(&bytes)
        .map_err(|_| "reference receipt failed")?;
    output
        .flush()
        .map_err(|_| "reference receipt flush failed".into())
}
pub(super) fn run() -> Result<(), String> {
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let initial: Initialize = read(&mut input)?;
    let mut pool = HostedSharedPool::<2, 64, 32>::new(
        &initial.plan,
        initial.fragment,
        initial.pool,
        NodeId(64),
        0,
    )
    .map_err(|e| format!("reference preparation: {e:?}"))?;
    let mut members: [Option<MemberIdentity>; 2] = [None, None];
    send(
        &mut output,
        &json!({"phase":"reference-ready","plan_id":pool.plan_id(),
        "cost_mode":"missing-estimates-capacity-fallback","population":pool.population()}),
    )?;
    for _ in 0..64 {
        let command: Option<Command> = read(&mut input)?;
        let Some(command) = command else {
            if pool.population() != 0 {
                send(
                    &mut output,
                    &json!({"phase":"reference-stop-refused", "population":pool.population(),
                    "reason":"occupied-members-require-explicit-lifecycle"}),
                )?;
                continue;
            }
            return Ok(());
        };
        let reply = match command {
            Command::Admit {
                operation,
                key,
                demand,
                current,
                sign,
            } => {
                if current.len() > 2 {
                    return Err("reference observation bound refused".into());
                }
                let admitted = pool
                    .admit_new_operation_with_costs(
                        operation,
                        MemberKey(key),
                        &current,
                        sign,
                        demand,
                        &[],
                        &[],
                    )
                    .map_err(|e| format!("reference admission: {e:?}"))?;
                let account = format!("{:?}", admitted.account);
                match admitted.admission {
                    SharedPoolAdmissionOutcome::Selected { member, evidence } => {
                        let slot = members
                            .iter()
                            .position(Option::is_none)
                            .ok_or("reference handle storage exhausted")?;
                        members[slot] = Some(member);
                        json!({"phase":"reference-selected","slot":slot,"selection":evidence,"cost_account":account})
                    }
                    SharedPoolAdmissionOutcome::Refused(evidence) => {
                        json!({"phase":"reference-refused","selection":evidence,"cost_account":account})
                    }
                }
            }
            Command::Trigger { slot } => {
                let member = members
                    .get(slot)
                    .and_then(|m| *m)
                    .ok_or("reference member absent")?;
                pool.trigger(member)
                    .map_err(|e| format!("reference trigger: {e:?}"))?;
                json!({"phase":"reference-triggered","slot":slot})
            }
            Command::FailPreparation { slot } => {
                let member = members
                    .get(slot)
                    .and_then(|m| *m)
                    .ok_or("reference member absent")?;
                pool.fail_preparation(member)
                    .map_err(|e| format!("reference failure release: {e:?}"))?;
                members[slot] = None;
                json!({"phase":"reference-preparation-retired","slot":slot})
            }
            Command::Release { slot } => {
                let member = members
                    .get(slot)
                    .and_then(|m| *m)
                    .ok_or("reference member absent")?;
                pool.release(member)
                    .map_err(|e| format!("reference release: {e:?}"))?;
                members[slot] = None;
                json!({"phase":"reference-released","slot":slot})
            }
        };
        send(
            &mut output,
            &json!({"outcome":reply,"population":pool.population()}),
        )?;
    }
    Err("reference command budget exhausted".into())
}
