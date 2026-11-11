//! Ordinary finite temporal retrieval from explicit windows and current intent.
use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ExternalEffectBehavior, Kind, KindSemanticLaw,
    PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
};
use conduit_plot::KindSignature;
mod codec;
pub use codec::*;
pub const TEMPORAL_EVIDENCE_FLOW_KIND: &str = "retrieval/temporal-select-flow";
pub const TEMPORAL_EVIDENCE_FLOW_REVISION: &str = "conduit.retrieval/temporal-select-flow@1";
pub const TEMPORAL_EVIDENCE_FLOW_MAX_BYTES: usize = 65536;
pub fn temporal_evidence_window_type() -> StructuredInfoType {
    crate::TemporalEvidenceWindow::semantic_type().expect("native temporal window")
}
pub fn temporal_evidence_outcome_type() -> StructuredInfoType {
    crate::TemporalEvidenceOutcome::semantic_type().expect("native temporal outcome")
}
pub fn temporal_evidence_flow_contract() -> Kind {
    let port = |name, ty: StructuredInfoType, direction, temporal| PortDescriptor {
        port_id: port_id(name),
        value_kind: ty
            .profile()
            .expect("native temporal Flow profile")
            .value_kind()
            .clone(),
        direction,
        temporal,
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(TEMPORAL_EVIDENCE_FLOW_KIND),
        kind_contract_revision: TEMPORAL_EVIDENCE_FLOW_REVISION.into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![
            port(
                "window",
                temporal_evidence_window_type(),
                PortDirection::Input,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "intent",
                crate::TemporalRetrievalIntent::semantic_type().expect("native retrieval intent"),
                PortDirection::Input,
                PortTemporal::Current,
            ),
        ],
        outputs: vec![port(
            "outcome",
            temporal_evidence_outcome_type(),
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
        )],
        configuration: vec![],
        semantic_laws: vec![KindSemanticLaw::ExternalEffects(
            ExternalEffectBehavior::None,
        )],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: TEMPORAL_EVIDENCE_FLOW_MAX_BYTES as u32,
        },
    }
}
pub fn install_temporal_evidence_flow_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for (name, ty) in [
        ("TemporalEvidenceWindow", temporal_evidence_window_type()),
        ("TemporalEvidenceOutcome", temporal_evidence_outcome_type()),
        (
            "TemporalRetrievalIntent",
            crate::TemporalRetrievalIntent::semantic_type().expect("native retrieval intent"),
        ),
    ] {
        startup
            .ensure_structured_type(name, ty)
            .map_err(|e| e.to_string())?;
    }
    let kind = temporal_evidence_flow_contract();
    startup
        .insert(KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|e| e.to_string())?;
    profile.insert_kind(kind).map_err(|e| e.to_string())
}
