//! Read-only request-bound forecast from the worker's private retained history.
use super::{attribution_owned, encoded, Retained};
use conduit_core::{ArtifactId, ValuePayload};
use conduit_planner::resource_cost::{CostDemand, CostInput, CostInputCodec};
use conduit_std_host::StdHost;
use serde_json::{json, Value};

pub(crate) fn query(
    host: &StdHost,
    retained: Option<&Retained>,
    source: &str,
    entry: &str,
    input: &ValuePayload,
) -> Value {
    let result = (|| -> Result<Value, String> {
        let gear = super::super::request::check(source, entry, input)?;
        let inputs = [CostInput {
            port: &gear.inputs[0].port_id,
            value: input,
        }];
        let revision = ArtifactId::from("conduit.proof/ollama-generation-utf8@1");
        let codecs = [CostInputCodec::admit(
            &input.value_kind,
            &revision,
            65_536,
            super::super::validate_prompt,
        )
        .map_err(|e| format!("service query codec: {e:?}"))?];
        let demand = CostDemand::admit_with_catalog(&gear, &inputs, &[], &codecs)
            .map_err(|e| format!("service query demand: {e:?}"))?;
        let prepared = retained
            .ok_or("retained service calibration absent")?
            .prepare(host, demand)?;
        let service = prepared
            .forecast
            .stages()
            .service
            .ok_or("retained service stage absent")?;
        Ok(
            json!({"phase":"worker-service-forecast","scope":"adapter_call_only",
            "claim":"last successful observation mean; not a future-outcome confidence bound",
            "input_bytes":input.encoded.len(),
            "input_digest":conduit_core::semantic_digest("conduit.std/local-model-completed-input@1", &input.encoded),
            "lower":encoded(service.lower().original())?,"upper":encoded(service.upper().original())?,
            "source":attribution_owned(prepared.source.as_ref().ok_or("retained service attribution absent")?)}),
        )
    })();
    match result {
        Ok(value) => value,
        Err(reason) => json!({"phase":"service-forecast-refused","reason":reason}),
    }
}
