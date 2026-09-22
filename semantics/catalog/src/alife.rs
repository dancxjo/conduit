//! Portable bounded Lenia meanings.

use super::{
    KindConfigurationField, KindConfigurationRule, KindTerminalBehavior, StandardKindContract,
};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_alife::{LENIA_MAXIMUM_FIELD_BYTES, MAXIMUM_PRESENTED_FIELDS};
use conduit_core::{CapabilityLimits, Kind};

pub fn alife_contracts() -> Vec<StandardKindContract> {
    vec![
        orbium_seed_contract(),
        lenia_step_contract(),
        scalar_field_presentation_contract(),
    ]
}

pub fn orbium_seed_contract() -> StandardKindContract {
    let definition = conduit_alife::orbium_seed_definition();
    StandardKindContract {
        kind_id: definition.kind_id,
        plain_name: "Deterministic Orbium seed".to_string(),
        summary: "Construct one bounded portable ScalarField2 specimen from semantic dimensions and seed.".to_string(),
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: standard_configuration(definition.configuration),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: LENIA_MAXIMUM_FIELD_BYTES * 4,
        },
        terminal_behavior: KindTerminalBehavior::EmitsOneField,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "seed: alife/orbium-seed(width = 128, height = 128, seed = 1)".to_string(),
    }
}

pub fn lenia_step_contract() -> StandardKindContract {
    let definition = conduit_alife::lenia_step_definition();
    StandardKindContract {
        kind_id: definition.kind_id,
        plain_name: "Lenia field evolution".to_string(),
        summary: "Evolve an initialized ScalarField2 once per closing-flow Tick using exact fixed-Q16.16 Lenia semantics.".to_string(),
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: standard_configuration(definition.configuration),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_PRESENTED_FIELDS + 1,
            max_queue_bytes: LENIA_MAXIMUM_FIELD_BYTES + 64,
        },
        terminal_behavior: KindTerminalBehavior::EvolvesAfterTicksAndCompletesWhenTickCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "evolve: alife/lenia-step(kernel_radius = 13, growth_mu = 0.15)".to_string(),
    }
}

pub fn scalar_field_presentation_contract() -> StandardKindContract {
    let definition = conduit_alife::scalar_field_presentation_definition();
    StandardKindContract {
        kind_id: definition.kind_id,
        plain_name: "Scalar field presentation".to_string(),
        summary:
            "Manifest each bounded ScalarField2 through one exact admitted presentation effect."
                .to_string(),
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: standard_configuration(definition.configuration),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_PRESENTED_FIELDS,
            max_queue_bytes: LENIA_MAXIMUM_FIELD_BYTES * u32::from(MAXIMUM_PRESENTED_FIELDS),
        },
        terminal_behavior: KindTerminalBehavior::PresentsEachFieldAndCompletesWhenInputCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "show: presentation/scalar-field(title = \"Orbium\", minimum = 0, maximum = 1)"
            .to_string(),
    }
}

pub fn orbium_seed_semantic_contract() -> Kind {
    semantic_contract(orbium_seed_contract(), conduit_alife::ORBIUM_SEED_REVISION)
}

pub fn lenia_step_semantic_contract() -> Kind {
    semantic_contract(lenia_step_contract(), conduit_alife::LENIA_STEP_REVISION)
}

pub fn scalar_field_presentation_semantic_contract() -> Kind {
    semantic_contract(
        scalar_field_presentation_contract(),
        conduit_alife::SCALAR_FIELD_PRESENTATION_REVISION,
    )
}

fn semantic_contract(contract: StandardKindContract, revision: &str) -> Kind {
    Kind {
        startup_parameters: super::startup_front(&contract.configuration),
        shorthand: None,
        kind_id: contract.kind_id,
        kind_contract_revision: revision.into(),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: contract.configuration,
        semantic_laws: alloc::vec![conduit_core::KindSemanticLaw::Terminal(
            contract.terminal_behavior
        )],
        limits: contract.limits,
    }
}

fn standard_configuration(
    fields: Vec<conduit_form::KindConfigurationField>,
) -> Vec<KindConfigurationField> {
    fields
        .into_iter()
        .map(|field| KindConfigurationField {
            key: field.key,
            default_value: field.default_value,
            rule: match field.rule {
                conduit_form::KindConfigurationRule::Any => KindConfigurationRule::Any,
                conduit_form::KindConfigurationRule::U64Range { minimum, maximum } => {
                    KindConfigurationRule::U64Range { minimum, maximum }
                }
                conduit_form::KindConfigurationRule::I64Range { minimum, maximum } => {
                    KindConfigurationRule::I64Range { minimum, maximum }
                }
                conduit_form::KindConfigurationRule::DurationMillis { minimum, maximum } => {
                    KindConfigurationRule::DurationMillis { minimum, maximum }
                }
                conduit_form::KindConfigurationRule::QuantityRange {
                    minimum,
                    maximum,
                    canonical_unit,
                } => KindConfigurationRule::QuantityRange {
                    minimum,
                    maximum,
                    canonical_unit,
                },
                conduit_form::KindConfigurationRule::TextBytes { maximum } => {
                    KindConfigurationRule::TextBytes { maximum }
                }
                conduit_form::KindConfigurationRule::TextOneOf { values } => {
                    KindConfigurationRule::TextOneOf { values }
                }
                conduit_form::KindConfigurationRule::Structured { .. } => {
                    unreachable!("Lenia definitions do not use structured configuration")
                }
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contracts_are_exact_finite_and_platform_neutral() {
        let contracts = alife_contracts();
        assert_eq!(contracts.len(), 3);
        assert!(contracts
            .iter()
            .all(|contract| !contract.inputs.is_empty() || !contract.outputs.is_empty()));
        let portable = alloc::format!("{contracts:?}").to_ascii_lowercase();
        for forbidden in ["host/", "boot/", "websocket", "framebuffer", "dom", "gpio"] {
            assert!(!portable.contains(forbidden), "leaked {forbidden}");
        }
    }
}
