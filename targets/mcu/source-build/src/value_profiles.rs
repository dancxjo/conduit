//! Cold physical value selection from an independently generated local graph.
//! These descriptions establish no initialized Back, capacity grant or authority.
use conduit_core::{BorrowedStructuredValueValidator, PortDirection, StructuredInfoType};
use conduit_embedded_build::{GeneratedCordEndpoint as Endpoint, GeneratedEmbeddedPlan};

/// An exact selected implementation's explicit promise for one typed Port.
/// Fabrication supplies these only for implementations that actually support
/// schema/node values; a matching semantic Kind alone is insufficient.
pub struct PortSchema<'a> {
    pub node: u16,
    pub port: u16,
    pub direction: PortDirection,
    pub artifact: &'a str,
    pub schema: &'a [u8],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot<'a> {
    Canonical(u32),
    Schema {
        canonical: &'a [u8],
        maximum_bytes: u32,
    },
}
pub struct Profiles<'a> {
    /// Original generated Cord order and original per-Cord item counts.
    pub cords: Vec<Slot<'a>>,
    /// Sink aligned with each original queue slot, including external sinks.
    pub cord_sinks: Vec<Endpoint>,
    /// Original routed-output order, independent of occupied Cord slots.
    pub steps: Vec<Slot<'a>>,
    /// Exact routed output aligned with each Step profile (fanout appears once).
    pub step_outputs: Vec<(u16, u16)>,
}

impl Profiles<'_> {
    /// Emit each immutable schema once, with explicit slots for the runtime's
    /// original cold constructor. Target support is a compile-time obligation.
    pub fn emit_rust(&self) -> String {
        use core::fmt::Write;
        let mut schemas = Vec::<&[u8]>::new();
        for slot in self.cords.iter().chain(&self.steps) {
            if let Slot::Schema { canonical, .. } = slot {
                if !schemas.contains(canonical) {
                    schemas.push(canonical);
                }
            }
        }
        let mut text = String::new();
        for (index, schema) in schemas.iter().enumerate() {
            writeln!(text, "static VALUE_SCHEMA_{index}:&[u8]=&{schema:?};").unwrap();
        }
        for (name, slots) in [
            ("CORD_VALUE_PROFILES", &self.cords),
            ("STEP_VALUE_PROFILES", &self.steps),
        ] {
            writeln!(
                text,
                "pub const {name}:&[conduit_mcu_host_runtime::native_values::ValueSlotProfile]=&["
            )
            .unwrap();
            for slot in slots {
                match slot {
                    Slot::Canonical(bytes) => writeln!(text, "conduit_mcu_host_runtime::native_values::ValueSlotProfile::Canonical({bytes}),").unwrap(),
                    Slot::Schema { canonical, maximum_bytes } => {
                        let index=schemas.iter().position(|s| s==canonical).unwrap();
                        writeln!(text, "conduit_mcu_host_runtime::native_values::ValueSlotProfile::Schema{{canonical:VALUE_SCHEMA_{index},maximum_bytes:{maximum_bytes}}},").unwrap();
                    }
                }
            }
            text.push_str("];\n");
        }
        text.push_str("pub const CORD_VALUE_SINKS:&[Option<(u16,u16)>]=&[\n");
        for sink in &self.cord_sinks {
            match sink {
                Endpoint::Local { node, port } => writeln!(text, "Some(({node},{port})),").unwrap(),
                Endpoint::Remote { .. } => writeln!(text, "None,").unwrap(),
            }
        }
        text.push_str("];\n");
        writeln!(
            text,
            "pub const STEP_VALUE_OUTPUTS:&[(u16,u16)]=&{:?};",
            self.step_outputs
        )
        .unwrap();
        text
    }
}

/// Preserve every logical maximum and queue count. All recipients of one
/// output must accept its exact schema before either its queues or staging
/// use node-only storage. Unknown, mixed and external routes stay canonical.
/// The runtime still checks its actual owners and every admission requirement.
pub fn select<'a>(
    kernel: &GeneratedEmbeddedPlan,
    declarations: &[PortSchema<'a>],
) -> Result<Profiles<'a>, &'static str> {
    for (i, declaration) in declarations.iter().enumerate() {
        let mut nodes = kernel.nodes.iter().filter(|n| n.node == declaration.node);
        let node = nodes.next().ok_or("schema-node")?;
        if nodes.next().is_some() || node.artifact_id != declaration.artifact {
            return Err("schema-artifact");
        }
        let ports = match declaration.direction {
            PortDirection::Input => &kernel.input_ports,
            PortDirection::Output => &kernel.output_ports,
        };
        let mut ports = ports
            .iter()
            .filter(|p| p.node == declaration.node && p.port == declaration.port);
        let port = ports.next().ok_or("schema-port")?;
        if ports.next().is_some()
            || declarations[..i].iter().any(|d| {
                d.node == declaration.node
                    && d.port == declaration.port
                    && d.direction == declaration.direction
            })
        {
            return Err("schema-duplicate");
        }
        let ty = StructuredInfoType::from_canonical_bytes(declaration.schema)
            .map_err(|_| "schema-bytes")?;
        if ty
            .profile()
            .map_err(|_| "schema-profile")?
            .value_kind()
            .as_str()
            != port.value_kind
        {
            return Err("schema-kind");
        }
    }
    let declared = |node, port, direction| {
        declarations
            .iter()
            .find(|d| d.node == node && d.port == port && d.direction == direction)
            .map(|d| d.schema)
    };
    let compatible = |source| {
        let Endpoint::Local { node, port } = source else {
            return None;
        };
        let schema = declared(node, port, PortDirection::Output)?;
        let mut seen = false;
        for cord in kernel.cords.iter().filter(|cord| cord.source == source) {
            seen = true;
            let Endpoint::Local { node, port } = cord.sink else {
                return None;
            };
            if declared(node, port, PortDirection::Input) != Some(schema) {
                return None;
            }
        }
        seen.then_some(schema)
    };
    let slot = |schema, maximum_bytes| -> Result<Slot<'a>, &'static str> {
        if let Some(canonical) = schema {
            BorrowedStructuredValueValidator::new(
                canonical,
                usize::try_from(maximum_bytes).map_err(|_| "schema-extent-width")?,
            )
            .map_err(|_| "schema-extent")?;
            Ok(Slot::Schema {
                canonical,
                maximum_bytes,
            })
        } else {
            Ok(Slot::Canonical(maximum_bytes))
        }
    };
    let mut cords = Vec::new();
    let mut cord_sinks = Vec::new();
    for cord in &kernel.cords {
        let selected = slot(compatible(cord.source), cord.maximum_value_bytes.max(1))?;
        cord_sinks.extend(core::iter::repeat_n(
            cord.sink,
            usize::from(cord.item_capacity),
        ));
        cords.extend(core::iter::repeat_n(
            selected,
            usize::from(cord.item_capacity),
        ));
    }
    let mut steps = Vec::new();
    let mut step_outputs = Vec::new();
    for output in &kernel.output_ports {
        let source = Endpoint::Local {
            node: output.node,
            port: output.port,
        };
        if let Some(maximum) = kernel
            .cords
            .iter()
            .filter(|c| c.source == source)
            .map(|c| c.maximum_value_bytes)
            .max()
        {
            steps.push(slot(compatible(source), maximum.max(1))?);
            step_outputs.push((output.node, output.port));
        }
    }
    Ok(Profiles {
        cords,
        cord_sinks,
        steps,
        step_outputs,
    })
}

#[cfg(test)]
#[path = "value_profile_tests.rs"]
mod tests;
