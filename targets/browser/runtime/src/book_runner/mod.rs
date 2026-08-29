//! Inline Forms executed by the ordinary finite browser Host installation.

mod abi;
mod engine;
mod interaction;

use crate::installed_browser::{advertisement, backs, catalogs, local_bases};
use conduit_core::{
    bind_active_play, bind_presentation, bind_sign, Plan, PlanFragment, PresentationIdentity,
    SignIdentity,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_options, PlanningOptions,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct IndicatorSegment {
    level: bool,
    units: u8,
}

#[derive(Debug, Serialize)]
pub(super) struct BookEffect {
    schema: &'static str,
    source_document_id: String,
    checked_form_id: String,
    expanded_form_id: String,
    plan_id: String,
    fragment_id: String,
    active_play_id: String,
    presentation_id: String,
    placement_id: String,
    host_id: String,
    boot_id: String,
    presentation_kind: String,
    realization: &'static str,
    expanded_gears: Vec<BookGearEvidence>,
    realization_backs: Vec<BookBackEvidence>,
    unit_millis: u16,
    segments: Vec<IndicatorSegment>,
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_interaction: Option<interaction::SourceInteractionEvidence>,
}

#[derive(Debug, Serialize)]
pub(super) struct BookGearEvidence {
    gear_id: String,
    kind_id: String,
    implementation_id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct BookBackEvidence {
    invocation_path: String,
    kind_id: String,
    checked_form_id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct BookReceipt {
    schema: &'static str,
    disposition: &'static str,
    active_play_id: String,
    presentation_id: String,
    terminal_sign_id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct BookRefusal {
    schema: &'static str,
    disposition: &'static str,
    category: &'static str,
    message: String,
}

pub(super) fn refusal(message: String) -> BookRefusal {
    let category = if message.contains("finite browser execution envelope")
        || message.contains("limit")
        || message.contains("Limit")
        || message.contains("bound")
        || message.contains("exceed")
        || message.contains("maximum")
        || message.contains("capacity")
    {
        "browser-bound"
    } else if message.starts_with("parse ") || message.starts_with("check ") {
        "type-or-source"
    } else if message.starts_with("expand recursive ") {
        "recursive-expansion"
    } else if message.starts_with("expand ") {
        "type-or-source"
    } else if message.starts_with("place ") {
        "missing-implementation-or-placement"
    } else if message.contains("authority") || message.contains("Authority") {
        "authority"
    } else if message.contains("resource") || message.contains("Resource") {
        "resource"
    } else {
        "planning-or-preparation"
    };
    BookRefusal {
        schema: "conduit.book/refusal@1",
        disposition: "refused-before-play",
        category,
        message,
    }
}

pub(super) struct BookSession {
    scheduler: engine::BookScheduler,
    pending: engine::PendingManifestation,
    active_play_id: conduit_core::ActivePlayId,
    presentation: PresentationIdentity,
    host_id: conduit_core::HostId,
    boot_id: conduit_core::BootId,
}

impl BookSession {
    pub(super) fn prepare(
        host_id: &str,
        boot_id: &str,
        source: &str,
        play_sequence: u64,
    ) -> Result<(Self, BookEffect), String> {
        Self::prepare_with_realization(
            host_id,
            boot_id,
            source,
            play_sequence,
            MorseRealization::Direct,
        )
    }

    pub(super) fn prepare_recursive(
        host_id: &str,
        boot_id: &str,
        source: &str,
        play_sequence: u64,
    ) -> Result<(Self, BookEffect), String> {
        Self::prepare_with_realization(
            host_id,
            boot_id,
            source,
            play_sequence,
            MorseRealization::Recursive,
        )
    }

    fn prepare_with_realization(
        host_id: &str,
        boot_id: &str,
        source: &str,
        play_sequence: u64,
        realization: MorseRealization,
    ) -> Result<(Self, BookEffect), String> {
        let (startup, catalog) = catalogs()?;
        let syntax = conduit_form::parse_syntax_document(source);
        if let Some(diagnostic) = syntax.diagnostics.first() {
            return Err(format!(
                "parse executable-book Form: {}",
                diagnostic.message
            ));
        }
        let checked = conduit_form::check_syntax_document(&syntax, &startup)
            .map_err(|error| format!("check executable-book Form: {error:?}"))?;
        let entry = checked
            .forms
            .last()
            .ok_or_else(|| "executable-book source has no Form".to_string())?
            .name
            .clone();
        let form = match realization {
            MorseRealization::Direct => {
                conduit_form::expand_canonical_form(&checked, &entry, &catalog)
                    .map_err(|error| format!("expand executable-book Form: {error:?}"))?
            }
            MorseRealization::Recursive => conduit_form::expand_canonical_form_with_backs(
                &checked,
                &entry,
                &catalog,
                &backs(&startup, &catalog)?,
            )
            .map_err(|error| format!("expand recursive executable-book Form: {error:?}"))?,
        };
        let host = advertisement(host_id.into(), boot_id.into());
        let hosts = [host];
        let placements = default_expanded_placements(&form, &hosts)
            .map_err(|error| format!("place executable-book Form: {error:?}"))?;
        let bases = local_bases();
        let plan = plan_expanded_canonical_with_options(
            &form,
            &hosts,
            &placements,
            &bases,
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES
                    as u32,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(|error| format!("plan executable-book Form: {error:?}"))?;
        let realization_backs = plan
            .realization_backs
            .iter()
            .map(|back| BookBackEvidence {
                invocation_path: back.invocation_path.clone(),
                kind_id: back.kind_id.as_str().into(),
                checked_form_id: back.checked_form_id.as_str().into(),
            })
            .collect();
        let fragment = exact_fragment(&plan)?;
        let expanded_gears = fragment
            .placements
            .iter()
            .map(|placement| BookGearEvidence {
                gear_id: placement.gear_id.as_str().into(),
                kind_id: placement.kind_id.as_str().into(),
                implementation_id: placement.implementation_id.as_str().into(),
            })
            .collect();
        let (scheduler, pending) = engine::prepare(fragment)?;
        let active = bind_active_play(
            &plan.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            play_sequence,
        );
        let placement = fragment
            .placements
            .get(usize::from(pending.request.node.0))
            .ok_or_else(|| "manifestation has no planned placement".to_string())?;
        let presentation = bind_presentation(&active.active_play_id, &placement.placement_id, 0);
        let manifestation = decode_manifestation(&pending.manifestation)?;
        let effect = BookEffect {
            schema: "conduit.book/manifestation-effect@2",
            source_document_id: fragment.source_document_id.as_str().into(),
            checked_form_id: fragment.checked_form_id.as_str().into(),
            expanded_form_id: fragment.expanded_form_id.as_str().into(),
            plan_id: fragment.plan_id.as_str().into(),
            fragment_id: fragment.fragment_id.as_str().into(),
            active_play_id: active.active_play_id.as_str().into(),
            presentation_id: presentation.presentation_id.as_str().into(),
            placement_id: placement.placement_id.as_str().into(),
            host_id: fragment.host_id.as_str().into(),
            boot_id: fragment.boot_id.as_str().into(),
            presentation_kind: pending.manifestation.kind_id.into(),
            realization: realization.as_str(),
            expanded_gears,
            realization_backs,
            unit_millis: manifestation.unit_millis,
            segments: manifestation.segments,
            text: manifestation.text,
            source_interaction: None,
        };
        Ok((
            Self {
                scheduler,
                pending,
                active_play_id: active.active_play_id,
                presentation,
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
            },
            effect,
        ))
    }

    pub(super) fn complete(mut self) -> Result<BookReceipt, String> {
        engine::complete_manifestation(&mut self.scheduler, &self.pending)?;
        engine::drive_to_completion(&mut self.scheduler)?;
        let sign = bind_sign(&self.host_id, &self.boot_id, Some(&self.active_play_id), 0);
        Ok(receipt(
            "completed",
            &self.active_play_id,
            &self.presentation,
            &sign,
        ))
    }

    pub(super) fn cancel(mut self) -> Result<BookReceipt, String> {
        self.scheduler
            .cancel()
            .map_err(|error| format!("{error:?}"))?;
        let sign = bind_sign(&self.host_id, &self.boot_id, Some(&self.active_play_id), 0);
        Ok(receipt(
            "cancelled",
            &self.active_play_id,
            &self.presentation,
            &sign,
        ))
    }
}

#[derive(Clone, Copy)]
enum MorseRealization {
    Direct,
    Recursive,
}

impl MorseRealization {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Recursive => "recursive",
        }
    }
}

struct DecodedManifestation {
    unit_millis: u16,
    segments: Vec<IndicatorSegment>,
    text: Option<String>,
}

fn decode_manifestation(
    manifestation: &crate::installed_browser::BrowserManifestation,
) -> Result<DecodedManifestation, String> {
    match manifestation.kind_id {
        conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND => {
            let pattern = conduit_text::MorsePattern::decode(&manifestation.canonical_value)
                .map_err(|error| format!("decode planned indicator effect: {error:?}"))?;
            Ok(DecodedManifestation {
                unit_millis: pattern.unit_millis,
                segments: pattern
                    .segments
                    .into_iter()
                    .map(|segment| IndicatorSegment {
                        level: segment.level,
                        units: segment.units,
                    })
                    .collect(),
                text: None,
            })
        }
        conduit_semantic_catalog::TEXT_PRESENTATION_KIND => {
            let text = String::from_utf8(manifestation.canonical_value.clone())
                .map_err(|_| "planned text manifestation is not UTF-8")?;
            Ok(DecodedManifestation {
                unit_millis: 0,
                segments: Vec::new(),
                text: Some(text),
            })
        }
        conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND => {
            let value = conduit_core::StructuredInfoValue::from_canonical_bytes(
                &manifestation.canonical_value,
            )
            .map_err(|error| format!("decode structured manifestation: {error:?}"))?;
            if value.value_type() != &conduit_language::annotation_bundle_four_type() {
                return Err("structured manifestation has the wrong exact type".into());
            }
            Ok(DecodedManifestation {
                unit_millis: 0,
                segments: Vec::new(),
                text: Some(format!(
                    "4 linguistic annotations · {} canonical bytes",
                    manifestation.canonical_value.len()
                )),
            })
        }
        conduit_semantic_catalog::SCALAR_VALUE_PRESENTATION_KIND => {
            let scalar = conduit_core::Scalar::decode(&manifestation.canonical_value)
                .map_err(|error| format!("decode scalar manifestation: {error:?}"))?;
            Ok(DecodedManifestation {
                unit_millis: 0,
                segments: Vec::new(),
                text: Some(format_scalar(scalar)),
            })
        }
        conduit_semantic_catalog::BOOL_VALUE_PRESENTATION_KIND => {
            let value = conduit_core::InfoBool::decode(&manifestation.canonical_value)
                .map_err(|error| format!("decode Boolean manifestation: {error:?}"))?;
            Ok(DecodedManifestation {
                unit_millis: 0,
                segments: Vec::new(),
                text: Some(value.get().to_string()),
            })
        }
        _ => Err("browser manifestation Kind is not installed in the book surface".into()),
    }
}

fn format_scalar(value: conduit_core::Scalar) -> String {
    let raw = i128::from(value.raw_microunits());
    let magnitude = raw.abs();
    format!(
        "{}{}.{:06}",
        if raw < 0 { "-" } else { "" },
        magnitude / i128::from(conduit_core::Scalar::SCALE),
        magnitude % i128::from(conduit_core::Scalar::SCALE)
    )
}

fn receipt(
    disposition: &'static str,
    active_play_id: &conduit_core::ActivePlayId,
    presentation: &PresentationIdentity,
    sign: &SignIdentity,
) -> BookReceipt {
    BookReceipt {
        schema: "conduit.book/manifestation-receipt@2",
        disposition,
        active_play_id: active_play_id.as_str().into(),
        presentation_id: presentation.presentation_id.as_str().into(),
        terminal_sign_id: sign.sign_id.as_str().into(),
    }
}

fn exact_fragment(plan: &Plan) -> Result<&PlanFragment, String> {
    if plan.fragments.len() != 1 {
        return Err("executable-book Plan must contain exactly one browser fragment".into());
    }
    plan.fragments
        .first()
        .ok_or_else(|| "executable-book Plan has no fragment".into())
}

#[cfg(test)]
mod tests;
