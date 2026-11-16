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
        for plot in plots {
            for gear in &plot.expanded.gears {
                let Some(configuration) = gear.configuration.iter().find(|c| c.key == "program")
                else {
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
                let instance = format!(
                    "{}:{}:{}",
                    document.source_document_id.as_str(),
                    plot.expanded.name,
                    gear.gear_id.as_str()
                );
                let image = conduit_embedded_build::PreparedResidentExpression::prepare_instance(
                    document,
                    &program,
                    temporal,
                    bounds,
                    Some(&instance),
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
                images.push(image);
            }
        }
        Ok(images)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_source_uses_fabricated_occurrence_identity_without_code_deduplication() {
        let source = crate::plot_source::parse("type Reading = {\n value: U8\n}\nplot left (\n reading: Reading...| >> echoed: Reading...|\n) = (.)\nplot right (\n reading: Reading...| >> echoed: Reading...|\n) = (.)\n").unwrap();
        let checked = source.check().unwrap();
        let plots = [
            source.expand_named_for_authoring("left").unwrap(),
            source.expand_named_for_authoring("right").unwrap(),
        ];
        let mut selection = ResidentPreparationSelection {
            maximum_images: 2,
            canonical_bytes: 4096,
            scopes: 64,
            invariants: 64,
            contracts: 64,
            maximum_occurrences: 4096,
            maximum_program_bytes: 524288,
            maximum_generated_bytes: 1048576,
            private_work_units: 1113,
        };
        let images = selection.prepare(&checked, &plots).unwrap();
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].kind_id(), images[1].kind_id());
        assert_ne!(images[0].artifact_id(), images[1].artifact_id());
        // This is the existing native fabrication constructor, not a copied
        // capability card or relaxed equality at remote admission.
        for (plot, image) in plots.iter().zip(&images) {
            let gear = &plot.expanded.gears[0];
            let conduit_core::ConfigurationValue::Text(text) = &gear.configuration[0].value else {
                panic!("checked program")
            };
            let program =
                conduit_plot::PortableExpressionProgram::from_canonical_hex(text).unwrap();
            let fabricated = conduit_embedded_build::PreparedResidentExpression::prepare_instance(
                &checked,
                &program,
                gear.inputs[0].temporal,
                conduit_embedded_build::ResidentExpressionBounds {
                    native: conduit_plot::rust_binding::PreparedNativeBounds {
                        canonical_bytes: 4096,
                        scopes: 64,
                        invariants: 64,
                        contracts: 64,
                    },
                    maximum_occurrences: 4096,
                    maximum_program_bytes: 524288,
                    maximum_generated_bytes: 1048576,
                    private_work_units: 1113,
                },
                Some(&format!(
                    "{}:{}:{}",
                    checked.source_document_id.as_str(),
                    plot.expanded.name,
                    gear.gear_id.as_str()
                )),
            )
            .unwrap();
            assert_eq!(image.offer_description(), fabricated.offer_description());
        }
        selection.maximum_images = 1;
        assert!(selection.prepare(&checked, &plots).is_err());
    }
    #[test]
    #[ignore = "requires private authenticated live member evidence and selected Source"]
    fn current_authenticated_member_residents_match_original_source_preparation() {
        let read = |name| {
            std::fs::read(std::env::var_os(name).expect("explicit private qualification input"))
                .unwrap()
        };
        let source_bytes = read("CONDUIT_RESIDENT_SOURCE");
        let source =
            crate::plot_source::parse(std::str::from_utf8(&source_bytes).unwrap()).unwrap();
        let policy: serde_json::Value =
            serde_json::from_slice(&read("CONDUIT_RESIDENT_POLICY")).unwrap();
        let selection: ResidentPreparationSelection =
            serde_json::from_value(policy["resident_preparation"].clone()).unwrap();
        let plots: Vec<_> = policy["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                source
                    .expand_named_for_authoring(e.as_str().unwrap())
                    .unwrap()
            })
            .collect();
        let images = selection.prepare(&source.check().unwrap(), &plots).unwrap();
        let member: crate::durable_host::owner::MemberPresentationBasis =
            serde_json::from_slice(&read("CONDUIT_RESIDENT_MEMBER")).unwrap();
        member.validate().unwrap();
        let resident: Vec<_> = member
            .host_advertisement
            .capabilities
            .iter()
            .filter(|o| {
                o.implementation.execution_profile_id.as_str()
                    == conduit_embedded_build::RESIDENT_EXPRESSION_PROFILE
            })
            .collect();
        assert_eq!(resident.len(), images.len());
        for offer in resident {
            assert_eq!(
                images
                    .iter()
                    .filter(|i| i.offer_description() == offer)
                    .count(),
                1,
                "actual authenticated resident {} lacks exact original prepared Source image",
                offer.capability_id.as_str()
            );
        }
    }
}
