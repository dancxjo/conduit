//! Prepare private request-bound costs from caller-admitted model estimates.
//! This neither authenticates serialized producers nor occupies a member. The
//! proof owner must admit its producing pipes and the supplied model definition.
use conduit_core::*;
use conduit_kernel::shared_pool::MemberKey;
use conduit_plan_lowering::shared_pool_runtime::{PoolWaitingClass, QueuedPoolOperation};
use conduit_planner::resource_cost::*;
use conduit_std_host::{NativePoolPreparedCosts, NativePoolRequestCustody};
use serde::Deserialize;
use serde_json::json;
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Range {
    lower: ValuePayload,
    upper: ValuePayload,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stages {
    setup: Option<Range>,
    queue: Option<Range>,
    service: Option<Range>,
    transfer: Option<Range>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Candidate {
    realization: u16,
    sample: super::samples::Request,
    stages: Stages,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    plan: Plan,
    pool: SharedPoolId,
    operation: PoolOperationId,
    requester: String,
    key: [u8; 32],
    demand_identity: [u8; 32],
    waiting_expires: MonotonicInstant,
    source: String,
    entry: String,
    input: ValuePayload,
    candidates: Vec<Candidate>,
    tolerance: ValuePayload,
}
fn quantity(value: &ValuePayload, work: &mut ExactNumericWork) -> Result<RationalQuantity, String> {
    if value.value_kind.as_str() != RATIONAL_QUANTITY_ID {
        return Err("forecast quantity semantic type changed".into());
    }
    RationalQuantity::decode(&value.encoded, work).map_err(|e| format!("forecast quantity: {e:?}"))
}
fn range(
    value: Option<&Range>,
    work: &mut ExactNumericWork,
) -> Result<Option<CostDurationRange>, String> {
    value
        .map(|value| {
            let lower = CostDuration::admit(quantity(&value.lower, work)?, work)
                .map_err(|e| format!("forecast lower Time: {e:?}"))?;
            let upper = CostDuration::admit(quantity(&value.upper, work)?, work)
                .map_err(|e| format!("forecast upper Time: {e:?}"))?;
            CostDurationRange::admit(lower, upper, work)
                .map_err(|e| format!("forecast interval: {e:?}"))
        })
        .transpose()
}
fn stages(value: &Stages, work: &mut ExactNumericWork) -> Result<FulfillmentCostForecast, String> {
    if [&value.setup, &value.queue, &value.service, &value.transfer]
        .iter()
        .any(|stage| stage.is_none())
    {
        return Err("MissingForecastStage".into());
    }

    FulfillmentCostForecast::sequential(
        CostForecastStages {
            setup: range(value.setup.as_ref(), work)?,
            queue: range(value.queue.as_ref(), work)?,
            service: range(value.service.as_ref(), work)?,
            transfer: range(value.transfer.as_ref(), work)?,
        },
        work,
    )
    .map_err(|e| format!("fulfillment stages: {e:?}"))
}
fn prompt(value: &[u8]) -> Result<(), CostEvidenceRefusal> {
    core::str::from_utf8(value)
        .map(|_| ())
        .map_err(|_| CostEvidenceRefusal::InvalidWorkInput)
}
fn prepare(input: &Input) -> Result<serde_json::Value, String> {
    if input.source.len() > 16_384
        || input.entry.is_empty()
        || input.entry.len() > 256
        || input.candidates.len() > 2
        || input.input.encoded.len() > 65_536
        || input.requester.is_empty()
        || input.requester.len() > 256
    {
        return Err("native cost preparation extent refused".into());
    }
    for (index, candidate) in input.candidates.iter().enumerate() {
        if input.candidates[..index]
            .iter()
            .any(|prior| prior.realization == candidate.realization)
        {
            return Err("duplicate native forecast realization".into());
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
    .map_err(|_| "cost Source failed checking")?;
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, &input.entry, &profiles)
            .map_err(|_| "cost Source failed expansion")?;
    if expanded.expanded.gears.len() != 1 {
        return Err("cost Source requires one checked Gear".into());
    }
    let gear = &expanded.expanded.gears[0];
    if gear.kind_id.as_str() != conduit_ai::LLM_GENERATE_KIND || gear.inputs.len() != 1 {
        return Err("cost Source generation profile changed".into());
    }
    let inputs = [CostInput {
        port: &gear.inputs[0].port_id,
        value: &input.input,
    }];
    let codec_id = ArtifactId::from("conduit.proof/ollama-generation-utf8@1");
    let codecs = [
        CostInputCodec::admit(&input.input.value_kind, &codec_id, 65_536, prompt)
            .map_err(|e| format!("cost input codec: {e:?}"))?,
    ];
    let demand = CostDemand::admit_with_catalog(gear, &inputs, &[], &codecs)
        .map_err(|e| format!("cost demand: {e:?}"))?;
    let metadata = QueuedPoolOperation {
        plan: input.plan.plan_id.clone(),
        pool: input.pool.clone(),
        operation: input.operation.clone(),
        requester: input.requester.clone(),
        class: PoolWaitingClass::Interactive,
        key: MemberKey(input.key),
        demand: input.demand_identity,
        admitted_bytes: u32::try_from(demand.input_bytes()).map_err(|_| "cost input extent")?,
        waiting_expires: input.waiting_expires.clone(),
    };
    let request = NativePoolRequestCustody::prepare(&input.plan, metadata, demand)
        .map_err(|e| format!("native cost custody: {e:?}"))?;
    let mut work = ExactNumericWork::new(100_000_000);
    let tolerance = quantity(&input.tolerance, &mut work)?;
    let samples = input
        .candidates
        .iter()
        .map(|c| super::samples::admit(&c.sample))
        .collect::<Result<Vec<_>, _>>()?;
    let now = samples
        .iter()
        .map(|s| s.current_upper_bound())
        .collect::<Result<Vec<_>, _>>()?;
    let mut refused = Vec::with_capacity(2);
    let estimates = input
        .candidates
        .iter()
        .map(|candidate| {
            if [
                &candidate.stages.setup,
                &candidate.stages.queue,
                &candidate.stages.service,
                &candidate.stages.transfer,
            ]
            .iter()
            .any(|stage| stage.is_none())
            {
                refused.push(
                    json!({"realization":candidate.realization,"reason":"MissingForecastStage"}),
                );
                Ok(None)
            } else {
                stages(&candidate.stages, &mut work).map(Some)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let forecasts = samples
        .iter()
        .zip(&estimates)
        .map(|(s, e)| {
            e.as_ref()
                .map(|e| {
                    CostedForecast::bind(s.source(), demand, e)
                        .map_err(|error| format!("request forecast: {error:?}"))
                })
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let executions = samples
        .iter()
        .map(|s| {
            CostExecutionObservation::admit(s.source(), s.condition())
                .map_err(|e| format!("current execution: {e:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let preparations = forecasts
        .iter()
        .enumerate()
        .filter_map(|(index, forecast)| {
            forecast.as_ref().map(|forecast| PoolForecastPreparation {
                plan: &input.plan,
                fragment_index: 0,
                pool: &input.pool,
                operation: &input.operation,
                demand_identity: input.demand_identity,
                realization: input.candidates[index].realization,
                request: demand,
                forecast,
                current_host: samples[index].source().host(),
                current_calibration: samples[index].source().calibration(),
                now: &now[index],
                current_execution: Some(&executions[index]),
                tolerance,
            })
        })
        .collect();
    let prepared = NativePoolPreparedCosts::prepare(&request, preparations, &mut work)
        .map_err(|e| format!("private native costs: {e:?}"))?;
    Ok(
        json!({"phase":"native-request-costs-prepared","prepared_candidates":prepared.accounts().len(),
        "refused_models":refused,"accounts":format!("{:?}",prepared.accounts()),"numeric_work":work.spent(),
        "input_bytes":demand.input_bytes(),"scope":"caller-admitted model estimates, not measured outcomes, authentication or dispatch"}),
    )
}
pub(super) fn run() -> Result<(), String> {
    let mut input = std::io::stdin().lock();
    let mut prefix = [0; 4];
    input
        .read_exact(&mut prefix)
        .map_err(|_| "native cost header unavailable")?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > 1024 * 1024 {
        return Err("native cost document extent refused".into());
    }
    let mut bytes = vec![0; length];
    input
        .read_exact(&mut bytes)
        .map_err(|_| "native cost document incomplete")?;
    let request = serde_json::from_slice(&bytes).map_err(|_| "native cost document invalid")?;
    let result =
        serde_json::to_vec(&prepare(&request)?).map_err(|_| "native cost receipt invalid")?;
    if result.len() > 1024 * 1024 {
        return Err("native cost receipt extent refused".into());
    }
    let mut output = std::io::stdout().lock();
    output
        .write_all(&(result.len() as u32).to_be_bytes())
        .and_then(|_| output.write_all(&result))
        .map_err(|_| "native cost receipt unavailable".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn payload(unit: &str, amount: i128) -> ValuePayload {
        let quantity = RationalQuantity::new(
            ExactRational::from_i128(amount),
            Unit::resolve(unit).unwrap(),
            QuantityRole::Linear,
        )
        .unwrap();
        let mut encoded = vec![0; RATIONAL_QUANTITY_MAX_ENCODED_BYTES];
        let length = quantity.encode(&mut encoded).unwrap();
        encoded.truncate(length);
        ValuePayload {
            value_kind: KindId::from(RATIONAL_QUANTITY_ID),
            encoded,
        }
    }
    #[test]
    fn memory_coordinate_cannot_become_a_fulfillment_time_stage() {
        let wrong = Range {
            lower: payload("B", 10),
            upper: payload("B", 20),
        };
        assert!(range(Some(&wrong), &mut ExactNumericWork::new(131_072)).is_err());
    }
    #[test]
    fn unknown_stage_is_not_zero_and_refuses_before_numeric_work() {
        let stage = || {
            Some(Range {
                lower: payload("us", 0),
                upper: payload("us", 0),
            })
        };
        let value = Stages {
            setup: None,
            queue: stage(),
            service: stage(),
            transfer: stage(),
        };
        let mut work = ExactNumericWork::new(131_072);
        assert_eq!(
            stages(&value, &mut work).unwrap_err(),
            "MissingForecastStage"
        );
        assert_eq!(work.spent(), 0);
    }
    #[test]
    fn inverted_and_negative_time_intervals_refuse() {
        for (lower, upper) in [(20, 10), (-1, 10)] {
            let bad = Range {
                lower: payload("us", lower),
                upper: payload("us", upper),
            };
            assert!(range(Some(&bad), &mut ExactNumericWork::new(131_072)).is_err());
        }
    }
}
