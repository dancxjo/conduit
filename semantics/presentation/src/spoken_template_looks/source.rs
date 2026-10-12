//! Exact checked-source custody and finite pure evaluation; no Host Play claim.
use super::*;
use crate::{PreparedCheckedSourcePlan, Presentation};
use alloc::{string::ToString, vec};
use conduit_core::{
    validate_canonical_structured_value, ConfigurationValue, StructuredFieldValue,
    StructuredInfoType, StructuredInfoValue,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog, PURE_EXPRESSION_REVISION,
};
use sha2::{Digest, Sha256};

pub struct PreparedSpokenTemplate {
    program: PortableExpressionProgram,
    identity: String,
    plot: PlotIdentity,
    plan_id: PlanId,
    source_digest: [u8; 32],
}

/// The caller first reproduces the ordinary selected Plan with
/// `prepare_checked_source_plan`; raw source/Plan pairs never admit a template.
pub fn prepare_checked_spoken_template(
    source: &PreparedCheckedSourcePlan<'_>,
) -> Result<PreparedSpokenTemplate, SpokenTemplateRefusal> {
    let name = source.expanded().expanded.name.as_str();
    let identity = match name {
        "spoken/brief-template" => "spoken/brief@1",
        "spoken/expanded-template" => "spoken/expanded@1",
        _ => return Err(SpokenTemplateRefusal::Source),
    };
    let checked = check_syntax_document(
        &parse_syntax_document(SPOKEN_TEMPLATE_LOOKS_SOURCE),
        &StartupCatalog::new(),
    )
    .map_err(|_| SpokenTemplateRefusal::Source)?;
    let expected = expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new())
        .map_err(|_| SpokenTemplateRefusal::Source)?;
    if source.expanded() != &expected {
        return Err(SpokenTemplateRefusal::Source);
    }
    let [gear] = expected.expanded.gears.as_slice() else {
        return Err(SpokenTemplateRefusal::Shape);
    };
    if gear.kind_contract_revision.as_str() != PURE_EXPRESSION_REVISION {
        return Err(SpokenTemplateRefusal::Shape);
    }
    let [field] = gear.configuration.as_slice() else {
        return Err(SpokenTemplateRefusal::Shape);
    };
    let ("program", ConfigurationValue::Text(encoded)) = (field.key.as_str(), &field.value) else {
        return Err(SpokenTemplateRefusal::Shape);
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded)
        .map_err(|_| SpokenTemplateRefusal::Shape)?;
    for (name, shape) in [
        ("SpokenTemplateSlots", &program.input_type),
        ("SpokenTemplateOutput", &program.output_type),
    ] {
        let laws = checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .ok_or(SpokenTemplateRefusal::Shape)?;
        if &laws.value_type != shape {
            return Err(SpokenTemplateRefusal::Shape);
        }
    }
    let expanded = &expected.expanded;
    Ok(PreparedSpokenTemplate {
        program,
        identity: identity.into(),
        plan_id: source.plan().plan_id.clone(),
        plot: PlotIdentity {
            source_document_id: expanded.source_document_id.clone(),
            checked_plot_id: expanded.checked_plot_id.clone(),
            expanded_plot_id: expanded.expanded_plot_id.clone(),
        },
        source_digest: Sha256::digest(SPOKEN_TEMPLATE_LOOKS_SOURCE.as_bytes()).into(),
    })
}

impl PreparedSpokenTemplate {
    pub fn producer_plan_id(&self) -> &PlanId {
        &self.plan_id
    }
    pub fn producer_plot(&self) -> &PlotIdentity {
        &self.plot
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Evaluate only slots derived from the current Face. Staleness checks bind
    /// both exact revision and utterance-plan digest; equal counts are insufficient.
    pub fn evaluate(
        &self,
        face: &Presentation,
        slots: &SpokenTemplateSlots,
    ) -> Result<SpokenTemplateWording, SpokenTemplateRefusal> {
        if slots.face_id != face.identity.as_str() || slots.revision != face.revision {
            return Err(SpokenTemplateRefusal::StaleFace);
        }
        let current =
            crate::plan_face_utterances(face).map_err(|_| SpokenTemplateRefusal::Selection)?;
        if slots.plan_digest != current.digest {
            return Err(SpokenTemplateRefusal::StaleFace);
        }
        let leaf = StructuredInfoType::leaf("value/text".into())
            .map_err(|_| SpokenTemplateRefusal::Shape)?;
        let mut fields = Vec::new();
        for (name, words) in [
            "title",
            "status",
            "item_one",
            "item_two",
            "item_three",
            "actions",
        ]
        .into_iter()
        .zip(&slots.values)
        {
            if words.len() > MAX_SPOKEN_TEMPLATE_SLOT_BYTES {
                return Err(SpokenTemplateRefusal::Bound);
            }
            fields.push(
                StructuredFieldValue::new(
                    name,
                    StructuredInfoValue::leaf(leaf.clone(), words.as_bytes().to_vec())
                        .map_err(|_| SpokenTemplateRefusal::Selection)?,
                )
                .map_err(|_| SpokenTemplateRefusal::Selection)?,
            );
        }
        let input = StructuredInfoValue::record(self.program.input_type.clone(), fields)
            .map_err(|_| SpokenTemplateRefusal::Selection)?
            .canonical_bytes()
            .map_err(|_| SpokenTemplateRefusal::Selection)?;
        let output = self
            .program
            .evaluate(&input)
            .map_err(|_| SpokenTemplateRefusal::Evaluation)?;
        let value = validate_canonical_structured_value(&output)
            .map_err(|_| SpokenTemplateRefusal::Evaluation)?;
        let identity = value
            .record_field("identity")
            .map_err(|_| SpokenTemplateRefusal::Evaluation)?
            .ok_or(SpokenTemplateRefusal::Evaluation)?
            .primitive_bytes("value/text")
            .map_err(|_| SpokenTemplateRefusal::Evaluation)?;
        if identity != self.identity.as_bytes() {
            return Err(SpokenTemplateRefusal::Evaluation);
        }
        let revision = value
            .record_field("revision")
            .map_err(|_| SpokenTemplateRefusal::Evaluation)?
            .ok_or(SpokenTemplateRefusal::Evaluation)?
            .primitive_bytes("value/u16")
            .map_err(|_| SpokenTemplateRefusal::Evaluation)?;
        if revision != 1_u16.to_le_bytes() {
            return Err(SpokenTemplateRefusal::Evaluation);
        }
        let mut segments = vec![];
        for name in ["a", "b", "c", "d", "e", "f", "g", "h", "i"] {
            let bytes = value
                .record_field(name)
                .map_err(|_| SpokenTemplateRefusal::Evaluation)?
                .ok_or(SpokenTemplateRefusal::Evaluation)?
                .primitive_bytes("value/text")
                .map_err(|_| SpokenTemplateRefusal::Evaluation)?;
            let words =
                core::str::from_utf8(bytes).map_err(|_| SpokenTemplateRefusal::Evaluation)?;
            if !words.is_empty() {
                segments.push(words.to_string());
            }
        }
        for essential in slots.values.iter().filter(|slot| !slot.is_empty()) {
            if !segments.contains(essential) {
                return Err(SpokenTemplateRefusal::MissingEssential);
            }
        }
        let wording = segments.join(" ");
        if wording.is_empty() || wording.len() > MAX_SPOKEN_TEMPLATE_BYTES {
            return Err(SpokenTemplateRefusal::Bound);
        }
        Ok(SpokenTemplateWording {
            template_identity: self.identity.clone(),
            template_revision: 1,
            template_source_digest: self.source_digest,
            producer_plot: self.plot.clone(),
            producer_plan_id: self.plan_id.clone(),
            source_face_identity: slots.face_id.clone(),
            source_face_revision: slots.revision,
            utterance_plan_digest: slots.plan_digest,
            selected_clause_indices: slots.indices.clone(),
            selected_action_ids: slots.actions.clone(),
            wording_digest: Sha256::digest(wording.as_bytes()).into(),
            segments,
            wording,
        })
    }
}
