//! Current Host selection of exact checked spoken template producers.
use super::parsec_source_plan::{expanded, Producer};
use conduit_core::{
    CapabilityOffer, ConfigurationValue, HostAdvertisement, PreparationHostIdentity,
};
use conduit_face::{
    prepare_checked_spoken_template, PreparedSpokenTemplate, SPOKEN_TEMPLATE_LOOKS_SOURCE,
};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CheckedSyntaxDocument, PortableExpressionProgram,
    StartupCatalog,
};

fn checked() -> Result<CheckedSyntaxDocument, String> {
    check_syntax_document(
        &parse_syntax_document(SPOKEN_TEMPLATE_LOOKS_SOURCE),
        &StartupCatalog::new(),
    )
    .map_err(|error| format!("spoken template check: {error:?}"))
}

fn required_offer(checked: &CheckedSyntaxDocument, name: &str) -> Result<CapabilityOffer, String> {
    let source = expanded(checked, name)?;
    let [gear] = source.expanded.gears.as_slice() else {
        return Err("spoken template must select one checked expression".into());
    };
    let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
        return Err("spoken template has no checked program".into());
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded)
        .map_err(|error| format!("spoken template program: {error:?}"))?;
    conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
        .map_err(|error| format!("spoken template offer: {error:?}"))
}

pub fn required_spoken_template_offers() -> Result<Vec<CapabilityOffer>, String> {
    let checked = checked()?;
    ["spoken/brief-template", "spoken/expanded-template"]
        .into_iter()
        .map(|name| required_offer(&checked, name))
        .collect()
}

/// Computational source custody only; no Show, voice effect or Body authority.
pub struct SpokenTemplateSourcePlan {
    checked: CheckedSyntaxDocument,
    hosts: [HostAdvertisement; 1],
    producer: Producer,
}
impl SpokenTemplateSourcePlan {
    pub fn prepare(host: &HostAdvertisement, name: &str) -> Result<Self, String> {
        if !matches!(name, "spoken/brief-template" | "spoken/expanded-template") {
            return Err("unknown checked spoken template".into());
        }
        let checked = checked()?;
        let offer = required_offer(&checked, name)?;
        if host
            .capabilities
            .iter()
            .find(|candidate| candidate.capability_id == offer.capability_id)
            != Some(&offer)
        {
            return Err("current Host does not offer the exact checked spoken template".into());
        }
        let hosts = [host.clone()];
        // Six bounded 8192-byte slots and nine output fields plus canonical framing.
        let producer = Producer::prepare_bounded(&checked, &hosts, name, 131_072)?;
        Ok(Self {
            checked,
            hosts,
            producer,
        })
    }
    pub fn evaluator(&self) -> Result<PreparedSpokenTemplate, String> {
        let custody = self.producer.custody(&self.checked, &self.hosts)?;
        prepare_checked_spoken_template(&custody)
            .map_err(|error| format!("spoken template custody: {error:?}"))
    }
    pub fn host_identity(&self) -> PreparationHostIdentity {
        PreparationHostIdentity {
            host_id: self.hosts[0].host_id.clone(),
            boot_id: self.hosts[0].boot_id.clone(),
            offer_generation: self.hosts[0].offer_generation,
        }
    }
}

impl crate::StdHost {
    /// Prepare checked wording from this Host's exact offered template and Face.
    /// The receipt proves source selection and wording only, never speech or Show.
    pub fn prepare_spoken_template_wording(
        &self,
        face: &conduit_face::Face,
        entry: &str,
    ) -> Result<conduit_face::SpokenTemplateWording, String> {
        let source = SpokenTemplateSourcePlan::prepare(self.advertisement(), entry)?;
        let outline = crate::spoken_face_mask::select_spoken_outline(face)
            .map_err(|error| format!("spoken template outline: {error:?}"))?;
        let slots = conduit_face::prepare_spoken_template_slots(face, &outline.text_indices)
            .map_err(|error| format!("spoken template selection: {error:?}"))?;
        source
            .evaluator()?
            .evaluate(face, &slots)
            .map_err(|error| format!("spoken template wording: {error:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StdHost, StdHostConfig};

    #[test]
    fn checked_templates_require_current_exact_constructor_offers() {
        let absent = StdHost::new_with_config(StdHostConfig {
            host_id: "proof/checked-spoken-template".into(),
            boot_id: "proof/checked-spoken-template/boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        });
        assert!(
            SpokenTemplateSourcePlan::prepare(absent.advertisement(), "spoken/brief-template")
                .is_err()
        );
        let offers = required_spoken_template_offers().unwrap();
        let mut advertisement = absent.advertisement().clone();
        advertisement.capabilities.extend(offers.clone());
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        // Select offers before constructing the actual test Host.
        let host = StdHost::from_advertisement(advertisement).unwrap();
        let brief =
            SpokenTemplateSourcePlan::prepare(host.advertisement(), "spoken/brief-template")
                .unwrap();
        let expanded =
            SpokenTemplateSourcePlan::prepare(host.advertisement(), "spoken/expanded-template")
                .unwrap();
        assert_eq!(brief.host_identity().boot_id, host.advertisement().boot_id);
        let brief = brief.evaluator().unwrap();
        let expanded = expanded.evaluator().unwrap();
        assert_ne!(brief.producer_plot(), expanded.producer_plot());
        assert_ne!(brief.producer_plan_id(), expanded.producer_plan_id());
        assert_eq!(brief.identity(), "spoken/brief@1");
        assert_eq!(expanded.identity(), "spoken/expanded@1");
        assert!(SpokenTemplateSourcePlan::prepare(
            host.advertisement(),
            "spoken/invented-template"
        )
        .is_err());
        let mut forged = host.advertisement().clone();
        let offer = forged
            .capabilities
            .iter_mut()
            .find(|candidate| candidate.capability_id == offers[0].capability_id)
            .unwrap();
        offer.implementation.artifact_id = "forged/template-program".into();
        assert!(SpokenTemplateSourcePlan::prepare(&forged, "spoken/brief-template").is_err());
    }
    #[test]
    #[ignore = "requires an explicitly supplied actual retained Todo Face"]
    fn actual_retained_face_uses_current_host_checked_templates() {
        let path = std::path::PathBuf::from(
            std::env::var_os("CONDUIT_SPOKEN_TEMPLATE_FACE").expect("actual Face path"),
        );
        assert!(path.is_absolute());
        let record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let face: conduit_face::Face =
            serde_json::from_value(record.get("face").unwrap_or(&record).clone()).unwrap();
        assert!(face.basis.body_id.is_some());
        let baseline = StdHost::new();
        let mut advertisement = baseline.advertisement().clone();
        advertisement
            .capabilities
            .extend(required_spoken_template_offers().unwrap());
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        let host = StdHost::from_advertisement(advertisement).unwrap();
        let brief = host
            .prepare_spoken_template_wording(&face, "spoken/brief-template")
            .unwrap();
        let expanded = host
            .prepare_spoken_template_wording(&face, "spoken/expanded-template")
            .unwrap();
        assert_eq!(brief.source_face_identity, face.identity.as_str());
        assert_eq!(expanded.source_face_identity, face.identity.as_str());
        assert_ne!(brief.wording, expanded.wording);
        assert_eq!(
            brief.selected_clause_indices,
            expanded.selected_clause_indices
        );
        let plan = conduit_face::plan_face_utterances(&face).unwrap();
        for clause in &plan.clauses {
            assert!(brief.wording.contains(&clause.text));
            assert!(expanded.wording.contains(&clause.text));
        }
        if let Some(path) = std::env::var_os("CONDUIT_SPOKEN_TEMPLATE_WORDING_EVIDENCE") {
            let path = std::path::PathBuf::from(path);
            assert!(path.is_absolute());
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let file = options.open(path).unwrap();
            serde_json::to_writer_pretty(file, &serde_json::json!({
                "proof_class": "software current-Host checked template preparation; no Play, WAV or Show",
                "template_host_id": host.advertisement().host_id,
                "template_host_boot": host.advertisement().boot_id,
                "template_host_offer_generation": host.advertisement().offer_generation,
                "actual_face_identity": face.identity,
                "actual_face_revision": face.revision,
                "aural_clause_count": plan.clauses.len(),
                "brief": brief, "expanded": expanded,
            })).unwrap();
        }
    }
}
