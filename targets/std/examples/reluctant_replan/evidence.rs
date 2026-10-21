use super::{clock::*, fixture};
use conduit_core::*;
use conduit_planner::resource_cost::*;
use conduit_std_host::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Path};
pub struct Log {
    file: fs::File,
    pub executions: usize,
}
impl Log {
    pub fn new(path: &Path) -> Result<Self, Error> {
        Ok(Self {
            file: fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(path)?,
            executions: 0,
        })
    }
    pub fn row(&mut self, value: &Value) -> Result<(), Error> {
        serde_json::to_writer(&mut self.file, value)?;
        self.file.write_all(b"\n")?;
        self.file.flush()?;
        Ok(())
    }
    pub fn execution(&mut self, label: &str, sample: &Sample) -> Result<(), Error> {
        if self.executions >= 1041 {
            return Err("finite execution ledger exhausted".into());
        }
        self.executions += 1;
        self.row(&json!({"case":"execution","request":label,"ordinal":self.executions,"receipt":sample.receipt,"queue":observation(&sample.queue)?,"service":observation(&sample.service)?}))
    }
}
pub fn digest(v: &Value) -> Result<String, Error> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(v)?)))
}
pub fn project(
    plan: &Plan,
    report: &StdRunReport,
    outputs: &fixture::Outputs,
) -> Result<(Value, SignId), Error> {
    let k = report.kernel.as_ref().ok_or("kernel receipt absent")?;
    if k.active_play.plan_id != plan.plan_id
        || k.active_play_id != k.active_play.active_play_id
        || k.value_allocation_capacity_before != k.value_allocation_capacity_after
        || usize::from(k.kernel_events) != k.kernel_sign.len()
        || outputs.0.len() != 1
        || &*outputs.0[0].bytes != 231u64.to_le_bytes().as_slice()
    {
        return Err("execution identity/output/storage differs".into());
    }
    let mut terminals = report.observations.iter().filter(|o| {
        o.plan_id.as_ref() == Some(&plan.plan_id)
            && o.active_play_id.as_ref() == Some(&k.active_play_id)
            && matches!(o.kind, ObservationKind::PlanTerminal { .. })
    });
    let terminal = terminals.next().ok_or("missing terminal")?;
    if terminals.next().is_some()
        || !matches!(
            terminal.kind,
            ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            }
        )
    {
        return Err("terminal not completed exactly once".into());
    }
    let signs=k.kernel_sign.iter().map(|e| {let conduit_kernel::KernelEvent {sequence,node,port,request,kind}=e;json!({"sequence":sequence,"node":node.0,"port":port.map(|p|p.0),"request":request.map(|r|r.0),"kind":format!("{kind:?}")})}).collect::<Vec<_>>();
    let output = &outputs.0[0];
    Ok((
        json!({"plan_id":plan.plan_id,"source_document_id":plan.source_document_id,"active_play":k.active_play,"terminal":terminal,"observations":report.observations,"kernel_sign":signs,"value_capacity_before":k.value_allocation_capacity_before,"value_capacity_after":k.value_allocation_capacity_after,
        "output":{"port":output.front_port_id,"kind":output.value_kind,"track":output.track,"sequence":output.sequence,"bytes":output.bytes}}),
        terminal.sign_id.clone(),
    ))
}
pub struct Sample {
    pub receipt: Value,
    pub terminal: SignId,
    pub queue: CostMeasuredDuration,
    pub service: CostMeasuredDuration,
}
pub fn submit(
    host: &mut StdHost,
    plan: &Plan,
    inputs: &[ExternalForeInput],
    clock: &MonotonicClockIdentity,
    arrival: Option<MonotonicInstant>,
) -> Result<Sample, Error> {
    let ready = now(clock)?;
    let arrival = arrival.unwrap_or_else(|| ready.clone());
    let mut outputs = fixture::Outputs::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            inputs,
            &mut outputs,
            &mut std::io::sink(),
            &mut fixture::NoTimer,
        )
        .map_err(fail)?;
    let (receipt, terminal) = project(plan, &report, &outputs)?;
    drop(report);
    drop(outputs);
    let end = now(clock)?;
    Ok(Sample {
        receipt,
        terminal,
        queue: CostMeasuredDuration::measure(&arrival, &ready, &mut work()).map_err(fail)?,
        service: CostMeasuredDuration::measure(&ready, &end, &mut work()).map_err(fail)?,
    })
}
pub fn decision(d: &ReluctantReplanDecision) -> Result<Value, Error> {
    let rows=d.observations.iter().map(|r|Ok(json!({"previous":attribution(&r.previous),"candidate":attribution(&r.candidate),"net_seconds":ratio(r.conservative_net_seconds)?}))).collect::<Result<Vec<_>,Error>>()?;
    Ok(
        json!({"previous_plan_id":d.previous_plan_id,"candidate_plan_id":d.candidate_plan_id,"at":d.evaluated_at,"keep_current":d.keep_current.map(|r|format!("{r:?}")),"overhead_upper":ratio(d.replacement_upper_seconds)?,"observations":rows,"replacement_sources":d.replacement_sources.iter().map(attribution).collect::<Vec<_>>(),"arithmetic_work":d.arithmetic_work_spent}),
    )
}

pub fn attribution(s: &CostSourceAttribution) -> Value {
    json!({"sign_id":s.sign_id,"host_id":s.host_id,"boot_id":s.boot_id,"capability_id":s.capability_id,"implementation_id":s.implementation_id,"artifact":s.implementation_artifact,"calibration_identity":s.calibration_identity,"calibration_revision":s.calibration_revision,"offer_generation":s.offer_generation,"host_profile":s.host_profile,"execution_profile_id":s.execution_profile_id,"clock":s.window.observed().clock(),"observed":s.window.observed().ticks().to_string(),"expires":s.window.expires().ticks().to_string()})
}
