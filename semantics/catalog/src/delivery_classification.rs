//! Explicit reviewed delivery/evolution metadata for portable Info.
//!
//! This table is keyed by exact Info contract identity. It never infers
//! behavior from a Rust type name, semantic spelling, or Host realization.

use conduit_core::{
    AdmissionUnit, DeliveryContract, DeliveryPressurePolicy, EvolutionSemantics, KindId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryClassification {
    pub info_kind: &'static str,
    pub contract: DeliveryContract,
    pub reasoning: &'static str,
}

const fn ordered(evolution: EvolutionSemantics, unit: AdmissionUnit) -> DeliveryContract {
    DeliveryContract::new(evolution, unit, DeliveryPressurePolicy::PreserveOrder)
}

const fn latest(evolution: EvolutionSemantics, unit: AdmissionUnit) -> DeliveryContract {
    DeliveryContract::new(evolution, unit, DeliveryPressurePolicy::CoalesceLatest)
}

pub const REVIEWED_DELIVERY_CLASSIFICATIONS: &[DeliveryClassification] = &[
    classification(
        conduit_human::KEY_EVENT_INFO_ID,
        conduit_human::KEY_EVENT_DELIVERY_CONTRACT,
        "each press and release is an ordered transition",
    ),
    classification(
        "input/button-transition@1",
        ordered(EvolutionSemantics::Occurrence, AdmissionUnit::Value),
        "each identified button phase and sequence is an occurrence",
    ),
    classification(
        "input/axis-state@1",
        latest(EvolutionSemantics::CurrentState, AdmissionUnit::Value),
        "a newer normalized axis state may supersede queued older state",
    ),
    classification(
        crate::POINTER_EVENT_INFO_ID,
        latest(
            EvolutionSemantics::CurrentState,
            AdmissionUnit::CoherentFrame,
        ),
        "buttons, position, delta, pressure, and sequence form one atomic snapshot",
    ),
    classification(
        "input/touch-frame@1",
        latest(
            EvolutionSemantics::CurrentState,
            AdmissionUnit::CoherentFrame,
        ),
        "contacts from unrelated source frames must never be combined",
    ),
    classification(
        "input/gamepad-state@1",
        latest(
            EvolutionSemantics::CurrentState,
            AdmissionUnit::CoherentFrame,
        ),
        "axes and buttons are one coherent current controller snapshot",
    ),
    classification(
        "input/rotary-step@1",
        ordered(EvolutionSemantics::Occurrence, AdmissionUnit::Value),
        "each directional step occurrence contributes to the resulting count",
    ),
    classification(
        "robotics/contact-event@1",
        ordered(EvolutionSemantics::Observation, AdmissionUnit::Value),
        "began and ended observations retain source sequence and time",
    ),
    classification(
        "robotics/pose-sample@1",
        ordered(EvolutionSemantics::Observation, AdmissionUnit::Value),
        "pose is a source-correlated observation with uncertainty",
    ),
    classification(
        "robotics/range-observation@1",
        ordered(EvolutionSemantics::Observation, AdmissionUnit::Value),
        "range retains exact source sample context",
    ),
    classification(
        "robotics/power-telemetry@1",
        ordered(EvolutionSemantics::Observation, AdmissionUnit::Value),
        "power telemetry retains exact source sample context",
    ),
    classification(
        "data/sampled-signal@1",
        ordered(
            EvolutionSemantics::SampledSignal,
            AdmissionUnit::SignalBatch,
        ),
        "the existing finite batch owns cadence, continuity, and gap semantics",
    ),
    classification(
        "robotics/motion-request@1",
        ordered(EvolutionSemantics::RequestIntent, AdmissionUnit::Value),
        "request identity and expiry prohibit implicit latest-wins replacement",
    ),
];

const fn classification(
    info_kind: &'static str,
    contract: DeliveryContract,
    reasoning: &'static str,
) -> DeliveryClassification {
    DeliveryClassification {
        info_kind,
        contract,
        reasoning,
    }
}

pub fn reviewed_delivery_contract(info_kind: &KindId) -> Option<DeliveryContract> {
    REVIEWED_DELIVERY_CLASSIFICATIONS
        .iter()
        .find(|entry| entry.info_kind == info_kind.as_str())
        .map(|entry| entry.contract)
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeSet;

    use super::*;

    #[test]
    fn every_initial_family_is_explicit_unique_versioned_and_valid() {
        let identities: BTreeSet<_> = REVIEWED_DELIVERY_CLASSIFICATIONS
            .iter()
            .map(|entry| entry.info_kind)
            .collect();
        assert_eq!(identities.len(), REVIEWED_DELIVERY_CLASSIFICATIONS.len());
        for required in [
            conduit_human::KEY_EVENT_INFO_ID,
            "input/button-transition@1",
            "input/axis-state@1",
            crate::POINTER_EVENT_INFO_ID,
            "input/touch-frame@1",
            "input/gamepad-state@1",
            "input/rotary-step@1",
            "robotics/contact-event@1",
            "robotics/pose-sample@1",
            "robotics/range-observation@1",
            "robotics/power-telemetry@1",
            "data/sampled-signal@1",
            "robotics/motion-request@1",
        ] {
            let entry = REVIEWED_DELIVERY_CLASSIFICATIONS
                .iter()
                .find(|entry| entry.info_kind == required)
                .unwrap();
            assert!(entry.contract.validate().is_ok());
            assert!(!entry.reasoning.is_empty());
        }
    }

    #[test]
    fn lookup_requires_exact_registered_identity_and_never_guesses_from_a_name() {
        assert!(reviewed_delivery_contract(&KindId::from("input/axis-state@1")).is_some());
        assert!(reviewed_delivery_contract(&KindId::from("input/future-state@1")).is_none());
        assert!(reviewed_delivery_contract(&KindId::from("State")).is_none());
        assert!(reviewed_delivery_contract(&KindId::from("browser/pointer@1")).is_none());
    }
}
