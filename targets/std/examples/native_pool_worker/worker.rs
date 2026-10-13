//! Bounded preparation documents followed by exact selected session frames.
//! No provider selection, retries, migration, or model download lives here.
mod calibration;
mod completion;
mod description;
mod pool_observation;
mod request;
mod sampling;

use conduit_ai::LocalModelKindProfile;
use conduit_core::{
    ArtifactId, BootId, HostId, OfferGeneration, PlacementId, Plan, PoolMemberSessionDirection,
    PoolSelectionEvidence, ValuePayload,
};
use conduit_planner::resource_cost::{CostDemand, CostEvidenceRefusal, CostInput, CostInputCodec};
use conduit_std_host::{
    hosted_local_model::{HostedLocalModelAdapter, LocalModelAdapterTerminal, OllamaDiscovery},
    pool_member_sessions::PoolMemberSession,
    StdHost, StdHostComposition, StdHostConfig,
};
use conduit_wire::{decode_session_frame, encode_session_frame_into, SessionMessage};
use serde::{de::DeserializeOwned, Deserialize};
use std::io::{Read, Write};

const CONTROL_BYTES: usize = 1024 * 1024;
const FRAME_BYTES: usize = 128 * 1024;
const OPERATIONS: usize = 64;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Initialize {
    endpoint: String,
    model: String,
    digest: String,
    host: String,
    boot: String,
    memory_mib: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    plan: Plan,
    selection: PoolSelectionEvidence,
    consumer: PlacementId,
    source: String,
    plot: String,
    input: ValuePayload,
    #[serde(default)]
    service_estimate: Option<calibration::Estimate>,
    #[serde(default)]
    reuse_service_calibration: bool,
}
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum PreparationCommand {
    Observe,
    ObserveClock,
    ObserveServiceForecast {
        source: String,
        entry: String,
        input: ValuePayload,
    },
    Describe {
        source: String,
        entry: String,
    },
    ObservePool {
        realization: Box<conduit_core::PoolRealizationEnvelope>,
    },
    Execute {
        operation: Box<Operation>,
    },
}

fn read_document<T: DeserializeOwned>(reader: &mut impl Read) -> Result<T, String> {
    let mut length = [0; 4];
    reader
        .read_exact(&mut length)
        .map_err(|_| "preparation header unavailable")?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > CONTROL_BYTES {
        return Err("preparation extent refused".into());
    }
    let mut bytes = vec![0; length];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| "preparation document incomplete")?;
    serde_json::from_slice(&bytes).map_err(|_| "preparation document invalid".into())
}
fn document(writer: &mut impl Write, value: &impl serde::Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|_| "receipt serialization failed")?;
    if bytes.len() > CONTROL_BYTES {
        return Err("receipt extent refused".into());
    }
    writer
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .map_err(io_error)?;
    writer.write_all(&bytes).map_err(io_error)?;
    writer.flush().map_err(io_error)
}
fn io_error(_: std::io::Error) -> String {
    "owned worker transport failed".into()
}
fn send(
    writer: &mut impl Write,
    session: &mut PoolMemberSession,
    buffer: &mut [u8],
    message: SessionMessage<'_>,
) -> Result<(), String> {
    let binding = session.binding();
    let frame = binding.frame(message);
    let length = encode_session_frame_into(
        frame,
        buffer,
        binding.limits.maximum_payload_bytes,
        binding.attachment.limits.maximum_frame_bytes,
    )
    .map_err(|e| format!("encode session: {e:?}"))?;
    session
        .admit_outbound_message(message)
        .map_err(|e| format!("outbound session: {e:?}"))?;
    writer
        .write_all(&(length as u32).to_be_bytes())
        .map_err(io_error)?;
    writer.write_all(&buffer[..length]).map_err(io_error)?;
    writer.flush().map_err(io_error)
}
fn receive<'a>(
    reader: &mut impl Read,
    session: &mut PoolMemberSession,
    buffer: &'a mut [u8],
) -> Result<SessionMessage<'a>, String> {
    let mut length = [0; 4];
    reader.read_exact(&mut length).map_err(io_error)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0
        || length > buffer.len()
        || length > session.binding().attachment.limits.maximum_frame_bytes as usize
    {
        return Err("session frame extent refused".into());
    }
    reader.read_exact(&mut buffer[..length]).map_err(io_error)?;
    let frame = decode_session_frame(
        &buffer[..length],
        session.binding().limits.maximum_payload_bytes,
        session.binding().attachment.limits.maximum_frame_bytes,
    )
    .map_err(|e| format!("decode session: {e:?}"))?;
    session
        .machine_mut()
        .admit_inbound(frame)
        .map_err(|e| format!("inbound session: {e:?}"))?;
    Ok(frame.message)
}
fn handshake(
    reader: &mut impl Read,
    writer: &mut impl Write,
    session: &mut PoolMemberSession,
    input: &mut [u8],
    output: &mut [u8],
) -> Result<(), String> {
    // The coordinator initiates both handshakes; roles still come from the Plan.
    if !matches!(receive(reader, session, input)?, SessionMessage::Hello(_)) {
        return Err("session omitted Hello".into());
    }
    let binding = session.binding().clone();
    send(writer, session, output, binding.hello_frame().message)?;
    if !matches!(receive(reader, session, input)?, SessionMessage::Ready) {
        return Err("session omitted Ready".into());
    }
    send(writer, session, output, SessionMessage::Ready)
}
fn validate_prompt(bytes: &[u8]) -> Result<(), CostEvidenceRefusal> {
    std::str::from_utf8(bytes)
        .map(|_| ())
        .map_err(|_| CostEvidenceRefusal::InvalidWorkInput)
}

pub(super) fn run() -> Result<(), String> {
    let mut reader = std::io::stdin().lock();
    let mut writer = std::io::stdout().lock();
    let init: Initialize = read_document(&mut reader)?;
    for value in [
        &init.endpoint,
        &init.model,
        &init.digest,
        &init.host,
        &init.boot,
    ] {
        if value.is_empty() || value.len() > 256 {
            return Err("initial identity extent refused".into());
        }
    }
    let discovery = OllamaDiscovery::discover_at(&init.endpoint, &init.model)?;
    if discovery.model_content_identity != init.digest {
        return Err("pinned model digest changed".into());
    }
    // Initialization performs the documented bounded warmup. Never label a
    // subsequent request cold merely because this worker process is new.
    let adapter = discovery.initialize(init.memory_mib, vec![LocalModelKindProfile::Generate])?;
    let identity = adapter.offer().identity.clone();
    let mut host = StdHost::new_with_local_model(
        StdHostConfig {
            host_id: HostId::from(init.host),
            boot_id: BootId::from(init.boot),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(adapter),
    )?;
    let mut sampler = sampling::prepare(&host)?;
    let mut service_history = None;
    let initial_sample = sampling::observe(&host, &mut sampler)?;
    document(
        &mut writer,
        &serde_json::json!({"phase":"initialized-after-warmup",
        "identity":identity, "advertisement":host.advertisement(), "cost_sample":initial_sample}),
    )?;
    for command_index in 0..OPERATIONS {
        let command: Option<PreparationCommand> = read_document(&mut reader)?;
        let op = match command {
            None => return Ok(()),
            Some(PreparationCommand::ObserveClock) => {
                document(&mut writer, &completion::clock(&host)?)?;
                continue;
            }
            Some(PreparationCommand::Observe) => {
                let reply = match sampling::observe(&host, &mut sampler) {
                    Ok(sample) => serde_json::json!({"phase":"observed","cost_sample":sample}),
                    Err(reason) => {
                        serde_json::json!({"phase":"observation-refused","reason":reason})
                    }
                };
                document(&mut writer, &reply)?;
                continue;
            }
            Some(PreparationCommand::Describe { source, entry }) => {
                let reply = match description::describe(&host, &source, &entry) {
                    Ok((plan, realization)) => {
                        serde_json::json!({"phase":"member-described","local_plan":plan,"realization":realization})
                    }
                    Err(reason) => {
                        serde_json::json!({"phase":"member-description-refused","reason":reason})
                    }
                };
                document(&mut writer, &reply)?;
                continue;
            }
            Some(PreparationCommand::ObservePool { realization }) => {
                let reply = match pool_observation::observe(&host, &realization, command_index) {
                    Ok(observation) => {
                        serde_json::json!({"phase":"pool-observed","observation":observation})
                    }
                    Err(reason) => {
                        serde_json::json!({"phase":"pool-observation-refused","reason":reason})
                    }
                };
                document(&mut writer, &reply)?;
                continue;
            }
            Some(PreparationCommand::ObserveServiceForecast {
                source,
                entry,
                input,
            }) => {
                let value =
                    calibration::query(&host, service_history.as_ref(), &source, &entry, &input);
                document(&mut writer, &value)?;
                continue;
            }
            Some(PreparationCommand::Execute { operation }) => operation,
        };
        let checked_gear = request::check(&op.source, &op.plot, &op.input)?;
        let gear = &checked_gear;
        let inputs = [CostInput {
            port: &gear.inputs[0].port_id,
            value: &op.input,
        }];
        let revision = ArtifactId::from("conduit.proof/ollama-generation-utf8@1");
        let codecs =
            [
                CostInputCodec::admit(&op.input.value_kind, &revision, 65_536, validate_prompt)
                    .map_err(|_| "operation codec admission refused")?,
            ];
        let demand = CostDemand::admit_with_catalog(gear, &inputs, &[], &codecs)
            .map_err(|_| "operation demand refused")?;
        let service_forecast = match calibration::prepare(
            &host,
            op.service_estimate.as_ref(),
            op.reuse_service_calibration,
            service_history.as_ref(),
            demand,
        ) {
            Ok(value) => value,
            Err(reason) => {
                document(
                    &mut writer,
                    &serde_json::json!({"phase":"service-forecast-refused","reason":reason}),
                )?;
                continue;
            }
        };
        let mut member = host.prepare_local_model_pool_member_with_demand(
            &op.plan,
            op.selection,
            &op.consumer,
            demand,
        )?;
        if member.sessions().iter().count() != 2 {
            return Err("operation requires one input and one output session".into());
        }
        for session in member.sessions().iter() {
            if session.binding().attachment.limits.maximum_frame_bytes as usize > FRAME_BYTES {
                return Err("session exceeds worker frame budget".into());
            }
        }
        let mut input = vec![0; FRAME_BYTES];
        let mut output = vec![0; FRAME_BYTES];
        let mut result = Vec::with_capacity(65_536);
        document(
            &mut writer,
            &serde_json::json!({"phase":"admitted","operation":member.sessions().selection().operation_id}),
        )?;
        for session in member.sessions_mut().iter_mut() {
            handshake(&mut reader, &mut writer, session, &mut input, &mut output)?;
        }
        let session = member
            .sessions_mut()
            .iter_mut()
            .find(|s| s.direction == PoolMemberSessionDirection::Input)
            .ok_or("input session absent")?;
        let request = match receive(&mut reader, session, &mut input)? {
            SessionMessage::Offered {
                sequence: 0,
                payload,
            } => payload,
            _ => return Err("operation omitted first Offered frame".into()),
        };
        send(
            &mut writer,
            session,
            &mut output,
            SessionMessage::Accepted { sequence: 0 },
        )?;
        let terminal = host.execute_local_model_pool_member(&mut member, request, &mut result);
        let session = member
            .sessions_mut()
            .iter_mut()
            .find(|s| s.direction == PoolMemberSessionDirection::Input)
            .unwrap();
        send(
            &mut writer,
            session,
            &mut output,
            SessionMessage::Delivered { sequence: 0 },
        )?;
        if !matches!(
            receive(&mut reader, session, &mut input)?,
            SessionMessage::InputClosed { final_sequence: 1 }
        ) {
            return Err("request input closure refused".into());
        }
        let session = member
            .sessions_mut()
            .iter_mut()
            .find(|s| s.direction == PoolMemberSessionDirection::Output)
            .unwrap();
        if matches!(
            terminal,
            LocalModelAdapterTerminal::Produced | LocalModelAdapterTerminal::Truncated
        ) {
            send(
                &mut writer,
                session,
                &mut output,
                SessionMessage::Offered {
                    sequence: 0,
                    payload: &result,
                },
            )?;
            if !matches!(
                receive(&mut reader, session, &mut input)?,
                SessionMessage::Accepted { sequence: 0 }
            ) || !matches!(
                receive(&mut reader, session, &mut input)?,
                SessionMessage::Delivered { sequence: 0 }
            ) {
                return Err("result acknowledgement refused".into());
            }
            send(
                &mut writer,
                session,
                &mut output,
                SessionMessage::InputClosed { final_sequence: 1 },
            )?;
        } else {
            send(
                &mut writer,
                session,
                &mut output,
                SessionMessage::Cancelled { code: 1 },
            )?;
        }
        let completed_work = completion::prepare(&member);
        let service_calibration = calibration::finish(
            &host,
            &member,
            demand,
            service_forecast.as_ref(),
            &mut service_history,
        );
        host.release_local_model_pool_member(member)?;
        document(
            &mut writer,
            &serde_json::json!({"phase":"released","terminal":format!("{terminal:?}"),"output_bytes":result.len(),"completed_work":completed_work,"service_calibration":service_calibration}),
        )?;
    }
    Err("worker operation budget exhausted".into())
}
