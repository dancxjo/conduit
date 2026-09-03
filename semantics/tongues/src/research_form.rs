//! Portable Form contracts for the bounded paired-latent run.

use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindDefinition,
    KindSignature, ProfileCatalog, StartupCatalog,
};

pub const TRAINING_FORM_SOURCE: &str =
    include_str!("../../../examples/tongues-latent-training.conduit");
pub const INFERENCE_FORM_SOURCE: &str =
    include_str!("../../../examples/tongues-bidirectional-inference.conduit");

pub fn check_research_forms() -> Result<[conduit_form::ExpandedCanonicalForm; 2], String> {
    let (startup, profile) = research_catalogs()?;
    let training = check_one(
        TRAINING_FORM_SOURCE,
        "tongues-latent-training",
        &startup,
        &profile,
    )?;
    let inference = check_one(
        INFERENCE_FORM_SOURCE,
        "tongues-bidirectional-inference",
        &startup,
        &profile,
    )?;
    Ok([training, inference])
}

fn check_one(
    source: &str,
    form: &str,
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
) -> Result<conduit_form::ExpandedCanonicalForm, String> {
    let syntax = parse_syntax_document(source);
    let checked = check_syntax_document(&syntax, startup).map_err(|error| format!("{error:?}"))?;
    expand_canonical_form(&checked, form, profile).map_err(|error| error.to_string())
}

fn research_catalogs() -> Result<(StartupCatalog, ProfileCatalog), String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    for definition in definitions() {
        startup.insert(KindSignature {
            kind: definition.kind_id.as_str().into(),
            startup_parameters: vec![],
        })?;
        profile
            .insert(definition)
            .map_err(|error| error.to_string())?;
    }
    Ok((startup, profile))
}

fn definitions() -> Vec<KindDefinition> {
    vec![
        kind(
            "tongues/paired-observations",
            vec![],
            vec![
                output("acoustic", "tongues/acoustic-trajectory@1"),
                output("articulation", "tongues/articulatory-trajectory@1"),
            ],
        ),
        kind(
            "tongues/train-shared-latent",
            vec![
                input("acoustic", "tongues/acoustic-trajectory@1"),
                input("articulation", "tongues/articulatory-trajectory@1"),
            ],
            vec![
                output("checkpoint", "model/checkpoint@1"),
                output("metrics", "model/training-metrics@1"),
            ],
        ),
        kind(
            "tongues/bidirectional-latent",
            vec![
                input("acoustic", "tongues/acoustic-trajectory@1"),
                input("articulation", "tongues/articulatory-trajectory@1"),
                input("checkpoint", "model/checkpoint@1"),
            ],
            vec![
                output("latent", "tongues/latent-trajectory@1"),
                output("generated-acoustic", "tongues/acoustic-trajectory@1"),
                output("inferred-articulation", "tongues/articulation-posterior@1"),
            ],
        ),
        kind(
            "tongues/evaluate-latent",
            vec![
                input("latent", "tongues/latent-trajectory@1"),
                input("generated-acoustic", "tongues/acoustic-trajectory@1"),
                input("inferred-articulation", "tongues/articulation-posterior@1"),
            ],
            vec![output("report", "tongues/evaluation-report@1")],
        ),
    ]
}

fn kind(
    identity: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(identity),
        kind_contract_revision: KindContractRevision::from(format!("conduit.{identity}@1")),
        inputs,
        outputs,
        configuration: vec![],
    }
}

fn input(identity: &str, value_kind: &str) -> PortDescriptor {
    port(identity, value_kind, PortDirection::Input)
}

fn output(identity: &str, value_kind: &str) -> PortDescriptor {
    port(identity, value_kind, PortDirection::Output)
}

fn port(identity: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(identity),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}
