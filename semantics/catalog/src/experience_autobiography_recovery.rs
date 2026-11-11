//! Explicit recovery of provider loss, preserving autobiography without model fiction.
use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::*;
use conduit_plot::KindSignature;
pub const EXPERIENCE_AUTOBIOGRAPHY_RECOVERY_KIND: &str =
    "experience/autobiography-retain-recoverable";
pub const EXPERIENCE_AUTOBIOGRAPHY_RECOVERY_REVISION: &str =
    "conduit.experience/autobiography-retain-recoverable@1";
pub fn experience_autobiography_recovery_contract() -> Kind {
    let mut kind = crate::experience_autobiography_retain_contract();
    kind.kind_id = kind_id(EXPERIENCE_AUTOBIOGRAPHY_RECOVERY_KIND);
    kind.kind_contract_revision = EXPERIENCE_AUTOBIOGRAPHY_RECOVERY_REVISION.into();
    kind.inputs.push(PortDescriptor {
        port_id: port_id("terminal"),
        value_kind: kind_id(TERMINAL_INFO_ID),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: Some(kind_id(TERMINAL_INFO_ID)),
    });
    kind.outputs.push(PortDescriptor {
        port_id: port_id("loss"),
        value_kind: kind_id(TERMINAL_INFO_ID),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    });
    // Successful model envelopes leave through this recovered outcome boundary.
    // Its abnormal outcome is exposed separately as loss, never hidden on a Fore.
    kind.outputs.push(PortDescriptor {
        port_id: port_id("response"),
        value_kind: kind_id("llm/generated-result@1"),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    });
    // Ordinary loss payload uses Core TerminalInfo's intrinsic exact codec.
    // Projection ingress additionally carries its matching source contract.
    kind.semantic_laws.push(KindSemanticLaw::ValueContracts(
        [
            FrontValueLocation::Input(port_id("terminal")),
            FrontValueLocation::InputAbnormal(port_id("terminal")),
        ]
        .into_iter()
        .map(|location| FrontValueContract {
            location,
            contract: CheckedValueContract::new(
                kind_id(TERMINAL_INFO_ID),
                TERMINAL_INFO_ENCODED_LEN as u32,
                vec![],
            )
            .expect("fixed terminal envelope"),
        })
        .collect(),
    ));
    for (name, abnormal) in [
        ("pending", AbnormalTerminalTransduction::NotAccepted),
        ("result", AbnormalTerminalTransduction::NotAccepted),
        ("terminal", AbnormalTerminalTransduction::Recover),
    ] {
        kind.semantic_laws
            .push(KindSemanticLaw::TerminalTransduction(
                TerminalTransductionProfile {
                    input_port_id: port_id(name),
                    output_port_id: port_id("next"),
                    normal_close: if name == "terminal" {
                        NormalCloseTransduction::NotAccepted
                    } else {
                        NormalCloseTransduction::Consume
                    },
                    abnormal,
                    cancellation: CancellationTransduction::NotCancellable,
                },
            ));
    }
    kind
}
pub fn install_experience_autobiography_recovery_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for (name, ty) in [
        (
            "ExperienceAutobiographyPending",
            crate::experience_autobiography_pending_type(),
        ),
        (
            "ExperienceAutobiographyState",
            crate::experience_autobiography_state_type(),
        ),
    ] {
        startup
            .ensure_structured_type(name, ty)
            .map_err(|error| error.to_string())?;
    }
    let kind = experience_autobiography_recovery_contract();
    startup.insert(KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: vec![],
    })?;
    profile.insert_kind(kind).map_err(|error| error.to_string())
}
