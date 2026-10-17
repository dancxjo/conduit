//! Actual canonical Theremin Distance-to-Frequency control law, no hardware effect.
use super::*;
use conduit_core::{Quantity, Unit, QUANTITY_ENCODED_LEN};
use conduit_std_host::{ExternalForeStreamConfig, RunControl};
const INSTRUMENT: &str = include_str!("../../../../../../plots/pocket-theremin/main.conduit");
const ITEMS: usize = 1024;
fn source() -> String {
    let pitch = INSTRUMENT
        .split("\n# The one canonical Theremin.")
        .next()
        .expect("canonical pitch definition");
    let (_, expression) = pitch
        .rsplit_once(") = ")
        .expect("canonical pitch expression");
    // Explicit Flow expression: the canonical reusable function has Value Fores.
    // Substitute its one bound parameter, without changing its arithmetic law.
    let expression = expression.trim().replace("distance", ".");
    format!("{pitch}\nplot baseline (\n >> input: Distance...|\n output: Frequency...| >>\n) {{\n input >> ({expression}) >> output\n}}\n")
}
fn input(index: usize) -> Vec<u8> {
    let cm = (index % 41) as i64 - 5;
    if index.is_multiple_of(2) {
        Quantity::new(cm, Unit::Centimeter).encode().to_vec()
    } else {
        Quantity::new(cm * 10, Unit::Millimeter).encode().to_vec()
    }
}
fn expected(index: usize) -> Vec<u8> {
    let cm = (index % 41) as i64 - 5;
    Quantity::new(
        if cm <= 0 {
            1760
        } else if cm >= 30 {
            110
        } else {
            1760 - cm * 55
        },
        Unit::Hertz,
    )
    .encode()
    .to_vec()
}
fn native(inputs: &[ExternalForeInput]) -> Result<Vec<Vec<u8>>, String> {
    inputs
        .iter()
        .map(|input| {
            let distance = Quantity::decode(&input.bytes)
                .map_err(|e| format!("native Distance: {e:?}"))?
                .to_i64(Unit::Centimeter)
                .map_err(|e| format!("native centimetres: {e:?}"))?;
            let frequency = if distance <= 0 {
                1760
            } else if distance >= 30 {
                110
            } else {
                1760 - distance * 55
            };
            Ok(Quantity::new(frequency, Unit::Hertz).encode().to_vec())
        })
        .collect()
}
fn prepare(boot: &str, count: usize) -> Result<Prepared, Box<dyn Error>> {
    let mut prepared = prepare_queue_mode(
        1,
        &source(),
        boot,
        BackProfile::Hosted,
        false,
        1,
        QUANTITY_ENCODED_LEN as u32,
    )?;
    prepared.inputs = (0..count)
        .map(|i| ExternalForeInput {
            front_port_id: "input".into(),
            track: ConnectionTrack::Payload,
            bytes: input(i),
        })
        .collect();
    Ok(prepared)
}
fn validate(outputs: &Outputs, count: usize) -> Result<String, Box<dyn Error>> {
    if outputs.values.len() != count {
        return Err("control output count differs".into());
    }
    let mut bytes = Vec::with_capacity(count * QUANTITY_ENCODED_LEN);
    for (i, value) in outputs.values.iter().enumerate() {
        if value.front_port_id.as_str() != "output"
            || value.track != ConnectionTrack::Payload
            || value.value_kind.as_str() != conduit_core::FREQUENCY_INFO_ID
            || value.sequence != i as u64
            || value.bytes != expected(i)
        {
            return Err("control exact Frequency output differs".into());
        }
        bytes.extend_from_slice(&value.bytes);
    }
    Ok(artifact::digest(&bytes))
}
pub(super) fn run(
    config: &Config,
    raw: &mut artifact::Raw,
    summaries: &mut Vec<Value>,
) -> Result<(), Box<dyn Error>> {
    let id = "theremin-pitch-control";
    artifact::new_file(&config.output.join("sources").join(format!("{id}.plot")))?
        .write_all(source().as_bytes())?;
    artifact::new_file(
        &config
            .output
            .join("sources")
            .join(format!("{id}-instrument.plot")),
    )?
    .write_all(INSTRUMENT.as_bytes())?;
    artifact::json(
        &config
            .output
            .join("sources")
            .join(format!("{id}-inputs.json")),
        &json!({"inputs":(0..ITEMS).map(input).collect::<Vec<_>>(),"centimetre_unit":Unit::Centimeter.encode().as_slice(),"millimetre_unit":Unit::Millimeter.encode().as_slice(),"hertz_unit":Unit::Hertz.encode().as_slice(),"quantity_bytes":QUANTITY_ENCODED_LEN}),
    )?;
    let identity = artifact::digest(config.output.to_string_lossy().as_bytes());
    for repetition in 0..config.repetitions {
        let boot = format!("boot/runtime-control/{identity}/{repetition}");
        let mut prepared = prepare(&boot, ITEMS)?;
        artifact::json(
            &config
                .output
                .join("sources")
                .join(format!("{id}-{repetition}-plan.json")),
            &prepared.plan,
        )?;
        artifact::json(
            &config
                .output
                .join("sources")
                .join(format!("{id}-{repetition}-host.json")),
            prepared.host.advertisement(),
        )?;
        let mut native_run = None;
        if repetition % 2 == 0 {
            let (value, metrics) = timing::measure(|| native(&prepared.inputs));
            native_run = Some((value?, metrics));
        }
        let mut collector = Outputs::new(ITEMS);
        let mut phases = phase_observation::PhaseObserver::default();
        let (report, metrics) = timing::measure(|| {
            collector.begin();
            phases.begin();
            let result = prepared.host.run_external_plot_stream_controlled_to(
                prepared.plan.fragments[0].clone(),
                &prepared.inputs,
                &mut collector,
                &mut std::io::sink(),
                &mut phases,
                &RunControl::default(),
                ExternalForeStreamConfig { admission_batch: 8 },
            );
            phases.end();
            result
        });
        let report = report?;
        observation::completed(&report)?;
        if native_run.is_none() {
            let (value, metrics) = timing::measure(|| native(&prepared.inputs));
            native_run = Some((value?, metrics));
        }
        let (native_outputs, native_metrics) = native_run.unwrap();
        let digest = validate(&collector, ITEMS)?;
        if native_outputs
            .iter()
            .enumerate()
            .any(|(i, b)| *b != expected(i))
            || artifact::digest(&native_outputs.concat()) != digest
        {
            return Err("control native parity differs".into());
        }
        let kernel = report.kernel.as_ref().ok_or("control kernel absent")?;
        let ingress = kernel
            .external_fore_stream
            .ok_or("control ingress absent")?;
        if usize::from(ingress.accepted) != ITEMS
            || kernel.value_allocation_capacity_before != kernel.value_allocation_capacity_after
            || kernel
                .kernel_sign
                .iter()
                .filter(|e| e.kind == conduit_kernel::KernelEventKind::RemoteValueDelivered)
                .count()
                != ITEMS
        {
            return Err("control storage or mandatory Signs differ".into());
        }
        let phases = phases.report()?;
        raw.row(&json!({"case":id,"mode":"canonical-theremin-control","phase":"ordinary-domain-control","repetition":repetition,"source_sha256":artifact::digest(source().as_bytes()),"instrument_sha256":artifact::digest(INSTRUMENT.as_bytes()),"control_updates":ITEMS,"cold_phases":prepared.phases,"active_play":kernel.active_play,"checked_plot_id":prepared.plan.checked_plot_id,"full_host_call":metrics,"kernel_phase_wall":phases,"native_canonical_control":native_metrics,"ordered_output_sha256":digest,
            "output_observations":collector.report()?,"observations":report.observations,"signs":observation::signs(kernel),"kernel_decisions":kernel.decisions,"kernel_logical_invocations":kernel.logical_invocations,"ingress":ingress,"value_capacity_before":kernel.value_allocation_capacity_before,"value_capacity_after":kernel.value_allocation_capacity_after,"canonical_input_bytes":ITEMS*QUANTITY_ENCODED_LEN,"canonical_output_bytes":ITEMS*QUANTITY_ENCODED_LEN,
            "scope":"canonical authored Theremin pitch law with deterministic -5..35cm controls, alternating centimetre/millimetre frames and exact clamped Frequency results; normal checked Plan/std Back/kernel; native control uses identical canonical Quantity framing and exact integer conversion; no audio rendering, cadence, playback or physical/human proof; per-ack clock cost included, no unreported warm-up"}))?;
        summaries.push(json!({"case":id,"mode":"canonical-theremin-control","repetition":repetition,"full_host_call":metrics,"kernel_phase_wall":phases}));
        eprintln!("validated {id} repetition {}", repetition + 1);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_pitch_controls_preserve_units_clamps_and_exact_kernel_receipts(
    ) -> Result<(), Box<dyn Error>> {
        let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
        let mut prepared = prepare("boot/control-conformance", 128)?;
        let mut out = Outputs::new(128);
        let (result, _) = timing::measure(|| {
            out.begin();
            prepared.host.run_external_plot_stream_controlled_to(
                prepared.plan.fragments[0].clone(),
                &prepared.inputs,
                &mut out,
                &mut std::io::sink(),
                &mut NoTimer,
                &RunControl::default(),
                ExternalForeStreamConfig { admission_batch: 8 },
            )
        });
        let report = result?;
        observation::completed(&report)?;
        validate(&out, 128)?;
        assert_eq!(
            native(&prepared.inputs)?,
            out.values
                .iter()
                .map(|v| v.bytes.clone())
                .collect::<Vec<_>>()
        );
        let original = out.values[0].clone();
        out.values[0].bytes = Quantity::new(0, Unit::Hertz).encode().to_vec();
        assert!(validate(&out, 128).is_err());
        out.values[0] = original;
        for value in [vec![0, 1], Quantity::new(4, Unit::Hertz).encode().to_vec()] {
            let mut bad = prepared.inputs.clone();
            bad[127].bytes = value;
            let mut outputs = Outputs::new(128);
            let (result, _) = timing::measure(|| {
                outputs.begin();
                prepared.host.run_external_plot_stream_controlled_to(
                    prepared.plan.fragments[0].clone(),
                    &bad,
                    &mut outputs,
                    &mut std::io::sink(),
                    &mut NoTimer,
                    &RunControl::default(),
                    ExternalForeStreamConfig { admission_batch: 8 },
                )
            });
            assert!(result.is_err());
            assert!(outputs.values.is_empty());
        }
        Ok(())
    }
}
