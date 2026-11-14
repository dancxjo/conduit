//! Canonical Conduitese rendering of checked construction intent.
use super::string;
use crate::{ConfigurationDiagnostic, HostConfiguration};
use std::fmt::Write as _;

pub fn canonical_host_configuration_conduit(
    configuration: &HostConfiguration,
) -> Result<String, ConfigurationDiagnostic> {
    let mut canonical = configuration.clone();
    canonical.normalize_declarations();
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
    for capability in &canonical.capabilities {
        write!(
            &mut source,
            "  capability = {{kind: {}",
            string(&capability.kind)?
        )
        .map_err(|error| ConfigurationDiagnostic::Encode {
            detail: error.to_string(),
        })?;
        if let Some(implementation) = &capability.pinned_implementation {
            write!(
                &mut source,
                ", pinned_implementation: {}",
                string(implementation)?
            )
            .map_err(|error| ConfigurationDiagnostic::Encode {
                detail: error.to_string(),
            })?;
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
    if let Some(policy) = canonical.preparation {
        writeln!(&mut source, "  preparation = {{authorizer: {}, maximum_assignment_bytes: {}, maximum_runtime_state_bytes: {}}}",
            string(policy.authorizer.as_str())?, policy.maximum_assignment_bytes, policy.maximum_runtime_state_bytes)
            .map_err(|error| ConfigurationDiagnostic::Encode { detail: error.to_string() })?;
    }
    if let Some(policy) = canonical.execution {
        let continuation = policy.continuation.map_or_else(String::new, |c| format!(", continuation: {{assignment_bytes: {}, runtime_bytes: {}, durable_bytes: {}, operation_slots: {}, active_worksets: {}}}", c.assignment_bytes,c.runtime_bytes,c.durable_bytes,c.operation_slots,c.active_worksets));
        writeln!(
            &mut source,
            "  execution = {{authorizer: {}, maximum_active_worksets: {}{}}}",
            string(policy.authorizer.as_str())?,
            policy.maximum_active_worksets,
            continuation
        )
        .map_err(|error| ConfigurationDiagnostic::Encode {
            detail: error.to_string(),
        })?;
    }
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
