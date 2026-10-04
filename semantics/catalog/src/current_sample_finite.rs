//! Finite sampling for payloads which carry their own typed refusal/loss values.
#[cfg(feature = "plot-catalog")]
use alloc::{string::ToString, vec::Vec};
use conduit_core::*;

pub const CURRENT_SAMPLE_FINITE_KIND: &str = "current/sample/finite";
pub const CURRENT_SAMPLE_FINITE_REVISION: &str = "conduit.current/sample-finite@1";

/// Each trigger samples committed current state. Normal trigger closure drains
/// prior outputs and closes the output; closure does not invent a final sample.
/// No abnormal lane is offered: protocol refusal/loss remains payload meaning.
pub fn current_sample_finite_semantic_contract(
    value: &CheckedValueContract,
    trigger: &CheckedValueContract,
) -> Result<Kind, &'static str> {
    let mut kind = super::current_sample_semantic_contract(value, trigger)?;
    kind.kind_id = kind_id(CURRENT_SAMPLE_FINITE_KIND);
    kind.kind_contract_revision = KindIdentity::from(CURRENT_SAMPLE_FINITE_REVISION);
    kind.inputs[1].temporal = PortTemporal::Flow { closes: true };
    kind.inputs[1].abnormal_kind = None;
    kind.outputs[0].temporal = PortTemporal::Flow { closes: true };
    kind.outputs[0].abnormal_kind = None;
    for law in &mut kind.semantic_laws {
        match law {
            KindSemanticLaw::ValueContracts(contracts) => contracts.retain(|entry| {
                matches!(
                    entry.location,
                    FrontValueLocation::Input(_) | FrontValueLocation::Output(_)
                )
            }),
            KindSemanticLaw::TerminalTransduction(profile) => {
                profile.normal_close = NormalCloseTransduction::PropagateAfterDrain;
                profile.abnormal = AbnormalTerminalTransduction::NotAccepted;
            }
            _ => {}
        }
    }
    kind.limits.max_queue_bytes = value
        .maximum_bytes
        .checked_add(trigger.maximum_bytes)
        .ok_or("finite sampler queue envelope overflows")?
        .max(1);
    Ok(kind)
}

#[cfg(feature = "plot-catalog")]
pub fn install_current_sample_finite_kind(
    value: &CheckedValueContract,
    trigger: &CheckedValueContract,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind = current_sample_finite_semantic_contract(value, trigger).map_err(str::to_string)?;
    startup.insert(conduit_plot::KindSignature {
        kind: CURRENT_SAMPLE_FINITE_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    startup.insert_fore(CURRENT_SAMPLE_FINITE_KIND, kind.checked_front())?;
    profile
        .insert_kind(kind)
        .map_err(|error| alloc::format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_sampling_has_exact_payload_bounds_and_a_close_without_extra_output() {
        let value = CheckedValueContract::new(kind_id("value/bytes"), 4096, alloc::vec![]).unwrap();
        let trigger = CheckedValueContract::new(kind_id("value/u64"), 8, alloc::vec![]).unwrap();
        let kind = current_sample_finite_semantic_contract(&value, &trigger).unwrap();
        kind.validate().unwrap();
        assert_eq!(kind.inputs[0].temporal, PortTemporal::Current);
        assert_eq!(kind.inputs[1].temporal, PortTemporal::Flow { closes: true });
        assert_eq!(
            kind.outputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert!(kind
            .inputs
            .iter()
            .chain(&kind.outputs)
            .all(|port| port.abnormal_kind.is_none()));
        let terminal = kind.terminal_transductions().next().unwrap();
        assert_eq!(
            terminal.normal_close,
            NormalCloseTransduction::PropagateAfterDrain
        );
        assert_eq!(terminal.abnormal, AbnormalTerminalTransduction::NotAccepted);
        assert_eq!(kind.limits.max_queue_bytes, 4104);
        assert_eq!(
            super::super::current_sample_semantic_contract(&value, &trigger)
                .unwrap()
                .inputs[1]
                .temporal,
            PortTemporal::Flow { closes: false }
        );
    }
}
