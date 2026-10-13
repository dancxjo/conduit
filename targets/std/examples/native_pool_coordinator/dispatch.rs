//! One retained original request and the existing queue/kernel lifecycle.
//! Cost comparisons and occupation remain in the generic dispatcher.
use conduit_core::*;
use conduit_kernel::NodeId;
use conduit_plan_lowering::shared_pool_runtime::{
    HostedSharedPool, PoolCostEvaluation, PoolWaitingLimits, PoolWaitingPolicy,
    SharedPoolAdmissionOutcome,
};
use conduit_std_host::{NativePoolPreparedCosts, NativePoolRequestCustody, NativePoolRequestQueue};
use serde::Deserialize;
use serde_json::json;
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Admission {
    pub(super) queued_at: MonotonicInstant,
    pub(super) evaluated_at: MonotonicInstant,
    pub(super) current: Vec<PoolRealizationObservation>,
    pub(super) sign: SignId,
}
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Trigger,
    FailPreparation,
    Release,
    CancelWaiting,
}
fn send(output: &mut impl Write, value: &serde_json::Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|_| "native dispatch receipt invalid")?;
    if bytes.len() > 1024 * 1024 {
        return Err("native dispatch receipt extent refused".into());
    }
    output
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .and_then(|_| output.write_all(&bytes))
        .and_then(|_| output.flush())
        .map_err(|_| "native dispatch receipt unavailable".into())
}
fn read(input: &mut impl Read) -> Result<Option<Command>, String> {
    let mut prefix = [0; 4];
    input
        .read_exact(&mut prefix)
        .map_err(|_| "native lifecycle header unavailable")?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > 4096 {
        return Err("native lifecycle extent refused".into());
    }
    let mut bytes = vec![0; length];
    input
        .read_exact(&mut bytes)
        .map_err(|_| "native lifecycle incomplete")?;
    serde_json::from_slice(&bytes).map_err(|_| "native lifecycle document invalid".into())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn run<'a>(
    plan: &Plan,
    pool_id: &SharedPoolId,
    request: NativePoolRequestCustody<'a>,
    costs: &NativePoolPreparedCosts<'a>,
    evaluations: &[PoolCostEvaluation<'_>],
    admission: &Admission,
    reader: &mut impl Read,
    writer: &mut impl Write,
) -> Result<(), String> {
    if admission.current.len() > 2 {
        return Err("native dispatch observation bound refused".into());
    }
    let pool = HostedSharedPool::<2, 64, 32>::new(plan, 0, pool_id.clone(), NodeId(64), 0)
        .map_err(|e| format!("native pool preparation: {e:?}"))?;
    let mut queue = NativePoolRequestQueue::prepare(
        pool,
        PoolWaitingLimits {
            operations: 1,
            per_requester: 1,
            admitted_bytes: 65_536,
        },
        PoolWaitingPolicy::Fifo,
        admission.queued_at.clone(),
    )
    .map_err(|e| format!("native waiting preparation: {e:?}"))?;
    let operation = request.operation().operation.clone();
    queue
        .enqueue(request, &admission.queued_at)
        .map_err(|e| format!("native request queue: {e:?}"))?;
    let decision_started = std::time::Instant::now();
    let dispatched = queue
        .dispatch(
            &admission.evaluated_at,
            &admission.current,
            admission.sign.clone(),
            costs,
            evaluations,
        )
        .map_err(|e| format!("native cost dispatch: {e:?}"))?
        .ok_or("native queue unexpectedly empty")?;
    let decision_processing_ns = decision_started.elapsed().as_nanos();
    let account = format!("{:?}", dispatched.admission.outcome.account);
    let cost_explanation = conduit_observatory::render_pool_cost_selection(
        plan,
        &dispatched.admission.outcome.account,
    )
    .map_err(|reason| format!("{reason:?}"));
    // Keep retired native byte custody until the explicit terminal lifecycle.
    let retained_request = dispatched.request;
    let mut member = match dispatched.admission.outcome.admission {
        SharedPoolAdmissionOutcome::Selected { member, evidence } => {
            let request = retained_request
                .as_ref()
                .ok_or("selected native request custody absent")?;
            send(
                writer,
                &json!({"phase":"native-cost-selected","decision_processing_ns":decision_processing_ns,
                "decision_scope":"generic queue dispatch only; includes checked cost comparison and atomic admission, excludes forecast preparation, JSON and native execution",
                "selection":evidence,"cost_account":account,"cost_explanation":cost_explanation,
                "input_bytes":request.payload().encoded.len(),"input_digest":semantic_digest("conduit.std/local-model-completed-input@1",&request.payload().encoded),
                "scope":"one original request; caller-admitted estimates; effects require exact native member preparation and explicit trigger"}),
            )?;
            Some(member)
        }
        SharedPoolAdmissionOutcome::Refused(evidence) => {
            send(
                writer,
                &json!({"phase":"native-cost-refused","decision_processing_ns":decision_processing_ns,
                "decision_scope":"generic queue dispatch only; includes checked cost comparison and atomic admission, excludes forecast preparation, JSON and native execution",
                "selection":evidence,"cost_account":account,"cost_explanation":cost_explanation,
                "waiting_population":queue.waiting_population()}),
            )?;
            None
        }
    };
    for _ in 0..64 {
        let Some(command) = read(reader)? else {
            if member.is_some() || queue.waiting_population() != 0 {
                send(
                    writer,
                    &json!({"phase":"native-stop-refused","waiting_population":queue.waiting_population(),"occupied":member.is_some()}),
                )?;
                continue;
            }
            drop(retained_request);
            return Ok(());
        };
        if matches!(command, Command::CancelWaiting) {
            let retired = queue
                .cancel(&operation)
                .map_err(|e| format!("native waiting cancellation: {e:?}"))?;
            send(
                writer,
                &json!({"phase":"native-waiting-cancelled","input_bytes":retired.payload().encoded.len(),"waiting_population":queue.waiting_population()}),
            )?;
            continue;
        }
        let selected = member.ok_or("native occupied member absent")?;
        let phase = match command {
            Command::CancelWaiting => unreachable!("handled before occupied-member lifecycle"),
            Command::Trigger => {
                queue
                    .trigger(selected)
                    .map_err(|e| format!("native trigger: {e:?}"))?;
                "native-triggered"
            }
            Command::FailPreparation => {
                queue
                    .fail_preparation(selected)
                    .map_err(|e| format!("native preparation retirement: {e:?}"))?;
                member = None;
                "native-preparation-retired"
            }
            Command::Release => {
                queue
                    .release(selected)
                    .map_err(|e| format!("native release: {e:?}"))?;
                member = None;
                "native-released"
            }
        };
        send(
            writer,
            &json!({"phase":phase,"occupied":member.is_some(),"waiting_population":queue.waiting_population()}),
        )?;
    }
    Err("native lifecycle command budget exhausted".into())
}
