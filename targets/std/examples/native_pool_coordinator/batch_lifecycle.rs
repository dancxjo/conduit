//! Persistent batch occupation and waiting; all policy stays in the common pool.
use super::*;
use crate::{evaluation, samples};
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Dispatch,
    Trigger { operation: PoolOperationId },
    FailPreparation { operation: PoolOperationId },
    Release { operation: PoolOperationId },
    Cancel { operation: PoolOperationId },
    Expire,
}
pub(super) fn read_document(reader: &mut impl Read, maximum: usize) -> Result<Vec<u8>, String> {
    let mut prefix = [0; 4];
    reader
        .read_exact(&mut prefix)
        .map_err(|_| "batch header unavailable")?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > maximum {
        return Err("batch document extent refused".into());
    }
    let mut bytes = vec![0; length];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| "batch document incomplete")?;
    Ok(bytes)
}
fn send(writer: &mut impl Write, value: serde_json::Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(&value).map_err(|_| "batch receipt invalid")?;
    if bytes.len() > 1024 * 1024 {
        return Err("batch receipt extent refused".into());
    }
    writer
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .and_then(|_| writer.write_all(&bytes))
        .and_then(|_| writer.flush())
        .map_err(|_| "batch receipt unavailable".into())
}
pub(super) fn run<'a>(
    input: &Input,
    initial: &[samples::CheckedSample<'_>],
    costs: &[NativePoolPreparedCosts<'a>],
    mut queue: NativePoolRequestQueue<'a, 2, 64, 32>,
    reader: &mut impl Read,
    writer: &mut impl Write,
) -> Result<(), String> {
    let mut members = [None; 3];
    let mut custody: [Option<NativePoolRequestCustody<'a>>; 3] = core::array::from_fn(|_| None);
    send(
        writer,
        json!({"phase":"native-batch-ready","waiting_population":queue.waiting_population(),"prepared_costs":costs.iter().map(|c| c.accounts().len()).collect::<Vec<_>>()}),
    )?;
    for _ in 0..64 {
        let bytes = read_document(reader, 4096)?;
        let command: Option<Command> =
            serde_json::from_slice(&bytes).map_err(|_| "batch lifecycle document invalid")?;
        let Some(command) = command else {
            if members.iter().any(Option::is_some) || queue.waiting_population() != 0 {
                send(
                    writer,
                    json!({"phase":"native-batch-stop-refused","waiting_population":queue.waiting_population(),"occupied":members.iter().filter(|m| m.is_some()).count()}),
                )?;
                continue;
            }
            return Ok(());
        };
        if matches!(command, Command::Dispatch | Command::Expire) {
            let refresh = evaluation::request(reader, writer)?;
            if refresh.samples.len() != initial.len()
                || refresh.admission.current.len() > 2
                || refresh.admission.queued_at != input.queued_at
                || refresh.admission.evaluated_at.clock() != refresh.preparation_received_at.clock()
                || refresh.admission.evaluated_at.ticks() < refresh.preparation_received_at.ticks()
            {
                return Err("batch refresh scope refused".into());
            }
            let current = refresh
                .samples
                .iter()
                .map(samples::admit)
                .collect::<Result<Vec<_>, _>>()?;
            for (old, new) in initial.iter().zip(&current) {
                evaluation::validate(&refresh.preparation_received_at, old, new)?;
                evaluation::validate_evaluation(
                    &refresh.admission.evaluated_at,
                    new.current_local_sample()?,
                )?;
            }
            if matches!(command, Command::Expire) {
                let retired = queue
                    .retire_expired(&refresh.admission.evaluated_at)
                    .map_err(|e| format!("batch expiry: {e:?}"))?;
                send(
                    writer,
                    json!({"phase":"native-batch-expired","operation":retired.as_ref().map(|r| &r.operation().operation),"waiting_population":queue.waiting_population()}),
                )?;
                continue;
            }
            let Some(request) = queue
                .next_request(&refresh.admission.evaluated_at)
                .map_err(|e| format!("batch next request: {e:?}"))?
            else {
                send(writer, json!({"phase":"native-batch-empty"}))?;
                continue;
            };
            let slot = input
                .operations
                .iter()
                .position(|op| op.operation == request.operation().operation)
                .ok_or("batch request correlation lost")?;
            let now = current
                .iter()
                .map(|s| s.current_upper_bound())
                .collect::<Result<Vec<_>, _>>()?;
            let revisions = current
                .iter()
                .map(|s| {
                    let revision = s.source().calibration();
                    PoolCostCalibrationRevision::new(
                        revision.identity().clone(),
                        revision.revision(),
                    )
                    .map(|r| [r])
                    .map_err(|e| format!("batch calibration: {e:?}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let evaluations = current
                .iter()
                .enumerate()
                .map(|(index, s)| PoolCostEvaluation {
                    host: s.source().host(),
                    now: &now[index],
                    calibrations: &revisions[index],
                    executions: &[],
                })
                .collect::<Vec<_>>();
            let started = std::time::Instant::now();
            let outcome = queue
                .dispatch(
                    &refresh.admission.evaluated_at,
                    &refresh.admission.current,
                    refresh.admission.sign.clone(),
                    &costs[slot],
                    &evaluations,
                )
                .map_err(|e| format!("batch dispatch: {e:?}"))?
                .ok_or("batch nonempty queue disappeared")?;
            let elapsed = started.elapsed().as_nanos();
            let account = format!("{:?}", outcome.admission.outcome.account);
            custody[slot] = outcome.request;
            match outcome.admission.outcome.admission {
                SharedPoolAdmissionOutcome::Selected { member, evidence } => {
                    if members[slot].is_some() {
                        return Err("batch duplicate occupation refused".into());
                    }
                    members[slot] = Some(member);
                    let original = custody[slot]
                        .as_ref()
                        .ok_or("batch selected custody absent")?;
                    send(
                        writer,
                        json!({"phase":"native-batch-selected","selection":evidence,"cost_account":account,"input_bytes":original.payload().encoded.len(),"input_digest":semantic_digest("conduit.std/local-model-completed-input@1", &original.payload().encoded),"decision_processing_ns":elapsed,"waiting_population":queue.waiting_population()}),
                    )?;
                }
                SharedPoolAdmissionOutcome::Refused(evidence) => {
                    send(
                        writer,
                        json!({"phase":"native-batch-refused","selection":evidence,"cost_account":account,"decision_processing_ns":elapsed,"waiting_population":queue.waiting_population()}),
                    )?;
                }
            }
            continue;
        }
        let (operation, terminal, trigger, cancel) = match command {
            Command::Trigger { operation } => (operation, false, true, false),
            Command::FailPreparation { operation } => (operation, true, false, false),
            Command::Release { operation } => (operation, true, true, false),
            Command::Cancel { operation } => (operation, true, false, true),
            _ => unreachable!("dispatch and expiry handled above"),
        };
        let slot = input
            .operations
            .iter()
            .position(|op| op.operation == operation)
            .ok_or("batch operation absent")?;
        if cancel {
            let retired = queue
                .cancel(&operation)
                .map_err(|e| format!("batch cancellation: {e:?}"))?;
            send(
                writer,
                json!({"phase":"native-batch-cancelled","operation":operation,"input_digest":semantic_digest("conduit.std/local-model-completed-input@1", &retired.payload().encoded),"waiting_population":queue.waiting_population()}),
            )?;
            continue;
        }
        let member = members[slot].ok_or("batch occupied operation absent")?;
        if !terminal {
            queue.trigger(member)
        } else if trigger {
            queue.release(member)
        } else {
            queue.fail_preparation(member)
        }
        .map_err(|e| format!("batch member lifecycle: {e:?}"))?;
        if terminal {
            members[slot] = None;
            custody[slot] = None;
        }
        send(
            writer,
            json!({"phase":if terminal {"native-batch-retired"} else {"native-batch-triggered"},"operation":operation,"occupied":members.iter().filter(|m| m.is_some()).count(),"waiting_population":queue.waiting_population()}),
        )?;
    }
    Err("batch lifecycle command budget exhausted".into())
}
