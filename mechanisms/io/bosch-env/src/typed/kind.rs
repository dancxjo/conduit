//! Semantic front only. Physical identity and realization belong to its owner.
use super::*;
use alloc::vec;
use conduit_core as c;

pub const KIND: &str = "device/bme280-compensation";
pub const KIND_REVISION: &str = "device/bme280-compensation@1";

impl ObservationContract {
    /// Describe the reviewed Source operation without manufacturing a provider,
    /// capability, resource reservation or execution authority. The whole Front
    /// matches the original Plot, so it can remain a checked Source Back for the
    /// same Kind. Callers must separately select and admit a real realization.
    pub fn kind(&self) -> Result<c::Kind, StructuredInfoRefusal> {
        let port = |name: &str, profile: &str, direction, closes| c::PortDescriptor {
            port_id: c::port_id(name),
            value_kind: profile.into(),
            direction,
            temporal: c::PortTemporal::Flow { closes },
            abnormal_kind: None,
        };
        let kind = c::Kind {
            kind_id: KIND.into(),
            kind_contract_revision: KIND_REVISION.into(),
            startup_parameters: vec![],
            shorthand: SHORTHAND.map(|(input, result)| (input.into(), result.into())),
            inputs: vec![port(
                "input",
                INPUT_PROFILE,
                c::PortDirection::Input,
                INPUT_CLOSES,
            )],
            outputs: vec![port(
                "result",
                RESULT_PROFILE,
                c::PortDirection::Output,
                RESULT_CLOSES,
            )],
            configuration: vec![],
            // Preserve the exact reviewed Front: no additional semantic law or
            // effect authority is inferred from native implementation support.
            semantic_laws: vec![],
            limits: c::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: u32::try_from(INPUT_MAXIMUM_BYTES.max(RESULT_MAXIMUM_BYTES))
                    .map_err(|_| StructuredInfoRefusal::CanonicalEncodingTooLarge)?,
            },
        };
        kind.validate()
            .map_err(|_| StructuredInfoRefusal::WrongType)?;
        Ok(kind)
    }
}
