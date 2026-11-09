//! Generic finite append equipment; no observation, catalog or delivery semantics.
use alloc::{vec, vec::Vec};
use conduit_core::*;
pub const RESOURCE_APPEND_KIND: &str = "resource/try-append-record";
pub const RESOURCE_APPEND_REVISION: &str = "conduit.resource/try-append-record@1";
/// Opaque existing Bytes, bounded to 200, and a canonical declared typed result.
/// A recorded output denotes verified media commitment only; capacity and
/// uncertainty remain ordinary values. Input closure drains
/// completed indices then closes; cancellation cannot retract committed media.
pub fn resource_append_semantic_contract() -> Kind {
    let input = PortDescriptor {
        port_id: port_id("record"),
        value_kind: kind_id(BYTES_INFO_ID),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    let output = PortDescriptor {
        port_id: port_id("index"),
        value_kind: kind_id(conduit_resource_snapshot::append_result::APPEND_RESULT_PROFILE),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(RESOURCE_APPEND_KIND),
        kind_contract_revision: RESOURCE_APPEND_REVISION.into(),
        startup_parameters: Vec::new(),
        shorthand: None,
        configuration: Vec::new(),
        inputs: vec![input.clone()],
        outputs: vec![output.clone()],
        semantic_laws: vec![
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Input(input.port_id.clone()),
                    contract: CheckedValueContract::new(input.value_kind, 200, vec![])
                        .expect("finite Bytes"),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(output.port_id.clone()),
                    contract: CheckedValueContract::new(
                        output.value_kind,
                        conduit_resource_snapshot::append_result::APPEND_RESULT_BYTES,
                        vec![],
                    )
                    .expect("finite canonical outcome"),
                },
            ]),
            KindSemanticLaw::TerminalTransduction(TerminalTransductionProfile {
                input_port_id: input.port_id,
                output_port_id: output.port_id,
                normal_close: NormalCloseTransduction::PropagateAfterDrain,
                abnormal: AbnormalTerminalTransduction::NotAccepted,
                cancellation: CancellationTransduction::NotCancellable,
            }),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 200 + conduit_resource_snapshot::append_result::APPEND_RESULT_BYTES,
        },
    }
}
#[cfg(feature = "plot-catalog")]
pub fn install_resource_append_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind = resource_append_semantic_contract();
    startup.insert_structured_type(
        "resource/try-append-record/result",
        conduit_resource_snapshot::append_result::result_type()
            .map_err(|e| alloc::format!("{e:?}"))?,
    )?;
    startup.insert(conduit_plot::KindSignature {
        kind: RESOURCE_APPEND_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    startup.insert_fore(RESOURCE_APPEND_KIND, kind.checked_front())?;
    profile
        .insert_kind(kind)
        .map_err(|e| alloc::format!("{e:?}"))
}
