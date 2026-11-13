//! Immutable implementation definitions from checked expanded Source. Emitting
//! these bytes creates neither initialized availability nor any Host authority.
use conduit_core as c;
use conduit_dht11_observation::ObservationContract;
use conduit_environment_runtime::publication::{PublicationChannel, PublicationContract};
use conduit_plot as p;
use sha2::{Digest, Sha256};
#[path = "software/emit_inventory.rs"]
mod emit_inventory;
#[path = "software/selected.rs"]
mod selected;
#[path = "software/selector.rs"]
mod selector;
pub(crate) use selector::selected as selected_selector;
pub(crate) fn implementation_identity(
    kind: &c::KindId,
    configuration: &[c::ConfigurationEntry],
) -> Result<c::ImplementationId, String> {
    let mut hash = Sha256::new();
    hash.update(kind.as_str().as_bytes());
    hash.update(serde_json::to_vec(configuration).map_err(|e| e.to_string())?);
    Ok(format!("conduit.mcu/software/{:x}", hash.finalize()).into())
}
pub const PROFILE: &str = "conduit.mcu/prepared-software@1";
pub const ARTIFACT: &str = "conduit-mcu-host-runtime/software@1";
pub enum SoftwareRole {
    Composite(crate::composite::Role),
    Tick {
        count: u64,
        period_ms: u64,
    },
    Trigger,
    Selector {
        canonical: Vec<u8>,
        input_bytes: usize,
        output_bytes: usize,
    },
    Dht11Observation,
    RecordingLog {
        maximum_samples: u32,
    },
    Publication {
        channel: PublicationChannel,
        maximum_events: u32,
    },
}
pub struct SoftwareNode {
    pub node: u16,
    pub role: SoftwareRole,
    pub kind: c::KindId,
    pub implementation: Option<c::ImplementationId>,
    pub artifact: Option<c::ArtifactId>,
    pub offer_image: Option<Vec<u8>>,
    /// Build-Host-only ordinary definition for independent template lowering.
    /// Firmware retains only the lossless immutable offer image.
    pub offer: Option<c::CapabilityOffer>,
    /// Exact immutable selected configuration, not a grant or input command.
    pub configuration: Vec<c::ConfigurationEntry>,
}
pub struct SoftwareMaterial {
    pub nodes: Vec<SoftwareNode>,
    pub node_count: usize,
    pub cord_count: usize,
}
/// Source is already checked/expanded by the ordinary authoring compiler. Every
/// supported provider is compared with its actual contract before definition
/// emission. Native pulse/output and resident expressions are separate owners.
pub fn compile(
    source: &p::ExpandedCanonicalPlot,
    maximum_image_bytes: usize,
) -> Result<SoftwareMaterial, String> {
    compile_with_types(source, None, maximum_image_bytes)
}
pub fn compile_with_types(
    source: &p::ExpandedCanonicalPlot,
    types: Option<&p::CheckedSyntaxDocument>,
    maximum_image_bytes: usize,
) -> Result<SoftwareMaterial, String> {
    source
        .validate_expansion()
        .map_err(|e| format!("expanded Source: {e:?}"))?;
    let tick = source
        .gears
        .iter()
        .find(|g| g.kind_id.as_str() == conduit_time::TICK_KIND);
    let cadence = tick
        .map(|g| conduit_time::parse_tick_configuration(&g.configuration))
        .transpose()
        .map_err(|e| format!("Source cadence: {e:?}"))?;
    let maximum_events =
        u32::try_from(cadence.as_ref().map_or(0, |c| c.count)).map_err(|_| "event bound")?;
    let mut nodes = Vec::new();
    for (node, gear) in source.gears.iter().enumerate() {
        if gear.configuration.iter().any(|c| c.key == "program")
            || [
                "machine/pulse/read",
                conduit_i2c_transaction::contract::I2C_KIND,
                conduit_monotonic_wait::clock_at::contract::CLOCK_KIND,
                "machine/tm1637/write",
                conduit_semantic_catalog::RESOURCE_APPEND_KIND,
                conduit_semantic_catalog::RESOURCE_RECORD_SERVICE_KIND,
                // Actual initialized Mask routing and renderer owners have
                // their own exact offers and assigned-kernel binding.
                "face/tee",
                "face/renderer",
                "face/interaction",
            ]
            .contains(&gear.kind_id.as_str())
        {
            continue;
        }
        let (kind, role, native) =
            if let Some((kind, role)) = crate::composite::selected(gear, types)? {
                (kind, SoftwareRole::Composite(role), false)
            } else if gear.kind_id.as_str() == conduit_time::TICK_KIND {
                let cadence = conduit_time::parse_tick_configuration(&gear.configuration)
                    .map_err(|e| format!("Source clock: {e:?}"))?;
                (
                    conduit_time::tick_semantic_contract(),
                    SoftwareRole::Tick {
                        count: cadence.count,
                        period_ms: cadence.period_ms,
                    },
                    true,
                )
            } else if gear.kind_id.as_str() == conduit_time::TICK_TRIGGER_KIND {
                (
                    conduit_time::tick_trigger_semantic_contract(),
                    SoftwareRole::Trigger,
                    false,
                )
            } else if gear.kind_id.as_str() == conduit_dht11_observation::KIND {
                (
                    ObservationContract::prepare()
                        .map_err(|e| format!("observation: {e:?}"))?
                        .kind()
                        .clone(),
                    SoftwareRole::Dht11Observation,
                    false,
                )
            } else if gear.kind_id.as_str() == conduit_environment_runtime::recording::LOG_KIND {
                if maximum_events == 0 {
                    return Err("recording without finite checked clock".into());
                }
                (
                    conduit_environment_runtime::recording::LogContract::prepare(
                        &ObservationContract::prepare()
                            .map_err(|e| format!("recording observation: {e:?}"))?,
                    )
                    .map_err(|e| format!("recording: {e:?}"))?
                    .kind()
                    .clone(),
                    SoftwareRole::RecordingLog {
                        maximum_samples: maximum_events,
                    },
                    false,
                )
            } else if let Some(channel) = PublicationChannel::ALL
                .into_iter()
                .find(|c| c.kind_id() == gear.kind_id.as_str())
            {
                if maximum_events == 0 {
                    return Err("publication without finite checked clock".into());
                }
                (
                    PublicationContract::prepare(channel)
                        .map_err(|e| format!("publication: {e:?}"))?
                        .kind()
                        .clone(),
                    SoftwareRole::Publication {
                        channel,
                        maximum_events,
                    },
                    false,
                )
            } else if let Some((kind, role)) = selected_selector(gear)? {
                (kind, role, false)
            } else {
                return Err(format!(
                    "unsupported initialized software provider: {}",
                    gear.kind_id.as_str()
                ));
            };
        if kind.kind_id != gear.kind_id
            || kind.kind_contract_revision != gear.kind_contract_revision
            || kind.startup_parameters != gear.startup_parameters
            || kind.shorthand != gear.shorthand
            || kind.inputs != gear.inputs
            || kind.outputs != gear.outputs
            || kind.semantic_contract() != gear.semantic_contract
        {
            return Err("software contract differs from checked Source".into());
        }
        let (implementation, artifact, offer) = if native {
            (None, None, None)
        } else {
            let implementation = implementation_identity(&kind.kind_id, &gear.configuration)?;
            let artifact: c::ArtifactId = ARTIFACT.into();
            let mut kind = kind;
            // Composite owners retain the independently constructed finite
            // storage shape (seed/current, pending sides and staged output).
            // A single-slot placeholder cannot realize their checked State law.
            let (queue_items, queue_bytes) = if matches!(&role, SoftwareRole::Composite(_)) {
                (kind.limits.max_queue_items, kind.limits.max_queue_bytes)
            } else {
                (1, 4096)
            };
            kind.limits = c::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: queue_items,
                max_queue_bytes: queue_bytes,
            };
            let offer = c::BackOfferBuilder::new(
                kind,
                c::Back {
                    // Equal code/configuration does not make two mutable Gear
                    // owners interchangeable. Preserve one finite owner per
                    // checked Source instance without enlarging its grant.
                    capability_id: format!(
                        "{}/instance/{:x}",
                        implementation.as_str(),
                        Sha256::digest(gear.gear_id.as_str().as_bytes())
                    )
                    .into(),
                    execution_profile_id: PROFILE.into(),
                    implementation_id: implementation.clone(),
                    artifact_id: artifact.clone(),
                    host_calls: vec![],
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build();
            (Some(implementation), Some(artifact), Some(offer))
        };
        nodes.push(SoftwareNode {
            node: u16::try_from(node).map_err(|_| "node bound")?,
            role,
            kind: gear.kind_id.clone(),
            implementation,
            artifact,
            offer_image: None,
            offer,
            configuration: gear.configuration.clone(),
        });
    }
    selected::prepare_images(source, &mut nodes, maximum_image_bytes)?;
    Ok(SoftwareMaterial {
        nodes,
        node_count: source.gears.len(),
        cord_count: source.connections.len(),
    })
}

/// Emit immutable flash data and constructors for this exact checked wrapper.
/// The consuming Root initializes these providers before publishing the images.
/// Native clock identity and admitted wait values remain actual caller custody.
pub fn emit_rust(
    source: &p::ExpandedCanonicalPlot,
    maximum_image_bytes: usize,
    maximum_generated_bytes: usize,
) -> Result<String, String> {
    let nodes = (0..source.gears.len())
        .map(|n| u16::try_from(n).map_err(|_| "node bound".to_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    emit_rust_for_layout(source, &nodes, maximum_image_bytes, maximum_generated_bytes)
}
/// Use only the independently lowered calibration permutation. Configuration
/// hashes include the actual kernel ordinal; runtime digest rewriting would
/// turn trusted Source pins into a comparison with untrusted assignment data.
pub fn emit_rust_for_layout(
    source: &p::ExpandedCanonicalPlot,
    source_to_kernel: &[u16],
    maximum_image_bytes: usize,
    maximum_generated_bytes: usize,
) -> Result<String, String> {
    emit_rust_for_layout_with_types(
        source,
        None,
        source_to_kernel,
        maximum_image_bytes,
        maximum_generated_bytes,
    )
}
pub fn emit_rust_with_types(
    source: &p::ExpandedCanonicalPlot,
    types: &p::CheckedSyntaxDocument,
    maximum_image_bytes: usize,
    maximum_generated_bytes: usize,
) -> Result<String, String> {
    let nodes = (0..source.gears.len())
        .map(|n| u16::try_from(n).map_err(|_| "node bound".to_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    emit_rust_for_layout_with_types(
        source,
        Some(types),
        &nodes,
        maximum_image_bytes,
        maximum_generated_bytes,
    )
}
pub fn emit_rust_for_layout_with_types(
    source: &p::ExpandedCanonicalPlot,
    types: Option<&p::CheckedSyntaxDocument>,
    source_to_kernel: &[u16],
    maximum_image_bytes: usize,
    maximum_generated_bytes: usize,
) -> Result<String, String> {
    use std::fmt::Write;
    if source_to_kernel.len() != source.gears.len() {
        return Err("Source/kernel map length".into());
    }
    let mut seen = vec![false; source_to_kernel.len()];
    for node in source_to_kernel {
        let slot = seen
            .get_mut(usize::from(*node))
            .ok_or("Source/kernel map range")?;
        if *slot {
            return Err("Source/kernel map duplicate".into());
        }
        *slot = true;
    }
    let mut material = compile_with_types(source, types, maximum_image_bytes)?;
    for node in &mut material.nodes {
        node.node = source_to_kernel[usize::from(node.node)];
    }
    let mut code=format!("pub const NODE_COUNT:usize={};\npub const CORD_COUNT:usize={};\npub const SOURCE:&str={:?};\npub const CHECKED:&str={:?};\npub const EXPANDED:&str={:?};\n",material.node_count,material.cord_count,source.source_document_id.as_str(),source.checked_plot_id.as_str(),source.expanded_plot_id.as_str());
    code.push_str("pub static NATIVE_KINDS:&[(u16,&str)]=&[\n");
    for (node, gear) in source.gears.iter().enumerate() {
        if [
            "machine/pulse/read",
            conduit_i2c_transaction::contract::I2C_KIND,
            conduit_monotonic_wait::clock_at::contract::CLOCK_KIND,
            "machine/tm1637/write",
            conduit_semantic_catalog::RESOURCE_APPEND_KIND,
            conduit_semantic_catalog::RESOURCE_RECORD_SERVICE_KIND,
            "face/renderer",
            conduit_time::TICK_KIND,
        ]
        .contains(&gear.kind_id.as_str())
        {
            writeln!(
                code,
                "({}, {:?}),",
                source_to_kernel[node],
                gear.kind_id.as_str()
            )
            .unwrap();
        }
    }
    code.push_str("];\n");
    code.push_str("pub static CONFIGURATION:&[conduit_mcu_host_runtime::execution_factory::SoftwareConfigurationPin]=&[\n");
    for node in &material.nodes {
        for entry in &node.configuration {
            use conduit_embedded_build::{
                GeneratedConfigurationEntry, GeneratedConfigurationValue as V,
            };
            let value = match &entry.value {
                c::ConfigurationValue::Bool(v) => V::Bool(*v),
                c::ConfigurationValue::I64(v) => V::I64(*v),
                c::ConfigurationValue::U64(v) => V::U64(*v),
                c::ConfigurationValue::Text(v) => V::Text(v.clone()),
                _ => return Err("software configuration outside assigned primitive profile".into()),
            };
            let payload = conduit_embedded_build::encode_assigned_software_configuration(
                &GeneratedConfigurationEntry {
                    node: node.node,
                    key: entry.key.clone(),
                    value,
                },
            )
            .map_err(|e| format!("software assigned configuration: {e:?}"))?;
            let record = c::AssignedRecordRequirement::from_record(c::AssignedRecord {
                tag: c::ASSIGNED_CONFIGURATION,
                payload: &payload,
            });
            writeln!(code,"conduit_mcu_host_runtime::execution_factory::SoftwareConfigurationPin{{node:{},record:conduit_core::AssignedRecordRequirement{{tag:{},payload_digest:{:?}}}}},",node.node,record.tag,record.payload_digest).unwrap();
        }
    }
    code.push_str("];\n");
    code.push_str("pub const CLOCKS:&[(u16,u64,u64)]=&[\n");
    for node in &material.nodes {
        if let SoftwareRole::Tick { count, period_ms } = node.role {
            writeln!(code, "({},{period_ms},{count}),", node.node).unwrap();
        }
    }
    code.push_str("];\n");
    code.push_str("pub static OFFER_IMAGES:&[&[u8]]=&[\n");
    for node in &material.nodes {
        if let Some(image) = &node.offer_image {
            writeln!(code, "&{image:?},").unwrap();
        }
    }
    code.push_str("];\n");
    emit_inventory::append(&mut code, &material)?;
    if code.len() > maximum_generated_bytes {
        return Err("software generated-byte ceiling".into());
    }
    Ok(code)
}

#[cfg(test)]
#[path = "software/recording_tests.rs"]
mod recording_tests;
