//! Bounded multi-request custody with ordinary cost preparation and dispatch.
use conduit_core::*;
use conduit_kernel::{shared_pool::MemberKey, NodeId};
use conduit_plan_lowering::shared_pool_runtime::*;
use conduit_planner::resource_cost::*;
use conduit_std_host::*;
use serde::Deserialize;
use serde_json::json;
use std::io::{Read, Write};
#[path = "batch_lifecycle.rs"]
mod lifecycle;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Model {
    realization: u16,
    stages: super::costs::Stages,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    operation: PoolOperationId,
    requester: String,
    key: [u8; 32],
    demand_identity: [u8; 32],
    waiting_expires: MonotonicInstant,
    input: ValuePayload,
    models: Vec<Model>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    plan: Plan,
    pool: SharedPoolId,
    source: String,
    entry: String,
    tolerance: ValuePayload,
    queued_at: MonotonicInstant,
    samples: Vec<super::samples::Request>,
    operations: Vec<Operation>,
}
fn prepare(input: &Input, reader: &mut impl Read, writer: &mut impl Write) -> Result<(), String> {
    if input.source.len() > 16_384
        || input.entry.is_empty()
        || input.entry.len() > 256
        || input.samples.len() != 2
        || !(1..=3).contains(&input.operations.len())
    {
        return Err("batch preparation extent refused".into());
    }
    let bytes = input.operations.iter().try_fold(0usize, |total, op| {
        total.checked_add(op.input.encoded.len())
    });
    if bytes.is_none_or(|bytes| bytes > 65_536) {
        return Err("batch total input extent refused".into());
    }
    for (index, op) in input.operations.iter().enumerate() {
        if op.input.encoded.len() > 65_536
            || op.requester.is_empty()
            || op.requester.len() > 256
            || op.operation.as_str().len() > 256
            || op.models.len() > 2
            || input.operations[..index]
                .iter()
                .any(|other| other.operation == op.operation)
        {
            return Err("batch request extent or identity refused".into());
        }
        for (position, model) in op.models.iter().enumerate() {
            if model.realization > 1
                || op.models[..position]
                    .iter()
                    .any(|other| other.realization == model.realization)
            {
                return Err("batch forecast realization refused".into());
            }
        }
    }
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles)
        .map_err(|e| e.to_string())?;
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&input.source),
        &startup,
    )
    .map_err(|_| "batch Source failed checking")?;
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, &input.entry, &profiles)
            .map_err(|_| "batch Source failed expansion")?;
    if expanded.expanded.gears.len() != 1 {
        return Err("batch Source requires one Gear".into());
    }
    let gear = &expanded.expanded.gears[0];
    if gear.kind_id.as_str() != conduit_ai::LLM_GENERATE_KIND || gear.inputs.len() != 1 {
        return Err("batch generation profile changed".into());
    }
    let inputs = input
        .operations
        .iter()
        .map(|op| {
            [CostInput {
                port: &gear.inputs[0].port_id,
                value: &op.input,
            }]
        })
        .collect::<Vec<_>>();
    let codec_id = ArtifactId::from("conduit.proof/ollama-generation-utf8@1");
    let codecs = [CostInputCodec::admit(
        &gear.inputs[0].value_kind,
        &codec_id,
        65_536,
        super::costs::prompt,
    )
    .map_err(|e| format!("batch codec: {e:?}"))?];
    let demands = inputs
        .iter()
        .map(|inputs| {
            CostDemand::admit_with_catalog(gear, inputs, &[], &codecs)
                .map_err(|e| format!("batch demand: {e:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let requests = input
        .operations
        .iter()
        .zip(&demands)
        .map(|(op, demand)| {
            NativePoolRequestCustody::prepare(
                &input.plan,
                QueuedPoolOperation {
                    plan: input.plan.plan_id.clone(),
                    pool: input.pool.clone(),
                    operation: op.operation.clone(),
                    requester: op.requester.clone(),
                    class: PoolWaitingClass::Interactive,
                    key: MemberKey(op.key),
                    demand: op.demand_identity,
                    admitted_bytes: u32::try_from(demand.input_bytes())
                        .map_err(|_| "batch input extent")?,
                    waiting_expires: op.waiting_expires.clone(),
                },
                *demand,
            )
            .map_err(|e| format!("batch custody: {e:?}"))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let samples = input
        .samples
        .iter()
        .map(super::samples::admit)
        .collect::<Result<Vec<_>, _>>()?;
    let now = samples
        .iter()
        .map(|s| s.current_upper_bound())
        .collect::<Result<Vec<_>, _>>()?;
    let executions = samples
        .iter()
        .map(|s| {
            CostExecutionObservation::admit(s.source(), s.condition())
                .map_err(|e| format!("batch execution source: {e:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut work = ExactNumericWork::new(100_000_000);
    let tolerance = super::costs::quantity(&input.tolerance, &mut work)?;
    let estimates = input
        .operations
        .iter()
        .map(|op| {
            op.models
                .iter()
                .map(
                    |model| match super::costs::stages(&model.stages, &mut work) {
                        Ok(value) => Ok(Some(value)),
                        Err(error) if error == "MissingForecastStage" => Ok(None),
                        Err(error) => Err(error),
                    },
                )
                .collect::<Result<Vec<_>, String>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let forecasts = input
        .operations
        .iter()
        .zip(&estimates)
        .enumerate()
        .map(|(index, (op, estimates))| {
            op.models
                .iter()
                .zip(estimates)
                .map(|(model, estimate)| {
                    estimate
                        .as_ref()
                        .map(|estimate| {
                            CostedForecast::bind(
                                samples[usize::from(model.realization)].source(),
                                demands[index],
                                estimate,
                            )
                            .map_err(|e| format!("batch forecast: {e:?}"))
                        })
                        .transpose()
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let prepared = input
        .operations
        .iter()
        .enumerate()
        .map(|(index, op)| {
            let preparations = op
                .models
                .iter()
                .zip(&forecasts[index])
                .filter_map(|(model, forecast)| {
                    forecast.as_ref().map(|forecast| {
                        let slot = usize::from(model.realization);
                        PoolForecastPreparation {
                            plan: &input.plan,
                            fragment_index: 0,
                            pool: &input.pool,
                            operation: &op.operation,
                            demand_identity: op.demand_identity,
                            realization: model.realization,
                            request: demands[index],
                            forecast,
                            current_host: samples[slot].source().host(),
                            current_calibration: samples[slot].source().calibration(),
                            now: &now[slot],
                            current_execution: Some(&executions[slot]),
                            tolerance,
                        }
                    })
                })
                .collect();
            NativePoolPreparedCosts::prepare(&requests[index], preparations, &mut work)
                .map_err(|e| format!("batch private costs: {e:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let pool =
        HostedSharedPool::<2, 64, 32>::new(&input.plan, 0, input.pool.clone(), NodeId(64), 0)
            .map_err(|e| format!("batch pool: {e:?}"))?;
    let mut queue = NativePoolRequestQueue::prepare(
        pool,
        PoolWaitingLimits {
            operations: 3,
            per_requester: 3,
            admitted_bytes: 65_536,
        },
        PoolWaitingPolicy::Fifo,
        input.queued_at.clone(),
    )
    .map_err(|e| format!("batch queue: {e:?}"))?;
    for request in requests {
        queue
            .enqueue(request, &input.queued_at)
            .map_err(|e| format!("batch enqueue: {e:?}"))?;
    }
    lifecycle::run(input, &samples, &prepared, queue, reader, writer)
}
pub(super) fn run() -> Result<(), String> {
    let mut reader = std::io::stdin().lock();
    let mut writer = std::io::stdout().lock();
    let value = lifecycle::read_document(&mut reader, 1024 * 1024)?;
    let input: Input =
        serde_json::from_slice(&value).map_err(|_| "batch preparation document invalid")?;
    prepare(&input, &mut reader, &mut writer)
}
