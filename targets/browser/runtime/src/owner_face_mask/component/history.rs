//! Finite DOM material from the exact checked history collection, not a recorder.
use conduit_environment_history::{
    GraphRefusal, HistoryGraph, HistoryTemperaturePresentation, HISTORY_SUBJECT,
};
use conduit_face::{Face, LookInfo};
use serde::Serialize;

#[derive(Clone, Serialize)]
pub(in crate::owner_face_mask) struct HistoryView {
    schema: &'static str,
    face_id: String,
    face_revision: String,
    subject: String,
    producer: [String; 4],
    omitted: String,
    temperature_unit: &'static str,
    temperature_top: String,
    temperature_bottom: String,
    samples: Vec<SampleView>,
}

#[derive(Clone, Serialize)]
struct SampleView {
    sequence: String,
    boot_ms: String,
    temperature: Option<String>,
    temperature_source: Option<String>,
    humidity: Option<String>,
    x: u16,
    temperature_y: Option<u16>,
    humidity_y: Option<u16>,
    failure: Option<&'static str>,
    gap_before: Option<[String; 2]>,
}

fn milli(value: i64) -> String {
    let value = i128::from(value);
    let sign = if value < 0 { "-" } else { "" };
    let magnitude = value.abs();
    format!("{sign}{}.{:03}", magnitude / 1000, magnitude % 1000)
}

pub(super) fn prepare(face: &Face, look: &LookInfo) -> Result<HistoryView, GraphRefusal> {
    let graph = HistoryGraph::prepare(face, HISTORY_SUBJECT)?;
    let shown = HistoryTemperaturePresentation::prepare(&graph, look)?;
    Ok(HistoryView {
        schema: "conduit.browser/environment-history@2",
        face_id: graph.content_id().into(),
        face_revision: graph.revision().to_string(),
        subject: graph.subject().into(),
        producer: graph.producer_context().map(str::to_owned),
        omitted: graph.omitted().to_string(),
        temperature_unit: shown.unit_label(),
        temperature_top: shown.top.number(),
        temperature_bottom: shown.bottom.number(),
        samples: graph
            .samples()
            .iter()
            .zip(&shown.samples)
            .map(|(sample, temperature)| {
                Ok(SampleView {
                    // Preserve exact u64/i64 values across the JS boundary. Only
                    // already-bounded drawing coordinates become JS numbers.
                    sequence: sample.sequence().to_string(),
                    boot_ms: sample.boot_ms().to_string(),
                    temperature: temperature.as_ref().map(|value| value.number()),
                    temperature_source: sample
                        .temperature_source()
                        .map(|value| {
                            value
                                .canonical_literal()
                                .map_err(|_| GraphRefusal::InvalidProperty)
                        })
                        .transpose()?,
                    humidity: sample.humidity_milli_percent().map(milli),
                    x: graph.x(sample),
                    temperature_y: sample.temperature_milli_c().map(|v| graph.y(v, false)),
                    humidity_y: sample.humidity_milli_percent().map(|v| graph.y(v, true)),
                    failure: sample.failure_reason(),
                    gap_before: sample
                        .gap_before()
                        .map(|(a, b)| [a.to_string(), b.to_string()]),
                })
            })
            .collect::<Result<_, GraphRefusal>>()?,
    })
}
