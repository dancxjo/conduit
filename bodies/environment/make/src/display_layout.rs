//! Independently provisioned acceptance facts from an ordinarily planned output.
//! This material never initializes a provider or authorizes the current Body.
use crate::PreparedDisplayOutput;
use conduit_core as c;
use conduit_embedded_build as e;
use std::fmt::{self, Write};
pub struct DisplayLayoutMaterial {
    source: c::PlotIdentity,
    records: Vec<c::AssignedRecordRequirement>,
    calls: Vec<c::AssignedIdentity>,
    resources: Vec<u16>,
    computations: Vec<c::AssignedComputationRequirement>,
    configurations: Vec<Vec<u8>>,
    fore: Vec<c::AssignedForeRequirement>,
    counts: [u8; c::ASSIGNED_PLAN_COUNT_KINDS],
    write: u16,
    sign_items: u16,
    sign_bytes: u32,
}
impl PreparedDisplayOutput {
    /// Provision the result independently through the trusted firmware build.
    /// Received assignment bytes cannot provide their own acceptance profile.
    pub fn display_layout(&self) -> Result<DisplayLayoutMaterial, String> {
        let [fragment] = self.plan().fragments.as_slice() else {
            return Err("local output fragment".into());
        };
        let image = e::generate_fore_native_plan(
            self.plan(),
            &fragment.fragment_id,
            e::EmbeddedImageBounds::HOST_TOOLING,
            e::EmbeddedComputationBounds {
                maximum_program_bytes: 32 * 1024,
                maximum_total_program_bytes: 96 * 1024,
            },
            2,
        )
        .map_err(|e| format!("output projection: {e:?}"))?;
        let bytes = image
            .encode(DisplayLayoutMaterial::maxima())
            .map_err(|e| format!("output encoding: {e:?}"))?;
        let mut records = image
            .layout_requirements(DisplayLayoutMaterial::maxima())
            .map_err(|e| format!("output layout: {e:?}"))?;
        let configurations = c::AssignedPlanRecords::new(&bytes)
            .map_err(|e| format!("output records: {e:?}"))?
            .map(|r| r.map(|r| (r.tag, r.payload.to_vec())))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("output records: {e:?}"))?;
        let configurations: Vec<_> = configurations
            .into_iter()
            .filter_map(|(tag, payload)| (tag == c::ASSIGNED_CONFIGURATION).then_some(payload))
            .collect();
        records.extend(configurations.iter().map(|payload| {
            c::AssignedRecordRequirement::from_record(c::AssignedRecord {
                tag: c::ASSIGNED_CONFIGURATION,
                payload,
            })
        }));
        let computations = e::assigned_computation_requirements(image.kernel())
            .map_err(|e| format!("output computations: {e:?}"))?;
        let writes = image
            .kernel()
            .nodes
            .iter()
            .filter(|n| n.kind_id == conduit_tm1637_wire::WRITE_KIND)
            .map(|n| n.node)
            .collect::<Vec<_>>();
        let [write] = writes.as_slice() else {
            return Err("one output owner".into());
        };
        let write = *write;
        if image.kernel().nodes.len() != 4
            || image.kernel().cords.len() != 5
            || image.fore().len() != 2
            || computations.len() != 3
            || configurations.len() != 3
        {
            return Err("fixed output geometry".into());
        }
        let mut counts = [0; c::ASSIGNED_PLAN_COUNT_KINDS];
        counts.copy_from_slice(&bytes[80..93]);
        Ok(DisplayLayoutMaterial {
            source: c::PlotIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_plot_id: fragment.checked_plot_id.clone(),
                expanded_plot_id: fragment.expanded_plot_id.clone(),
            },
            records,
            calls: image
                .kernel()
                .host_calls
                .iter()
                .map(|c| c::AssignedIdentity::from_text(&c.contract_id))
                .collect(),
            resources: image
                .kernel()
                .resources
                .iter()
                .map(|r| r.resource)
                .collect(),
            computations,
            configurations,
            fore: image.fore().to_vec(),
            counts,
            write,
            sign_items: self.lowered().sign_items,
            sign_bytes: self.lowered().sign_bytes,
        })
    }
}
impl DisplayLayoutMaterial {
    pub fn maxima() -> c::AssignedPlanMaxima {
        c::AssignedPlanMaxima {
            encoded_bytes: 16 * 1024,
            runtime_state_bytes: 32 * 1024,
            counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
        }
    }
    pub fn source(&self) -> &c::PlotIdentity {
        &self.source
    }
    pub fn records(&self) -> &[c::AssignedRecordRequirement] {
        &self.records
    }
    pub fn fore(&self) -> &[c::AssignedForeRequirement] {
        &self.fore
    }
    pub fn write_rust(&self, out: &mut impl Write) -> fmt::Result {
        writeln!(out,"// Trusted Make output layout; acceptance constraints, never authority.\nuse conduit_core as c;")?;
        writeln!(out,"pub fn source() -> c::PlotIdentity {{ c::PlotIdentity {{source_document_id:{:?}.into(),checked_plot_id:{:?}.into(),expanded_plot_id:{:?}.into()}} }}",self.source.source_document_id.as_str(),self.source.checked_plot_id.as_str(),self.source.expanded_plot_id.as_str())?;
        writeln!(
            out,
            "#[rustfmt::skip]\nstatic RECORDS: [c::AssignedRecordRequirement; {}] = [",
            self.records.len()
        )?;
        for r in &self.records {
            writeln!(
                out,
                "c::AssignedRecordRequirement {{tag:{},payload_digest:{:?}}},",
                r.tag, r.payload_digest
            )?;
        }
        writeln!(out, "];")?;
        writeln!(
            out,
            "#[rustfmt::skip]\nstatic CALLS: [c::AssignedIdentity; {}] = [",
            self.calls.len()
        )?;
        for id in &self.calls {
            writeln!(out, "c::AssignedIdentity({:?}),", id.0)?;
        }
        writeln!(out, "];")?;
        writeln!(
            out,
            "#[rustfmt::skip]\nstatic COMPUTATIONS: [c::AssignedComputationRequirement; {}] = [",
            self.computations.len()
        )?;
        for r in &self.computations {
            writeln!(out,"c::AssignedComputationRequirement {{node:{},key:c::AssignedIdentity({:?}),text_bytes:{},text_digest:{:?}}},",r.node,r.key.0,r.text_bytes,r.text_digest)?;
        }
        writeln!(out, "];")?;
        writeln!(out, "pub fn fore()->[c::AssignedForeRequirement;2] {{ [")?;
        for f in &self.fore {
            let p = f.payload();
            writeln!(out,"c::AssignedForeRequirement::new({},{},c::AssignedForeDirection::{:?},c::AssignedIdentity({:?}),{:?}),",f.endpoint(),f.cord(),f.direction(),&p[5..21],&p[21..])?;
        }
        writeln!(out, "] }}")?;
        writeln!(
            out,
            "#[rustfmt::skip]\nstatic CONFIGURATIONS: [&[u8];{}] = [",
            self.configurations.len()
        )?;
        for r in &self.configurations {
            writeln!(out, "&{:?},", r)?;
        }
        writeln!(out,"];\npub fn configurations()->[c::AssignedRecord<'static>;3] {{core::array::from_fn(|i|c::AssignedRecord {{tag:c::ASSIGNED_CONFIGURATION,payload:CONFIGURATIONS[i]}})}}")?;
        writeln!(out,"pub fn profile<'a>(source:&'a c::PlotIdentity,fore:&'a[c::AssignedForeRequirement])->Result<conduit_environment_runtime::display_output::AssignedDisplayProfile<'a>,()> {{")?;
        writeln!(out,"if source.source_document_id.as_str()!={:?} || source.checked_plot_id.as_str()!={:?} || source.expanded_plot_id.as_str()!={:?} || fore != &self::fore() {{return Err(());}}",self.source.source_document_id.as_str(),self.source.checked_plot_id.as_str(),self.source.expanded_plot_id.as_str())?;
        writeln!(out,"Ok(conduit_environment_runtime::display_output::AssignedDisplayProfile {{source,layout:conduit_environment_runtime::assigned::ResidentLayout {{records:&RECORDS,counts:{:?}}},computations:c::AssignedComputationProfile {{maximum_text_bytes:{},maximum_total_text_bytes:{},requirements:&COMPUTATIONS}},fore,host_calls:&CALLS,resources:&{:?},write:conduit_kernel::NodeId({}),sign_items:{},sign_bytes:{} }}) }}",self.counts,self.computations.iter().map(|c|c.text_bytes).max().unwrap_or(0),self.computations.iter().map(|c|u32::from(c.text_bytes)).sum::<u32>(),self.resources,self.write,self.sign_items,self.sign_bytes)?;
        Ok(())
    }
}
