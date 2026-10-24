//! Trusted build material for output computations. No current Host/Boot, Plan,
//! provider readiness, grant or runtime admission is serialized here.
use crate::DisplayOutputSource;
use conduit_core as c;
use conduit_environment_runtime::appliance::software::{ReadySoftwareBacks, SoftwareRefusal};
use std::fmt::{self, Write};
const MAXIMUM_GEAR_BYTES: usize = 32 * 1024;
pub struct DisplaySourceMaterial {
    identity: c::PlotIdentity,
    gears: Vec<(Vec<u8>, Vec<c::ConfigurationEntry>)>,
}
impl DisplaySourceMaterial {
    pub fn prepare(source: &DisplayOutputSource) -> Result<Self, String> {
        let mut gears = Vec::new();
        for original in &source.plot.expanded.gears {
            if original.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND {
                continue;
            }
            let mut gear = original.clone();
            let configuration = core::mem::take(&mut gear.configuration);
            if configuration.len() != 1
                || !matches!(configuration[0].key.as_str(), "program" | "selector")
                || !matches!(configuration[0].value, c::ConfigurationValue::Text(_))
            {
                return Err("unsupported portable configuration".into());
            }
            let bytes = serde_json::to_vec(&gear).map_err(|e| e.to_string())?;
            if bytes.len() > MAXIMUM_GEAR_BYTES {
                return Err("gear metadata capacity".into());
            }
            gears.push((bytes, configuration));
        }
        if gears.len() != 3 {
            return Err("output computation closure".into());
        }
        Ok(Self {
            identity: c::PlotIdentity {
                source_document_id: source.plot.expanded.source_document_id.clone(),
                checked_plot_id: source.plot.expanded.checked_plot_id.clone(),
                expanded_plot_id: source.plot.expanded.expanded_plot_id.clone(),
            },
            gears,
        })
    }
    pub fn metadata_bytes(&self) -> usize {
        self.gears.iter().map(|(bytes, _)| bytes.len()).sum()
    }
    pub fn largest_metadata_bytes(&self) -> usize {
        self.gears
            .iter()
            .map(|(bytes, _)| bytes.len())
            .max()
            .unwrap_or(0)
    }
    /// Same independently checked material and sequential reconstruction used
    /// by generated firmware. This allocates only during preparation.
    pub fn prepare_software(
        &self,
        host: c::HostId,
        boot: c::BootId,
    ) -> Result<ReadySoftwareBacks, SoftwareRefusal> {
        let gears = self.gears.iter().map(|(bytes, config)| {
            let mut gear: conduit_plot::CheckedGear =
                serde_json::from_slice(bytes).map_err(|_| SoftwareRefusal::Initialization)?;
            gear.configuration = config.clone();
            Ok(gear)
        });
        // SAFETY: private material is constructed solely from the ordinary
        // checked output Source above; no received identities enter this path.
        unsafe {
            ReadySoftwareBacks::prepare_portable_make(self.identity.clone(), gears, host, boot)
        }
    }
    /// Source/metadata and computation texts remain distinct static material.
    /// The receiver reconstructs one gear at a time; it runs no Source compiler.
    pub fn write_rust(&self, out: &mut impl Write) -> fmt::Result {
        writeln!(
            out,
            "// Trusted Make-checked output software; no native authority."
        )?;
        writeln!(out, "use conduit_core as c;")?;
        writeln!(out, "use conduit_environment_runtime::appliance::software::{{ReadySoftwareBacks, SoftwareRefusal}};")?;
        writeln!(out, "pub fn prepare(host:c::HostId, boot:c::BootId) -> Result<ReadySoftwareBacks, SoftwareRefusal> {{")?;
        writeln!(out, "let source=c::PlotIdentity {{ source_document_id:{:?}.into(),checked_plot_id:{:?}.into(),expanded_plot_id:{:?}.into() }};", self.identity.source_document_id.as_str(), self.identity.checked_plot_id.as_str(), self.identity.expanded_plot_id.as_str())?;
        writeln!(out, "let material: &[(&[u8], &str, &str)] = &[")?;
        for (bytes, config) in &self.gears {
            let c::ConfigurationValue::Text(text) = &config[0].value else {
                unreachable!("checked material")
            };
            writeln!(out, "(&{:?}, {:?}, {:?}),", bytes, config[0].key, text)?;
        }
        writeln!(out, "]; let gears=material.iter().map(|(bytes,key,text)| {{ let mut gear: conduit_plot::CheckedGear=serde_json::from_slice(bytes).map_err(|_| SoftwareRefusal::Initialization)?; gear.configuration=alloc::vec![c::ConfigurationEntry {{ key:(*key).into(),value:c::ConfigurationValue::Text((*text).into()) }}]; Ok(gear) }});")?;
        writeln!(out, "// SAFETY: this module is emitted only by the checked Make producer and linked into trusted firmware. No ingress bytes are accepted.")?;
        writeln!(
            out,
            "unsafe {{ ReadySoftwareBacks::prepare_portable_make(source,gears,host,boot) }} }}"
        )
    }
}
