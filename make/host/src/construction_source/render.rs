use super::*;
use std::fmt::Write as _;

pub fn canonical_host_configuration_conduit(
    configuration: &HostConfiguration,
) -> Result<String, ConfigurationDiagnostic> {
    let mut canonical = configuration.clone();
    canonical
        .bases
        .sort_by(|left, right| left.kind.cmp(&right.kind));
    canonical
        .resources
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical.buses.sort_by(|a, b| a.id().cmp(b.id()));
    canonical.devices.sort_by(|a, b| a.id().cmp(b.id()));
    if let Some(software) = &mut canonical.software {
        software.canonicalize();
    }
    let mut source = String::new();
    writeln!(&mut source, "host {} {{", canonical.name).map_err(|error| {
        ConfigurationDiagnostic::Encode {
            detail: error.to_string(),
        }
    })?;
    writeln!(&mut source, "  schema = {}", canonical.schema).map_err(|error| {
        ConfigurationDiagnostic::Encode {
            detail: error.to_string(),
        }
    })?;
    write!(
        &mut source,
        "  target = {{architecture: {}, machine: {}",
        string(&canonical.target.architecture)?,
        string(&canonical.target.machine)?
    )
    .map_err(|error| ConfigurationDiagnostic::Encode {
        detail: error.to_string(),
    })?;
    if let Some(board) = &canonical.target.board {
        write!(&mut source, ", board: {}", string(board)?).map_err(|error| {
            ConfigurationDiagnostic::Encode {
                detail: error.to_string(),
            }
        })?;
    }
    if let Some(os) = &canonical.target.os {
        write!(&mut source, ", os: {}", string(os)?).map_err(|error| {
            ConfigurationDiagnostic::Encode {
                detail: error.to_string(),
            }
        })?;
    }
    if let Some(descriptor) = &canonical.target.make_descriptor {
        write!(&mut source, ", make_descriptor: {}", string(descriptor)?).map_err(|error| {
            ConfigurationDiagnostic::Encode {
                detail: error.to_string(),
            }
        })?;
    }
    source.push_str("}\n");
    if let Some(software) = &canonical.software {
        source.push_str("  software = {capabilities: [");
        for (index, selected) in software.capabilities.iter().enumerate() {
            if index != 0 {
                source.push_str(", ");
            }
            write!(
                &mut source,
                "{{kind: {}, contract_revision: {}, implementation: {}}}",
                string(&selected.kind)?,
                string(&selected.contract_revision)?,
                string(&selected.implementation)?
            )
            .map_err(|error| ConfigurationDiagnostic::Encode {
                detail: error.to_string(),
            })?;
        }
        source.push_str("]}\n");
    }
    for base in &canonical.bases {
        write!(&mut source, "  base = {{kind: {}", string(&base.kind)?).map_err(|error| {
            ConfigurationDiagnostic::Encode {
                detail: error.to_string(),
            }
        })?;
        if let Some(implementation) = &base.implementation {
            write!(&mut source, ", implementation: {}", string(implementation)?).map_err(
                |error| ConfigurationDiagnostic::Encode {
                    detail: error.to_string(),
                },
            )?;
        }
        if !base.implementations.is_empty() {
            let implementations = base
                .implementations
                .iter()
                .map(|item| string(item))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            write!(&mut source, ", implementations: [{implementations}]").map_err(|error| {
                ConfigurationDiagnostic::Encode {
                    detail: error.to_string(),
                }
            })?;
        }
        if let Some(initialization) = &base.initialization {
            initialization
                .validate_source()
                .map_err(|detail| ConfigurationDiagnostic::Encode { detail })?;
            write!(&mut source, ", initialization: {{").map_err(|error| {
                ConfigurationDiagnostic::Encode {
                    detail: error.to_string(),
                }
            })?;
            for (index, (key, value)) in initialization.values.iter().enumerate() {
                if index != 0 {
                    source.push_str(", ");
                }
                write!(&mut source, "{}: {}", key, value).map_err(|error| {
                    ConfigurationDiagnostic::Encode {
                        detail: error.to_string(),
                    }
                })?;
            }
            source.push('}');
        }
        source.push_str("}\n");
    }
    for need in &canonical.resources {
        writeln!(
            &mut source,
            "  need = {{id: {}, class: {}, slots: {}, bytes: {}}}",
            string(&need.id)?,
            string(&need.class)?,
            need.slots,
            need.bytes
        )
        .map_err(|error| ConfigurationDiagnostic::Encode {
            detail: error.to_string(),
        })?;
    }
    crate::configuration::write_attachment_conduit(
        &mut source,
        &canonical.buses,
        &canonical.devices,
    )
    .map_err(|e| ConfigurationDiagnostic::Encode {
        detail: e.to_string(),
    })?;
    let limits = &canonical.limits;
    writeln!(
        &mut source,
        "  limits = {{static_memory_bytes: {}, heap_arena_bytes: {}, queue_items: {}, buffered_bytes: {}, active_instances: {}, operation_slots: {}, timer_slots: {}, line_sessions: {}, evidence_items: {}}}",
        limits.static_memory_bytes,
        limits.heap_arena_bytes,
        limits.queue_items,
        limits.buffered_bytes,
        limits.active_instances,
        limits.operation_slots,
        limits.timer_slots,
        limits.line_sessions,
        limits.evidence_items
    )
    .map_err(|error| ConfigurationDiagnostic::Encode { detail: error.to_string() })?;
    source.push_str("}\n");
    Ok(source)
}
