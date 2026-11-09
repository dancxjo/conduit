//! Exact selected resident codebooks under the original Source preparation policy.
use super::*;
impl ResidentPreparationSelection {
    pub(super) fn prepare(
        &self,
        document: &conduit_plot::CheckedSyntaxDocument,
        plots: &[ExpandedAuthoringPlot],
    ) -> Result<Vec<conduit_embedded_build::PreparedResidentExpression>, String> {
        let bounds = conduit_embedded_build::ResidentExpressionBounds {
            native: conduit_plot::rust_binding::PreparedNativeBounds {
                canonical_bytes: self.canonical_bytes,
                scopes: self.scopes,
                invariants: self.invariants,
                contracts: self.contracts,
            },
            maximum_occurrences: self.maximum_occurrences,
            maximum_program_bytes: self.maximum_program_bytes,
            maximum_generated_bytes: self.maximum_generated_bytes,
            private_work_units: self.private_work_units,
        };
        let mut images = Vec::new();
        for gear in plots.iter().flat_map(|p| &p.expanded.gears) {
            let Some(configuration) = gear.configuration.iter().find(|c| c.key == "program") else {
                continue;
            };
            if images.len() >= self.maximum_images {
                return Err("selected resident image capacity exhausted".into());
            }
            let conduit_core::ConfigurationValue::Text(text) = &configuration.value else {
                return Err("selected resident program is not canonical text".into());
            };
            let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(text)
                .map_err(|e| format!("selected resident program: {e:?}"))?;
            let temporal = gear
                .inputs
                .first()
                .ok_or("selected resident input absent")?
                .temporal;
            let image = conduit_embedded_build::PreparedResidentExpression::prepare(
                document, &program, temporal, bounds,
            )
            .map_err(|e| format!("selected resident compiler ceiling: {e:?}"))?;
            if image.kind_id() != &gear.kind_id
                || image.offer_description().semantic_contract != gear.semantic_contract
            {
                return Err(
                    "selected resident compiler differs from exact expanded Source Gear contract"
                        .into(),
                );
            }
            if !images.iter().any(
                |known: &conduit_embedded_build::PreparedResidentExpression| {
                    known.artifact_id() == image.artifact_id() && known.kind_id() == image.kind_id()
                },
            ) {
                images.push(image);
            }
        }
        Ok(images)
    }
}
