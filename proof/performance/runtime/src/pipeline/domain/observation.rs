//! Fixed-capacity acknowledgement custody shared by domain fixtures.
use super::*;
pub(in crate::pipeline) struct Outputs {
    pub(in crate::pipeline) values: Vec<ExternalForeDelivery>,
    pub(in crate::pipeline) offsets: Vec<u64>,
    start: Option<Instant>,
    first_allocations: Option<crate::allocation::Counts>,
    last_allocations: Option<crate::allocation::Counts>,
    capacity: usize,
}
impl Outputs {
    pub(in crate::pipeline) fn new(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
            offsets: Vec::with_capacity(capacity),
            start: None,
            first_allocations: None,
            last_allocations: None,
            capacity,
        }
    }
    pub(in crate::pipeline) fn begin(&mut self) {
        self.start = Some(Instant::now());
    }
    pub(in crate::pipeline) fn report(&self) -> Result<Value, Box<dyn Error>> {
        if self.values.len() != self.capacity {
            return Err("domain output count differs".into());
        }
        let deltas = self
            .offsets
            .windows(2)
            .map(|p| p[1] - p[0])
            .collect::<Vec<_>>();
        let window = self.offsets.last().unwrap() - self.offsets[0];
        let allocation = self
            .first_allocations
            .as_ref()
            .ok_or("first domain allocation snapshot absent")?
            .interval_to(
                self.last_allocations
                    .as_ref()
                    .ok_or("last domain allocation snapshot absent")?,
            )
            .ok_or("domain allocator counters changed")?;
        Ok(
            json!({"acknowledgement_offsets_ns":self.offsets,"inter_acknowledgement_distribution":(!deltas.is_empty()).then(||timing::distribution(&deltas)),"completion_window_outputs":self.values.len()-1,"first_to_last_acknowledgement_ns":window,
            "completion_window_useful_outputs_per_second":(window>0).then(||(self.values.len()-1) as f64 * 1e9/window as f64),"post_first_output_allocations":allocation,
            "outputs":self.values.iter().map(|v|json!({"port":v.front_port_id,"track":v.track,"value_kind":v.value_kind,"sequence":v.sequence,"canonical":v.bytes,"sha256":artifact::digest(&v.bytes)})).collect::<Vec<_>>() }),
        )
    }
}
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if self.values.len() == self.capacity {
            return Err("domain collector capacity exceeded".into());
        }
        if self.values.is_empty() {
            self.first_allocations = crate::allocation::snapshot();
        }
        self.offsets.push(timing::ns(
            self.start.ok_or("domain call origin absent")?.elapsed(),
        ));
        self.values.push(value);
        if self.values.len() == self.capacity {
            self.last_allocations = crate::allocation::snapshot();
        }
        Ok(())
    }
}
pub(in crate::pipeline) fn signs(
    kernel: &conduit_std_host::StdKernelExecutionReport,
) -> Vec<Value> {
    kernel.kernel_sign.iter().map(|e|json!({"sequence":e.sequence,"node":e.node.0,"port":e.port.map(|p|p.0),"request":e.request.map(|r|r.0),"kind":format!("{:?}",e.kind)})).collect()
}
pub(in crate::pipeline) fn completed(
    report: &conduit_std_host::StdRunReport,
) -> Result<(), Box<dyn Error>> {
    if !matches!(
        report.observations.last().map(|o| &o.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: conduit_core::TerminalDisposition::Completed
        })
    ) {
        return Err("domain Play did not complete".into());
    }
    Ok(())
}
