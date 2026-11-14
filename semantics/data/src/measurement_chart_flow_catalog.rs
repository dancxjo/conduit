//! Explicit terminal-capable chart Face; original outputless Kind is unchanged.
use alloc::{string::String, vec};
use conduit_core::*;
pub const MEASUREMENT_CHART_FLOW_FACE_KIND: &str = "face/measurement-chart-flow";
pub const MEASUREMENT_CHART_FLOW_FACE_REVISION: &str = "conduit.data/face-measurement-chart-flow@1";
pub fn measurement_chart_flow_semantic_contract() -> Kind {
    let mut kind = crate::measurement_chart_face_semantic_contract();
    kind.kind_id = kind_id(MEASUREMENT_CHART_FLOW_FACE_KIND);
    kind.kind_contract_revision = MEASUREMENT_CHART_FLOW_FACE_REVISION.into();
    kind.inputs[0].abnormal_kind = Some(kind_id(TERMINAL_INFO_ID));
    let input = kind.inputs[0].port_id.clone();
    let output = port_id("done");
    kind.outputs = vec![PortDescriptor {
        port_id: output.clone(),
        value_kind: kind_id(EMPTY_INFO_ID),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: Some(kind_id(TERMINAL_INFO_ID)),
    }];
    let terminal = CheckedValueContract::new(
        kind_id(TERMINAL_INFO_ID),
        TERMINAL_INFO_ENCODED_LEN as u32,
        vec![],
    )
    .unwrap();
    let contracts = kind
        .semantic_laws
        .iter_mut()
        .find_map(|law| {
            if let KindSemanticLaw::ValueContracts(values) = law {
                Some(values)
            } else {
                None
            }
        })
        .expect("original exact chart input");
    contracts.extend([
        FrontValueContract {
            location: FrontValueLocation::Output(output.clone()),
            contract: CheckedValueContract::new(kind_id(EMPTY_INFO_ID), 0, vec![]).unwrap(),
        },
        FrontValueContract {
            location: FrontValueLocation::InputAbnormal(input.clone()),
            contract: terminal.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::OutputAbnormal(output.clone()),
            contract: terminal,
        },
    ]);
    kind.semantic_laws
        .push(KindSemanticLaw::TerminalTransduction(
            TerminalTransductionProfile {
                input_port_id: input,
                output_port_id: output,
                normal_close: NormalCloseTransduction::PropagateAfterDrain,
                abnormal: AbnormalTerminalTransduction::PropagateAfterDrain,
                cancellation: CancellationTransduction::NotCancellable,
            },
        ));
    kind
}
pub fn install_measurement_chart_flow_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profiles: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    startup.ensure_structured_type("MeasurementChart", crate::measurement_chart_type())?;
    startup.insert(conduit_plot::KindSignature {
        kind: MEASUREMENT_CHART_FLOW_FACE_KIND.into(),
        startup_parameters: vec![
            conduit_plot::StartupParameterSignature {
                name: "label".into(),
                value_type: "Text".into(),
                default: None,
            },
            conduit_plot::StartupParameterSignature {
                name: "maximum-updates".into(),
                value_type: "Count".into(),
                default: Some("1024".into()),
            },
        ],
    })?;
    profiles
        .insert_kind(measurement_chart_flow_semantic_contract())
        .map_err(|e| alloc::format!("{e}"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_terminal_route_preserves_original_outputless_kind() {
        let old = crate::measurement_chart_face_semantic_contract();
        assert!(old.outputs.is_empty());
        assert!(old.inputs[0].abnormal_kind.is_none());
        let new = measurement_chart_flow_semantic_contract();
        new.validate().unwrap();
        assert_eq!(new.outputs[0].port_id.as_str(), "done");
        assert_eq!(new.outputs[0].value_kind.as_str(), EMPTY_INFO_ID);
        assert_eq!(new.inputs[0].abnormal_kind, new.outputs[0].abnormal_kind);
        assert_eq!(new.configuration, old.configuration);
        assert_eq!(new.terminal_transductions().count(), 1);
    }
}
