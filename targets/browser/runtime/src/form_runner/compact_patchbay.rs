//! Bounded read-only Patchbay facts for one executable Tour listing.
//!
//! This projection is produced from the same production parse, check, and
//! expansion path used before Tour execution. It contains no renderer geometry,
//! Host offer, placement, implementation, Plan, Play, or mutable editor state.

use crate::installed_browser::{backs, catalogs, MAXIMUM_BROWSER_CORDS, MAXIMUM_BROWSER_GEARS};
use conduit_form::ExpandedCanonicalForm;
use serde::Serialize;

const MAXIMUM_COMPACT_PORTS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompactPatchbayProjection {
    pub(super) schema: &'static str,
    pub(super) sequence: u64,
    pub(super) source_proposal_id: String,
    pub(super) source_document_id: String,
    pub(super) checked_form_id: String,
    pub(super) visible_expanded_form_id: String,
    pub(super) realization_expanded_form_id: String,
    pub(super) form_name: String,
    pub(super) realization: &'static str,
    pub(super) gears: Vec<CompactGear>,
    pub(super) cords: Vec<CompactCord>,
    pub(super) realization_gears: Vec<CompactGear>,
    pub(super) realization_cords: Vec<CompactCord>,
    pub(super) realization_backs: Vec<CompactBack>,
    pub(super) diagnostics: Vec<CompactDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompactGear {
    pub(super) gear_id: String,
    pub(super) kind_id: String,
    pub(super) inputs: Vec<CompactPort>,
    pub(super) outputs: Vec<CompactPort>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompactPort {
    pub(super) port_id: String,
    pub(super) info_kind: String,
    pub(super) temporal: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompactCord {
    pub(super) source_gear_id: String,
    pub(super) source_port_id: String,
    pub(super) sink_gear_id: String,
    pub(super) sink_port_id: String,
    pub(super) info_kind: String,
    pub(super) temporal: &'static str,
    pub(super) invalid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompactDiagnostic {
    pub(super) code: &'static str,
    pub(super) message: String,
    pub(super) fix: String,
    pub(super) subjects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompactBack {
    pub(super) invocation_path: String,
    pub(super) kind_id: String,
    pub(super) checked_form_id: String,
}

pub(super) fn project(
    source: &str,
    sequence: u64,
    recursive: bool,
) -> Result<CompactPatchbayProjection, String> {
    let interaction = crate::source_interaction::admit_source(source.as_bytes(), sequence)?;
    let (startup, catalog) = catalogs()?;
    let syntax = conduit_form::parse_syntax_document(source);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(format!(
            "parse compact Tour Patchbay: {}",
            diagnostic.message
        ));
    }
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("check compact Tour Patchbay: {error:?}"))?;
    let entry = checked
        .forms
        .last()
        .ok_or_else(|| "compact Tour Patchbay source has no Form".to_owned())?
        .name
        .clone();

    // The visible graph is authored meaning. A recursive realization may have a
    // different expanded identity and Back evidence, but it cannot replace the
    // checked Gear/Port/Cord face shown beside the source.
    let visible = match conduit_form::expand_canonical_form(&checked, &entry, &catalog) {
        Ok(visible) => visible,
        Err(error) if error.code == "CND-FRM-045" => {
            return project_incompatible_cord(
                interaction.proposal_identity,
                sequence,
                &checked,
                &entry,
                &catalog,
                error,
            )
        }
        Err(error) => return Err(format!("expand compact Tour Patchbay: {error:?}")),
    };
    admit_topology(&visible)?;
    let realized = recursive
        .then(|| {
            conduit_form::expand_canonical_form_with_backs(
                &checked,
                &entry,
                &catalog,
                &backs(&startup, &catalog)?,
            )
            .map_err(|error| format!("expand recursive compact Tour Patchbay: {error:?}"))
        })
        .transpose()?;
    let realization = realized.as_ref().unwrap_or(&visible);
    admit_topology(realization)?;

    Ok(CompactPatchbayProjection {
        schema: "conduit.tour/compact-patchbay@1",
        sequence,
        source_proposal_id: interaction.proposal_identity,
        source_document_id: visible.source_document_id.as_str().into(),
        checked_form_id: visible.checked_form_id.as_str().into(),
        visible_expanded_form_id: visible.expanded_form_id.as_str().into(),
        realization_expanded_form_id: realization.expanded_form_id.as_str().into(),
        form_name: visible.name.clone(),
        realization: if recursive { "recursive" } else { "direct" },
        gears: visible
            .gears
            .iter()
            .map(|gear| CompactGear {
                gear_id: gear.gear_id.as_str().into(),
                kind_id: gear.kind_id.as_str().into(),
                inputs: gear.inputs.iter().map(port).collect(),
                outputs: gear.outputs.iter().map(port).collect(),
            })
            .collect(),
        cords: visible
            .connections
            .iter()
            .map(|cord| CompactCord {
                source_gear_id: cord.source_gear_id.as_str().into(),
                source_port_id: cord.source_port_id.as_str().into(),
                sink_gear_id: cord.sink_gear_id.as_str().into(),
                sink_port_id: cord.sink_port_id.as_str().into(),
                info_kind: cord.value_kind.as_str().into(),
                temporal: cord.temporal.as_str(),
                invalid: false,
            })
            .collect(),
        realization_gears: realization
            .gears
            .iter()
            .map(|gear| CompactGear {
                gear_id: gear.gear_id.as_str().into(),
                kind_id: gear.kind_id.as_str().into(),
                inputs: gear.inputs.iter().map(port).collect(),
                outputs: gear.outputs.iter().map(port).collect(),
            })
            .collect(),
        realization_cords: realization
            .connections
            .iter()
            .map(|cord| CompactCord {
                source_gear_id: cord.source_gear_id.as_str().into(),
                source_port_id: cord.source_port_id.as_str().into(),
                sink_gear_id: cord.sink_gear_id.as_str().into(),
                sink_port_id: cord.sink_port_id.as_str().into(),
                info_kind: cord.value_kind.as_str().into(),
                temporal: cord.temporal.as_str(),
                invalid: false,
            })
            .collect(),
        realization_backs: realization
            .realization_backs
            .iter()
            .map(|back| CompactBack {
                invocation_path: back.invocation_path.clone(),
                kind_id: back.kind_id.as_str().into(),
                checked_form_id: back.checked_form_id.as_str().into(),
            })
            .collect(),
        diagnostics: Vec::new(),
    })
}

fn project_incompatible_cord(
    source_proposal_id: String,
    sequence: u64,
    checked: &conduit_form::CheckedSyntaxDocument,
    entry: &str,
    catalog: &conduit_form::ProfileCatalog,
    error: conduit_form::CanonicalExpansionDiagnostic,
) -> Result<CompactPatchbayProjection, String> {
    let form = checked
        .forms
        .iter()
        .find(|form| form.name == entry)
        .ok_or_else(|| "compact Tour Patchbay checked Form disappeared".to_owned())?;
    let prefix = format!("{entry}/");
    let mut gears = Vec::new();
    for gear in &form.gears {
        let name = gear
            .name
            .as_deref()
            .ok_or_else(|| "invalid compact Patchbay draft contains an inline Gear".to_owned())?;
        let definition = catalog
            .get(&conduit_core::KindId::new(gear.kind.clone()))
            .ok_or_else(|| format!("invalid compact Patchbay Kind '{}' disappeared", gear.kind))?;
        gears.push(CompactGear {
            gear_id: format!("{prefix}{name}"),
            kind_id: gear.kind.clone(),
            inputs: definition.inputs.iter().map(port).collect(),
            outputs: definition.outputs.iter().map(port).collect(),
        });
    }
    let mut cords = Vec::new();
    let mut diagnostic = None;
    for cord in &form.cords {
        for pair in cord.stages.windows(2) {
            let [conduit_form::CheckedCordStage::Reference(source_name), conduit_form::CheckedCordStage::Reference(sink_name)] =
                pair
            else {
                return Err(
                    "invalid compact Patchbay draft contains a non-reference Cord stage".into(),
                );
            };
            let source = gears
                .iter()
                .find(|gear| gear.gear_id == format!("{prefix}{source_name}"))
                .ok_or_else(|| {
                    format!("invalid compact Patchbay source Gear '{source_name}' disappeared")
                })?;
            let sink = gears
                .iter()
                .find(|gear| gear.gear_id == format!("{prefix}{sink_name}"))
                .ok_or_else(|| {
                    format!("invalid compact Patchbay sink Gear '{sink_name}' disappeared")
                })?;
            let output = source
                .outputs
                .first()
                .ok_or_else(|| format!("Gear '{source_name}' has no output"))?;
            let input = sink
                .inputs
                .first()
                .ok_or_else(|| format!("Gear '{sink_name}' has no input"))?;
            let invalid = output.info_kind != input.info_kind || output.temporal != input.temporal;
            let cord_index = cords.len();
            cords.push(CompactCord {
                source_gear_id: source.gear_id.clone(),
                source_port_id: output.port_id.clone(),
                sink_gear_id: sink.gear_id.clone(),
                sink_port_id: input.port_id.clone(),
                info_kind: output.info_kind.clone(),
                temporal: output.temporal,
                invalid,
            });
            if invalid && diagnostic.is_none() {
                diagnostic = Some(CompactDiagnostic {
                    code: error.code,
                    message: error.message.clone(),
                    fix: format!(
                        "Replace '{sink_name}' with a Gear whose input is {} ({}) or change '{source_name}' to emit {} ({}).",
                        output.info_kind, output.temporal, input.info_kind, input.temporal
                    ),
                    subjects: vec![
                        format!("cord:{cord_index}:{}.{}->{}.{}", source.gear_id, output.port_id, sink.gear_id, input.port_id),
                        source.gear_id.clone(),
                        format!("{}.emitting:{}", source.gear_id, output.port_id),
                        sink.gear_id.clone(),
                        format!("{}.receiving:{}", sink.gear_id, input.port_id),
                    ],
                });
            }
        }
    }
    let diagnostic =
        diagnostic.ok_or_else(|| format!("expand compact Tour Patchbay: {error:?}"))?;
    admit_draft_topology(&gears, &cords)?;
    Ok(CompactPatchbayProjection {
        schema: "conduit.tour/compact-patchbay@1",
        sequence,
        source_proposal_id,
        source_document_id: checked.source_document_id.as_str().into(),
        checked_form_id: form.checked_form_id.as_str().into(),
        visible_expanded_form_id: String::new(),
        realization_expanded_form_id: String::new(),
        form_name: form.name.clone(),
        realization: "invalid-source-proposal",
        realization_gears: gears.clone(),
        realization_cords: cords.clone(),
        gears,
        cords,
        realization_backs: Vec::new(),
        diagnostics: vec![diagnostic],
    })
}

fn admit_draft_topology(gears: &[CompactGear], cords: &[CompactCord]) -> Result<(), String> {
    if gears.len() > MAXIMUM_BROWSER_GEARS || cords.len() > MAXIMUM_BROWSER_CORDS {
        return Err("invalid compact Patchbay draft exceeds its topology bound".into());
    }
    let ports = gears.iter().try_fold(0usize, |count, gear| {
        count
            .checked_add(gear.inputs.len())?
            .checked_add(gear.outputs.len())
    });
    if ports.is_none_or(|count| count > MAXIMUM_COMPACT_PORTS) {
        return Err("invalid compact Patchbay draft exceeds its Port bound".into());
    }
    Ok(())
}

fn admit_topology(form: &ExpandedCanonicalForm) -> Result<(), String> {
    if form.gears.len() > MAXIMUM_BROWSER_GEARS {
        return Err(format!(
            "compact Tour Patchbay Gear bound exceeded: {} > {MAXIMUM_BROWSER_GEARS}",
            form.gears.len()
        ));
    }
    if form.connections.len() > MAXIMUM_BROWSER_CORDS {
        return Err(format!(
            "compact Tour Patchbay Cord bound exceeded: {} > {MAXIMUM_BROWSER_CORDS}",
            form.connections.len()
        ));
    }
    let ports = form.gears.iter().try_fold(0usize, |count, gear| {
        count
            .checked_add(gear.inputs.len())?
            .checked_add(gear.outputs.len())
    });
    if ports.is_none_or(|count| count > MAXIMUM_COMPACT_PORTS) {
        return Err("compact Tour Patchbay Port bound exceeded".into());
    }
    Ok(())
}

fn port(port: &conduit_core::PortDescriptor) -> CompactPort {
    CompactPort {
        port_id: port.port_id.as_str().into(),
        info_kind: port.value_kind.as_str().into(),
        temporal: port.temporal.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MORSE: &str = r#"form signal {
    message: text/literal("SOS")
    morse: text/morse
    light: presentation/indicator
    message > morse > light
}"#;

    #[test]
    fn projects_exact_typed_gears_ports_and_explicit_cords_without_host_facts() {
        let projection = project(MORSE, 7, false).unwrap();
        assert_eq!(projection.sequence, 7);
        assert_eq!(projection.gears.len(), 3);
        assert_eq!(projection.cords.len(), 2);
        assert!(projection.gears.iter().all(|gear| gear
            .inputs
            .iter()
            .chain(&gear.outputs)
            .all(|port| !port.info_kind.is_empty())));
        let encoded = serde_json::to_string(&projection).unwrap();
        for forbidden in ["host_id", "boot_id", "implementation_id", "plan_id"] {
            assert!(!encoded.contains(forbidden));
        }
    }

    #[test]
    fn recursive_realization_preserves_the_face_and_carries_bounded_back_topology() {
        let direct = project(MORSE, 8, false).unwrap();
        let recursive = project(MORSE, 9, true).unwrap();
        assert_eq!(direct.gears, recursive.gears);
        assert_eq!(direct.cords, recursive.cords);
        assert_eq!(direct.realization_gears, direct.gears);
        assert_eq!(direct.realization_cords, direct.cords);
        assert_ne!(recursive.realization_gears, recursive.gears);
        assert_ne!(recursive.realization_cords, recursive.cords);
        assert!(recursive.realization_gears.len() <= MAXIMUM_BROWSER_GEARS);
        assert!(recursive.realization_cords.len() <= MAXIMUM_BROWSER_CORDS);
        assert_eq!(direct.checked_form_id, recursive.checked_form_id);
        assert_eq!(
            direct.visible_expanded_form_id,
            recursive.visible_expanded_form_id
        );
        assert_ne!(
            direct.realization_expanded_form_id,
            recursive.realization_expanded_form_id
        );
        assert!(direct.realization_backs.is_empty());
        assert!(!recursive.realization_backs.is_empty());
    }

    #[test]
    fn invalid_cord_projects_the_draft_and_topology_bounds_still_refuse() {
        assert!(project("form nope {", 1, false)
            .unwrap_err()
            .starts_with("parse compact Tour Patchbay"));
        let wrong_type = r#"form wrong {
    text: text/literal("x")
    light: presentation/indicator
    text > light
}"#;
        let invalid = project(wrong_type, 2, false).unwrap();
        assert_eq!(invalid.realization, "invalid-source-proposal");
        assert!(invalid.visible_expanded_form_id.is_empty());
        assert_eq!(invalid.gears.len(), 2);
        assert_eq!(invalid.cords.len(), 1);
        assert!(invalid.cords[0].invalid);
        assert_eq!(invalid.diagnostics[0].code, "CND-FRM-045");
        assert!(invalid.diagnostics[0].fix.contains("value/text@1"));
        assert!(invalid.diagnostics[0]
            .subjects
            .contains(&"wrong/light.receiving:pattern".to_owned()));

        let mut oversized = String::from("form oversized {\n");
        for index in 0..=MAXIMUM_BROWSER_GEARS {
            oversized.push_str(&format!("g{index}: text/literal(\"x\")\n"));
        }
        oversized.push('}');
        assert!(project(&oversized, 3, false)
            .unwrap_err()
            .contains("Gear bound exceeded"));
    }
}
