//! Exact linked reconstruction of checked network meaning. No offer, provider,
//! Host/Boot, selected Plan, credential or execution authority is constructed.
use std::fmt::{self, Write};
pub struct NetworkSourceMaterial {
    image: Vec<u8>,
}
impl NetworkSourceMaterial {
    pub fn prepare(source: &conduit_plot::ExpandedCanonicalPlot) -> Result<Self, String> {
        Ok(Self {
            image: conduit_plot::flat_image::indexed::encode(source)
                .map_err(|error| format!("network source image: {error:?}"))?,
        })
    }
    pub fn planning_image(&self) -> &[u8] {
        &self.image
    }
    pub fn write_rust(&self, output: &mut impl Write) -> fmt::Result {
        writeln!(
            output,
            "// Exact checked network construction; no current authority.\nuse conduit_core as c;"
        )?;
        // SAFETY: private bytes were produced by the checked flat-image encoder;
        // no caller identity or received wire replaces this original Make graph.
        let image = unsafe {
            conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&self.image)
        }
        .map_err(|_| fmt::Error)?;
        for index in 0..image.gear_count() {
            let gear = image.gear(index).map_err(|_| fmt::Error)?;
            crate::source_recipe::linked_text::write_gear(
                output,
                &format!("network_gear_{index}"),
                &gear,
            )?;
            crate::source_recipe::linked_text::write_configuration(
                output,
                &format!("network_configuration_{index}"),
                &gear,
            )?;
        }
        for (name, result, prefix) in [
            (
                "decode_planning_gear",
                "conduit_plot::CheckedGear",
                "network_gear",
            ),
            (
                "decode_planning_configuration",
                "conduit_plot::flat_image::indexed::IndexedGearConfiguration",
                "network_configuration",
            ),
        ] {
            writeln!(output, "pub fn {name}(index:usize) -> Result<{result},conduit_plot::flat_image::ImageRefusal> {{ match index {{")?;
            for index in 0..image.gear_count() {
                writeln!(output, "{index} => {prefix}_{index}(),")?;
            }
            writeln!(
                output,
                "_ => Err(conduit_plot::flat_image::ImageRefusal::Representation) }} }}"
            )?;
        }
        write_linked_facts(output, &image)?;
        Ok(())
    }
}

// Integrate in NetworkSourceMaterial's owner module after the current freeze.
fn write_linked_facts(
    output: &mut impl std::fmt::Write,
    image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
) -> std::fmt::Result {
    use std::fmt;
    let metadata = image.planning_metadata().map_err(|_| fmt::Error)?;
    let gears = (0..image.gear_count())
        .map(|i| image.gear(i).map_err(|_| fmt::Error))
        .collect::<Result<Vec<_>, _>>()?;
    let supported = gears.iter().all(|g| {
        g.startup_parameters.is_empty()
            && g.semantic_contract.configuration.is_empty()
            && g.semantic_contract.laws.is_empty()
            && g.terminal_transductions.is_empty()
            && g.resource_ports.is_empty()
            && g.configuration.is_empty()
            && g.pool_references.is_empty()
    });
    // Unsupported facts are refused; the preexisting complete owned decoder
    // remains available. Never erase those facts from an admitted flat Source.
    if !supported {
        return writeln!(output, "pub fn linked_planning_facts()->Result<conduit_plot::LinkedPlanningFactsRef<'static>,conduit_plot::flat_image::ImageRefusal>{{Err(conduit_plot::flat_image::ImageRefusal::UnsupportedGraph)}}");
    }
    writeln!(output, "static NETWORK_SEMANTICS:c::KindSemanticContract=c::KindSemanticContract{{configuration:alloc::vec::Vec::new(),laws:alloc::vec::Vec::new()}};")?;
    for (index, gear) in gears.iter().enumerate() {
        for (name, ports) in [("INPUT", &gear.inputs), ("OUTPUT", &gear.outputs)] {
            writeln!(
                output,
                "static NETWORK_{name}_{index}:[c::PortDescriptorRef<'static>;{}]=[",
                ports.len()
            )?;
            for p in ports {
                writeln!(output,"c::PortDescriptorRef{{port_id:{:?},value_kind:{:?},direction:c::PortDirection::{:?},temporal:c::PortTemporal::{:?},abnormal_kind:{:?}}},",p.port_id.as_str(),p.value_kind.as_str(),p.direction,p.temporal,p.abnormal_kind.as_ref().map(|id|id.as_str()))?;
            }
            writeln!(output, "];")?;
        }
    }
    writeln!(
        output,
        "static NETWORK_GEARS:[conduit_plot::CheckedGearRef<'static>;{}]=[",
        gears.len()
    )?;
    for (index, g) in gears.iter().enumerate() {
        let shorthand = g.shorthand.as_ref().map(|(a, b)| (a.as_str(), b.as_str()));
        writeln!(output,"conduit_plot::CheckedGearRef{{gear_id:{:?},kind_id:{:?},kind_contract_revision:{:?},startup_parameters:&[],shorthand:{shorthand:?},inputs:c::PortDescriptorRefs::Borrowed(&NETWORK_INPUT_{index}),outputs:c::PortDescriptorRefs::Borrowed(&NETWORK_OUTPUT_{index}),semantic_contract:&NETWORK_SEMANTICS,terminal_transductions:&[],resource_ports:&[],configuration:&[],pool_references:&[]}},",g.gear_id.as_str(),g.kind_id.as_str(),g.kind_contract_revision.as_str())?;
    }
    writeln!(output, "];")?;
    writeln!(
        output,
        "static NETWORK_CONNECTIONS:[conduit_plot::CheckedConnectionRef<'static>;{}]=[",
        metadata.connections().len()
    )?;
    for c in metadata.connections() {
        writeln!(output,"conduit_plot::CheckedConnectionRef{{source_gear_id:{:?},source_port_id:{:?},sink_gear_id:{:?},sink_port_id:{:?},value_kind:{:?},track:c::ConnectionTrack::{:?},temporal:c::PortTemporal::{:?}}},",c.source_gear_id.as_str(),c.source_port_id.as_str(),c.sink_gear_id.as_str(),c.sink_port_id.as_str(),c.value_kind.as_str(),c.track,c.temporal)?;
    }
    writeln!(output, "];")?;
    writeln!(
        output,
        "static NETWORK_ORIGINS:[conduit_plot::PlanningOriginRef<'static>;{}]=[",
        gears.len()
    )?;
    for g in &gears {
        let span = metadata.source_span(g.gear_id.as_str()).ok_or(fmt::Error)?;
        writeln!(output,"conduit_plot::PlanningOriginRef{{gear_id:{:?},source_span:conduit_plot::Span{{start:{},end:{},line:{},column:{},end_line:{},end_column:{}}}}},",g.gear_id.as_str(),span.start,span.end,span.line,span.column,span.end_line,span.end_column)?;
    }
    writeln!(output, "];")?;
    let identity = metadata.identity();
    writeln!(output,"pub fn linked_planning_facts()->Result<conduit_plot::LinkedPlanningFactsRef<'static>,conduit_plot::flat_image::ImageRefusal>{{Ok(conduit_plot::LinkedPlanningFactsRef{{source_document_id:{:?},checked_plot_id:{:?},expanded_plot_id:{:?},name:{:?},completion:conduit_plot::PlotCompletionPolicy::{:?},gears:&NETWORK_GEARS,connections:&NETWORK_CONNECTIONS,provenance:&NETWORK_ORIGINS}})}}",identity.source_document_id.as_str(),identity.checked_plot_id.as_str(),identity.expanded_plot_id.as_str(),metadata.name(),metadata.completion())
}
