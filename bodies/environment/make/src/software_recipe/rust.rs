//! Firmware build input, emitted only from the independently owned Make recipe.
use super::NativeSoftwareRecipe;
use std::fmt::{self, Write};

impl NativeSoftwareRecipe {
    /// Emit only graph/configuration pins for independent firmware provisioning.
    /// This omits portable software bodies and all current native authority.
    pub fn write_layout_rust(&self, out: &mut impl Write) -> fmt::Result {
        writeln!(
            out,
            "// Generated Make layout pins; independently provision through firmware."
        )?;
        writeln!(out, "use conduit_core as c;")?;
        writeln!(out, "pub const SIGN_ITEMS: u16 = {};", self.sign_items)?;
        writeln!(out, "pub const SIGN_BYTES: u32 = {};", self.sign_bytes)?;
        self.write_records(out)?;
        writeln!(
            out,
            "#[rustfmt::skip]\nstatic CALLS: [c::AssignedIdentity; {}] = [",
            self.host_calls.len()
        )?;
        for call in &self.host_calls {
            writeln!(out, "c::AssignedIdentity({:?}),", call.0)?;
        }
        writeln!(out, "];\nstatic RESOURCES: &[u16] = &{:?};", self.resources)?;
        writeln!(out, "pub fn requirements(host: c::AssignedIdentity, boot: c::AssignedIdentity) -> c::AssignedPlanRequirements<'static> {{ c::AssignedPlanRequirements {{ host, boot, host_calls: &CALLS, resources: RESOURCES, remote_bindings: &[] }} }}")?;
        writeln!(out, "pub fn layout(source: conduit_environment_runtime::appliance::assigned::ResidentSourceIdentity) -> Result<conduit_environment_runtime::assigned::ResidentLayout<'static>, ()> {{")?;
        writeln!(
            out,
            "if source.expanded_plot_digest != {:?} {{ return Err(()); }}",
            self.source.expanded_plot_digest
        )?;
        writeln!(out, "Ok(conduit_environment_runtime::assigned::ResidentLayout {{ records: &RECORDS, counts: {:?} }})\n}}", self.counts)
    }
    fn write_records(&self, out: &mut impl Write) -> fmt::Result {
        writeln!(
            out,
            "#[rustfmt::skip]\nstatic RECORDS: [c::AssignedRecordRequirement; {}] = [",
            self.records.len()
        )?;
        for record in &self.records {
            writeln!(
                out,
                "c::AssignedRecordRequirement {{ tag: {}, payload_digest: {:?} }},",
                record.tag, record.payload_digest
            )?;
        }
        writeln!(out, "];")
    }
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
        writeln!(out, "use conduit_environment_runtime::{{appliance::assigned::{{ResidentAppliance, ResidentComputation, ResidentRoles, ResidentSoftwareOffer}}, assigned::ResidentLayout}};")?;
        self.write_records(out)?;
        write_material(out, &self.computations, &self.texts, &self.offers)?;
        writeln!(out, "pub fn resident() -> ResidentAppliance<'static> {{")?;
        writeln!(
            out,
            "ResidentAppliance {{ cadence: conduit_time::TickConfiguration {{ count: {}, period_ms: {} }}, source: conduit_environment_runtime::appliance::assigned::ResidentSourceIdentity {{ expanded_plot_digest: {:?} }}, layout: ResidentLayout {{ records: &RECORDS, counts: {:?} }},",
            self.cadence.count, self.cadence.period_ms, self.source.expanded_plot_digest, self.counts
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

pub(crate) fn write_material(
    out: &mut impl Write,
    computations: &[conduit_core::AssignedComputationRequirement],
    texts: &[(u16, conduit_core::AssignedIdentity, String, Vec<u8>)],
    offers: &[(u16, Vec<u8>)],
) -> fmt::Result {
    writeln!(
        out,
        "static COMPUTATIONS: [c::AssignedComputationRequirement; {}] = [",
        computations.len()
    )?;
    for computation in computations {
        writeln!(out, "c::AssignedComputationRequirement {{ node: {}, key: c::AssignedIdentity({:?}), text_bytes: {}, text_digest: {:?} }},", computation.node, computation.key.0, computation.text_bytes, computation.text_digest)?;
    }
    writeln!(
        out,
        "];\nstatic TEXTS: [ResidentComputation<'static>; {}] = [",
        texts.len()
    )?;
    for (node, key, text, bytes) in texts {
        writeln!(out, "ResidentComputation {{ node: {node}, key: c::AssignedIdentity({:?}), text: {text:?}, canonical_bytes: Some(&{bytes:?}) }},", key.0)?;
    }
    writeln!(
        out,
        "];\npub fn computations() -> &'static [ResidentComputation<'static>] {{ &TEXTS }}"
    )?;
    writeln!(
        out,
        "static OFFERS: [ResidentSoftwareOffer<'static>; {}] = [",
        offers.len()
    )?;
    for (node, json) in offers {
        writeln!(
            out,
            "unsafe {{ ResidentSoftwareOffer::from_make({node}, &{json:?}) }},"
        )?;
    }
    writeln!(
        out,
        "];\npub fn offers() -> &'static [ResidentSoftwareOffer<'static>] {{ &OFFERS }}"
    )?;
    Ok(())
}
