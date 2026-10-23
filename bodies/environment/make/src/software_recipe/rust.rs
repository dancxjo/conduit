//! Firmware build input, emitted only from the independently owned Make recipe.
use super::NativeSoftwareRecipe;
use std::fmt::{self, Write};

impl NativeSoftwareRecipe {
    /// Emit a standalone Rust module with `resident()` borrowing static data.
    /// Provision this module through the trusted firmware build, separately
    /// from assignments. It contains no current Host, Boot, Plan or authority.
    /// This pins acceptance constraints; it does not initialize software, admit
    /// runtime memory or prove that a native provider is available.
    pub fn write_resident_rust(&self, out: &mut impl Write) -> fmt::Result {
        writeln!(
            out,
            "// Generated Make recipe; acceptance constraints, never authority."
        )?;
        writeln!(out, "use conduit_core as c;")?;
        writeln!(out, "use conduit_environment_runtime::{{appliance::assigned::{{ResidentAppliance, ResidentRoles}}, assigned::ResidentLayout}};")?;
        writeln!(
            out,
            "static RECORDS: [c::AssignedRecordRequirement; {}] = [",
            self.records.len()
        )?;
        for record in &self.records {
            writeln!(
                out,
                "c::AssignedRecordRequirement {{ tag: {}, payload_digest: {:?} }},",
                record.tag, record.payload_digest
            )?;
        }
        writeln!(
            out,
            "];\nstatic COMPUTATIONS: [c::AssignedComputationRequirement; {}] = [",
            self.computations.len()
        )?;
        for computation in &self.computations {
            writeln!(out, "c::AssignedComputationRequirement {{ node: {}, key: c::AssignedIdentity({:?}), text_bytes: {}, text_digest: {:?} }},", computation.node, computation.key.0, computation.text_bytes, computation.text_digest)?;
        }
        writeln!(
            out,
            "];\npub fn resident() -> ResidentAppliance<'static> {{"
        )?;
        writeln!(
            out,
            "ResidentAppliance {{ layout: ResidentLayout {{ records: &RECORDS, counts: {:?} }},",
            self.counts
        )?;
        writeln!(out, "computations: c::AssignedComputationProfile {{ maximum_text_bytes: {}, maximum_total_text_bytes: {}, requirements: &COMPUTATIONS }},", self.largest, self.total)?;
        let roles = self.roles;
        writeln!(out, "roles: ResidentRoles {{ pulse: conduit_kernel::NodeId({}), tick: conduit_kernel::NodeId({}), trigger: conduit_kernel::NodeId({}), observation: conduit_kernel::NodeId({}), publications: [", roles.pulse.0, roles.tick.0, roles.trigger.0, roles.observation.0)?;
        for node in roles.publications {
            writeln!(out, "conduit_kernel::NodeId({}),", node.0)?;
        }
        writeln!(out, "] }} }}\n}}")
    }
}
