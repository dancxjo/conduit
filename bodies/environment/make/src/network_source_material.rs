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
        Ok(())
    }
}
