//! Read-only graph of externally verified retained evidence, never a Body contribution.
use crate::graph::{ChartGeometry, GraphRefusal, GraphSample, MAX_GRAPH_SAMPLES, quantity_milli};
use alloc::vec::Vec;
use conduit_core::Unit;
use conduit_environment_runtime::observed_record::{self, ObservedRecord};
use conduit_face::{GraphicsError, GraphicsScene};
/// Caller authenticates original signatures/binding before supplying these bytes.
/// This projection validates representation, extent and order, not custody authority.
pub struct RetainedEvidenceGraph {
    samples: Vec<GraphSample>,
    outside: u32,
}
impl RetainedEvidenceGraph {
    pub fn prepare(
        records: &[u8],
        verified_through: u32,
        outside: u32,
    ) -> Result<Self, GraphRefusal> {
        if records.is_empty()
            || !records.len().is_multiple_of(observed_record::BYTES)
            || records.len() > MAX_GRAPH_SAMPLES * observed_record::BYTES
        {
            return Err(GraphRefusal::TooLarge);
        }
        let mut samples = Vec::with_capacity(MAX_GRAPH_SAMPLES);
        let mut previous: Option<(u64, u64)> = None;
        for bytes in records.chunks_exact(observed_record::BYTES) {
            let record =
                ObservedRecord::decode(bytes).map_err(|_| GraphRefusal::InvalidProperty)?;
            if record.sequence > u64::from(verified_through)
                || previous.is_some_and(|(sequence, time)| {
                    record.sequence <= sequence || record.observed_at_boot_ms < time
                })
            {
                return Err(GraphRefusal::InvalidOrder);
            }
            let gap_before = previous.and_then(|(sequence, _)| {
                (record.sequence > sequence + 1).then_some((sequence + 1, record.sequence - 1))
            });
            let (temperature_milli_c, humidity_milli_percent) = match record.quantities {
                Some((temperature, humidity)) => (
                    Some(quantity_milli(temperature, Unit::Celsius)?),
                    Some(quantity_milli(humidity, Unit::Percent)?),
                ),
                None => (None, None),
            };
            samples.push(GraphSample {
                sequence: record.sequence,
                boot_ms: record.observed_at_boot_ms,
                temperature_milli_c,
                humidity_milli_percent,
                gap_before,
                failure: record.quantities.is_none(),
                failure_reason: None,
            });
            previous = Some((record.sequence, record.observed_at_boot_ms));
        }
        if previous.map(|(sequence, _)| sequence) != Some(u64::from(verified_through)) {
            return Err(GraphRefusal::InvalidOrder);
        }
        Ok(Self { samples, outside })
    }
    pub fn native_scene(&self) -> Result<GraphicsScene, GraphicsError> {
        self.native_scene_in_unit(Unit::Celsius)
    }
    pub fn native_scene_in_unit(&self, unit: Unit) -> Result<GraphicsScene, GraphicsError> {
        ChartGeometry {
            samples: &self.samples,
        }
        .native_scene(
            u64::from(self.outside),
            "RETAINED HISTORY / SIGNED EVIDENCE",
            unit,
        )
    }
    pub fn samples(&self) -> &[GraphSample] {
        &self.samples
    }
}
