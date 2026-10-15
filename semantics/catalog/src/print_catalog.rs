//! Portable print submission meaning. Actual provider bindings remain Host-owned.
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::*;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};
use conduit_process::{PrintLifecycle, PrintRequest, PRINT_MAXIMUM_WIRE_BYTES};

pub const PRINT_SUBMIT_KIND: &str = "print/submit";
pub const PRINT_SUBMIT_REVISION: &str = "conduit.process/print-submit@1";
pub const PRINT_SUBMIT_HOST_CALL: &str = "conduit.host/print-submit@1";

pub fn print_submit_semantic_contract() -> Kind {
    fn port(name: &str, value: StructuredInfoType, direction: PortDirection) -> PortDescriptor {
        PortDescriptor {
            port_id: port_id(name),
            value_kind: value
                .profile()
                .expect("checked print Type")
                .value_kind()
                .clone(),
            direction,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }
    }
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(PRINT_SUBMIT_KIND),
        kind_contract_revision: PRINT_SUBMIT_REVISION.into(),
        inputs: vec![port(
            "request",
            PrintRequest::semantic_type().expect("checked print request"),
            PortDirection::Input,
        )],
        outputs: vec![port(
            "lifecycle",
            PrintLifecycle::semantic_type().expect("checked print lifecycle"),
            PortDirection::Output,
        )],
        configuration: vec![],
        semantic_laws: vec![
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable),
            KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained),
            KindSemanticLaw::TimeDependence(SemanticDependence::None),
            KindSemanticLaw::RandomDependence(SemanticDependence::None),
            KindSemanticLaw::ResourceDependence(SemanticDependence::ExplicitInput),
            KindSemanticLaw::Suspension(SuspensionBehavior::MaySuspend),
            KindSemanticLaw::Variability(VariabilityBehavior::AdmittedVariability),
            KindSemanticLaw::Replay(ReplayBehavior::Ineligible),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: PRINT_MAXIMUM_WIRE_BYTES as u32,
        },
    }
}

/// Installs data and Fore contracts, never an implicit print provider or URI.
pub fn install_print_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for (name, ty) in [
        ("PrintRequest", PrintRequest::semantic_type()),
        ("PrintLifecycle", PrintLifecycle::semantic_type()),
    ] {
        startup
            .insert_structured_type(name, ty.map_err(|e| alloc::format!("{e:?}"))?)
            .map_err(|e| e.to_string())?;
    }
    let contract = print_submit_semantic_contract();
    startup
        .insert(KindSignature {
            kind: PRINT_SUBMIT_KIND.into(),
            startup_parameters: vec![],
        })
        .map_err(|e| e.to_string())?;
    startup
        .insert_fore(PRINT_SUBMIT_KIND, contract.checked_front())
        .map_err(|e| e.to_string())?;
    profile.insert_kind(contract).map_err(|e| e.to_string())
}

pub fn print_kind_contracts() -> Vec<Kind> {
    vec![print_submit_semantic_contract()]
}
