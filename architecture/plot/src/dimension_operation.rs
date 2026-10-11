//! Ordinary checked physical introspection: typed startup input, Dimension port
//! output. Preparation reads the sealed value and never reparses unit spelling.
use crate::{prelude::*, KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature};
use conduit_core::*;

pub const UNIT_KIND: &str = "units/dimension";
pub const QUANTITY_KIND: &str = "units/quantity-dimension";

pub fn contract(kind: &str) -> Option<Kind> {
    let (source_kind, rule, default) = match kind {
        UNIT_KIND => (
            UNIT_INFO_ID,
            KindConfigurationRule::Unit,
            ConfigurationValue::Unit(
                UnitConfigurationValue::parse("m").expect("source-defined Unit"),
            ),
        ),
        QUANTITY_KIND => (
            QUANTITY_INFO_ID,
            KindConfigurationRule::Quantity,
            ConfigurationValue::Quantity(
                QuantityConfigurationValue::parse("0m").expect("source-defined Quantity"),
            ),
        ),
        _ => return None,
    };
    Some(Kind {
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from("physical/dimension-introspection@1"),
        startup_parameters: vec![FrontStartupParameter {
            name: "source".into(),
            value_type: kind_id(source_kind),
            has_default: false,
        }],
        shorthand: None,
        inputs: vec![],
        outputs: vec![PortDescriptor {
            port_id: port_id("dimension"),
            value_kind: kind_id(DIMENSION_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        configuration: vec![KindConfigurationField {
            key: "source".into(),
            default_value: default,
            rule,
        }],
        semantic_laws: vec![
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
            KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
            KindSemanticLaw::TimeDependence(SemanticDependence::None),
            KindSemanticLaw::RandomDependence(SemanticDependence::None),
            KindSemanticLaw::ResourceDependence(SemanticDependence::None),
            KindSemanticLaw::Suspension(SuspensionBehavior::Never),
            KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
            KindSemanticLaw::Replay(ReplayBehavior::Exact),
        ],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: DIMENSION_ENCODED_LEN as u32,
        },
    })
}

pub fn install(startup: &mut StartupCatalog, profile: &mut ProfileCatalog) -> Result<(), String> {
    for (kind, source_type) in [(UNIT_KIND, "Unit"), (QUANTITY_KIND, "Quantity")] {
        let contract = contract(kind).expect("reviewed introspection Kind");
        startup.insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "source".into(),
                value_type: source_type.into(),
                default: None,
            }],
        })?;
        startup.insert_fore(kind, contract.checked_front())?;
        profile
            .insert_kind(contract)
            .map_err(|reason| format!("{reason}"))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimensionPreparationRefusal {
    UnknownKind,
    Configuration,
    WrongSourceType,
}

pub fn prepare(
    kind: &str,
    configuration: &[ConfigurationEntry],
) -> Result<Dimension, DimensionPreparationRefusal> {
    use DimensionPreparationRefusal as R;
    if contract(kind).is_none() {
        return Err(R::UnknownKind);
    }
    let [source] = configuration else {
        return Err(R::Configuration);
    };
    if source.key != "source" {
        return Err(R::Configuration);
    }
    let definition = match (kind, &source.value) {
        (UNIT_KIND, ConfigurationValue::Unit(value)) => value.value().dimension(),
        (QUANTITY_KIND, ConfigurationValue::Quantity(value)) => value.value().dimension(),
        _ => return Err(R::WrongSourceType),
    };
    Ok(Dimension::from_definition(definition))
}
