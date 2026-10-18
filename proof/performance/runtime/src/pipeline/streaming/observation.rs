//! Per-branch ordered output custody, acknowledged-event timing and fairness.
use super::*;
struct Sample {
    value: ExternalForeDelivery,
    offset_ns: u64,
}
pub(super) struct StreamCollector {
    case: Case,
    count: usize,
    start: Option<Instant>,
    samples: Vec<Sample>,
    first_allocations: Option<crate::allocation::Counts>,
    last_allocations: Option<crate::allocation::Counts>,
}
impl StreamCollector {
    pub(super) fn new(case: Case, count: usize) -> Self {
        Self {
            case,
            count,
            start: None,
            samples: Vec::with_capacity(count * case.branches),
            first_allocations: None,
            last_allocations: None,
        }
    }
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
    pub(super) fn begin(&mut self) {
        self.start = Some(Instant::now());
    }
    pub(super) fn report(&self) -> Result<Value, Box<dyn Error>> {
        if self.samples.len() != self.count * self.case.branches {
            return Err("stream output loss/duplication".into());
        }
        let mut counts = vec![0; self.case.branches];
        let mut last = vec![None; self.case.branches];
        let mut gaps = vec![0; self.case.branches];
        let mut offsets = Vec::with_capacity(self.samples.len());
        let mut receipts = Vec::with_capacity(self.samples.len());
        for (index, sample) in self.samples.iter().enumerate() {
            let value = &sample.value;
            let branch = (0..self.case.branches)
                .find(|b| value.front_port_id.as_str() == format!("output{b}"))
                .ok_or("unplanned output Fore")?;
            if value.track != ConnectionTrack::Payload
                || value.sequence != counts[branch] as u64
                || value.bytes != self.case.expected(counts[branch])
                || value.value_kind.as_str()
                    != if self.case.text {
                        "value/text"
                    } else {
                        "value/u64"
                    }
            {
                return Err("stream exact output contract differs".into());
            }
            if let Some(previous) = last[branch] {
                gaps[branch] = gaps[branch].max(index - previous - 1);
            }
            counts[branch] += 1;
            last[branch] = Some(index);
            offsets.push(sample.offset_ns);
            receipts.push(json!({"branch":branch,"sequence":value.sequence,"bytes":value.bytes.len(),"sha256":artifact::digest(&value.bytes)}));
        }
        if counts.iter().any(|n| *n != self.count) || offsets.windows(2).any(|p| p[1] < p[0]) {
            return Err("stream branch order or clock differs".into());
        }
        let interval = offsets.last().unwrap() - offsets[0];
        let deltas = offsets.windows(2).map(|p| p[1] - p[0]).collect::<Vec<_>>();
        let mut canonical = Vec::new();
        for branch in 0..self.case.branches {
            for sample in &self.samples {
                if sample.value.front_port_id.as_str() == format!("output{branch}") {
                    canonical.extend_from_slice(&sample.value.bytes);
                }
            }
        }
        let allocation = self
            .first_allocations
            .as_ref()
            .ok_or("first stream allocation snapshot absent")?
            .interval_to(
                self.last_allocations
                    .as_ref()
                    .ok_or("last stream allocation snapshot absent")?,
            )
            .ok_or("stream allocator counters changed")?;
        Ok(
            json!({"per_branch_counts":counts,"max_other_acknowledgements_between_branch_outputs":gaps,"ordered_output_sha256":artifact::digest(&canonical),"output_receipts":receipts,
            "first_to_last_acknowledgement_ns":interval,"completion_window_outputs":offsets.len()-1,"completion_window_useful_outputs_per_second":(offsets.len()-1) as f64*1e9/interval as f64,
            "acknowledgement_offsets_ns":offsets,"inter_acknowledgement_distribution":timing::distribution(&deltas),"post_first_output_allocations":allocation,
            "fairness_scope":"observed other completed outputs between consecutive outputs of each branch; finite cooperative execution, not priority or preemption guarantee",
            "latency_scope":"elapsed from Host call entry to acknowledgement readiness; first-to-last and inter-event times include synchronous sink dwell and per-delivery Instant observation; not producer-to-consumer hardware latency"}),
        )
    }
}
impl ExternalForeOutputAdapter for StreamCollector {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if self.samples.len() == self.count * self.case.branches {
            return Err("stream collector capacity exceeded".into());
        }
        if self.samples.is_empty() {
            self.first_allocations = crate::allocation::snapshot();
        }
        let branch = value
            .front_port_id
            .as_str()
            .strip_prefix("output")
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|b| *b < self.case.branches)
            .ok_or("stream output has no admitted sink")?;
        self.case.acknowledge(branch);
        let offset_ns = timing::ns(self.start.ok_or("stream call origin missing")?.elapsed());
        self.samples.push(Sample { value, offset_ns });
        if self.samples.len() == self.count * self.case.branches {
            self.last_allocations = crate::allocation::snapshot();
        }
        Ok(())
    }
}
pub(super) fn validate_native(
    case: Case,
    count: usize,
    values: &[(usize, u64, Vec<u8>)],
) -> Result<String, Box<dyn Error>> {
    if values.len() != count * case.branches {
        return Err("native stream count differs".into());
    }
    let mut canonical = Vec::new();
    for branch in 0..case.branches {
        let mut sequence = 0;
        for (b, seq, bytes) in values {
            if *b == branch {
                if *seq != sequence || *bytes != case.expected(sequence as usize) {
                    return Err("native stream meaning differs".into());
                }
                canonical.extend_from_slice(bytes);
                sequence += 1;
            }
        }
        if sequence != count as u64 {
            return Err("native stream branch count differs".into());
        }
    }
    Ok(artifact::digest(&canonical))
}
