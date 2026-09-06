//! Bounded Tour ABI effects, receipts, and manifestation decoding.

use crate::source_interaction::SourceInteractionEvidence;
use conduit_core::{PresentationIdentity, SignIdentity};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct IndicatorSegment {
    level: bool,
    units: u8,
}

#[derive(Debug, Serialize)]
pub(super) struct TourEffect {
    pub(super) schema: &'static str,
    pub(super) effect_kind: &'static str,
    pub(super) source_document_id: String,
    pub(super) checked_form_id: String,
    pub(super) expanded_form_id: String,
    pub(super) plan_id: String,
    pub(super) fragment_id: String,
    pub(super) active_play_id: String,
    pub(super) presentation_id: String,
    pub(super) placement_id: String,
    pub(super) host_id: String,
    pub(super) boot_id: String,
    pub(super) presentation_kind: String,
    pub(super) observation_sequence: u32,
    pub(super) realization: &'static str,
    pub(super) expanded_gears: Vec<TourGearEvidence>,
    pub(super) realization_backs: Vec<TourBackEvidence>,
    pub(super) unit_millis: u16,
    pub(super) segments: Vec<IndicatorSegment>,
    pub(super) text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source_interaction: Option<SourceInteractionEvidence>,
}

#[derive(Debug, Serialize)]
pub(super) struct TourTimerEffect {
    pub(super) schema: &'static str,
    pub(super) effect_kind: &'static str,
    pub(super) active_play_id: String,
    pub(super) placement_id: String,
    pub(super) host_id: String,
    pub(super) boot_id: String,
    pub(super) request_sequence: u32,
    pub(super) duration_millis: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source_interaction: Option<SourceInteractionEvidence>,
}

#[derive(Debug, Serialize)]
pub(super) struct TourKeyEventEffect {
    pub(super) schema: &'static str,
    pub(super) effect_kind: &'static str,
    pub(super) active_play_id: String,
    pub(super) placement_id: String,
    pub(super) host_id: String,
    pub(super) boot_id: String,
    pub(super) request_sequence: u32,
    pub(super) maximum_output_bytes: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source_interaction: Option<SourceInteractionEvidence>,
}

#[derive(Debug, Serialize)]
pub(super) struct TourButtonTransitionEffect {
    pub(super) schema: &'static str,
    pub(super) effect_kind: &'static str,
    pub(super) active_play_id: String,
    pub(super) placement_id: String,
    pub(super) host_id: String,
    pub(super) boot_id: String,
    pub(super) request_sequence: u32,
    pub(super) maximum_output_bytes: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source_interaction: Option<SourceInteractionEvidence>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(super) enum TourHostEffect {
    ClockObservation(Box<TourKeyEventEffect>),
    Manifestation(Box<TourEffect>),
    Timer(Box<TourTimerEffect>),
    KeyEvent(Box<TourKeyEventEffect>),
    PointerEvent(Box<TourKeyEventEffect>),
    ButtonTransition(Box<TourButtonTransitionEffect>),
}

impl TourHostEffect {
    pub(super) fn attach_source_interaction(
        &mut self,
        source_interaction: SourceInteractionEvidence,
    ) {
        match self {
            Self::ClockObservation(effect) => effect.source_interaction = Some(source_interaction),
            Self::Manifestation(effect) => effect.source_interaction = Some(source_interaction),
            Self::Timer(effect) => effect.source_interaction = Some(source_interaction),
            Self::KeyEvent(effect) => effect.source_interaction = Some(source_interaction),
            Self::PointerEvent(effect) => effect.source_interaction = Some(source_interaction),
            Self::ButtonTransition(effect) => effect.source_interaction = Some(source_interaction),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct TourGearEvidence {
    pub(super) gear_id: String,
    pub(super) kind_id: String,
    pub(super) implementation_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct TourBackEvidence {
    pub(super) invocation_path: String,
    pub(super) kind_id: String,
    pub(super) checked_form_id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct TourReceipt {
    pub(super) schema: &'static str,
    pub(super) disposition: &'static str,
    pub(super) active_play_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) presentation_id: Option<String>,
    pub(super) terminal_sign_id: String,
    pub(super) timer_completions: u32,
    pub(super) manifestation_completions: u32,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(super) enum TourProgress {
    Cancellation {
        schema: &'static str,
        effect_kind: &'static str,
        active_play_id: String,
        placement_id: String,
        request_sequence: u32,
    },
    Effect(Box<TourHostEffect>),
    Receipt(Box<TourReceipt>),
    Waiting {
        schema: &'static str,
        disposition: &'static str,
        active_play_id: String,
        pending_effects: usize,
    },
}

#[derive(Debug, Serialize)]
pub(crate) struct TourRefusal {
    pub(super) schema: &'static str,
    pub(super) disposition: &'static str,
    pub(super) category: &'static str,
    pub(super) message: String,
}

pub(crate) fn refusal(message: String) -> TourRefusal {
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
    TourRefusal {
        schema: "conduit.tour/refusal@1",
        disposition: "refused-before-play",
        category,
        message,
    }
}

pub(super) fn decode_manifestation(
    manifestation: &crate::installed_browser::BrowserManifestation,
) -> Result<(u16, Vec<IndicatorSegment>, Option<String>), String> {
    match manifestation.kind_id {
        conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND => {
            let pattern = conduit_text::MorsePattern::decode(&manifestation.canonical_value)
                .map_err(|error| format!("decode planned indicator effect: {error:?}"))?;
            Ok((
                pattern.unit_millis,
                pattern
                    .segments
                    .into_iter()
                    .map(|segment| IndicatorSegment {
                        level: segment.level,
                        units: segment.units,
                    })
                    .collect(),
                None,
            ))
        }
        conduit_semantic_catalog::TEXT_PRESENTATION_KIND => {
            let text = String::from_utf8(manifestation.canonical_value.clone())
                .map_err(|_| "planned text manifestation is not UTF-8")?;
            Ok((0, Vec::new(), Some(text)))
        }
        conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND => {
            let value = conduit_core::StructuredInfoValue::from_canonical_bytes(
                &manifestation.canonical_value,
            )
            .map_err(|error| format!("decode structured manifestation: {error:?}"))?;
            if value.value_type() == &conduit_semantic_catalog::wrapped_quantity_type() {
                let quantity =
                    crate::installed_browser::decode_quantity_leaf(&manifestation.canonical_value)?;
                return Ok((
                    0,
                    Vec::new(),
                    Some(format!(
                        "{} {}",
                        quantity.value(),
                        quantity.unit().form_suffix()
                    )),
                ));
            }
            if value.value_type() != &conduit_language::annotation_bundle_four_type() {
                return Err("structured manifestation has the wrong exact type".into());
            }
            Ok((
                0,
                Vec::new(),
                Some(format!(
                    "4 linguistic annotations · {} canonical bytes",
                    manifestation.canonical_value.len()
                )),
            ))
        }
        conduit_semantic_catalog::SCALAR_VALUE_PRESENTATION_KIND => {
            let scalar = conduit_core::Scalar::decode(&manifestation.canonical_value)
                .map_err(|error| format!("decode scalar manifestation: {error:?}"))?;
            Ok((0, Vec::new(), Some(format_scalar(scalar))))
        }
        conduit_semantic_catalog::BOOL_VALUE_PRESENTATION_KIND => {
            let value = conduit_core::InfoBool::decode(&manifestation.canonical_value)
                .map_err(|error| format!("decode Boolean manifestation: {error:?}"))?;
            Ok((0, Vec::new(), Some(value.get().to_string())))
        }
        conduit_semantic_catalog::BOOL_PRESENTATION_KIND => {
            let value = conduit_core::InfoBool::decode(&manifestation.canonical_value)
                .map_err(|error| format!("decode current Boolean manifestation: {error:?}"))?;
            Ok((0, Vec::new(), Some(value.get().to_string())))
        }
        conduit_semantic_catalog::INDICATOR_STATE_PRESENTATION_KIND => {
            let value = conduit_core::InfoBool::decode(&manifestation.canonical_value)
                .map_err(|error| format!("decode indicator state manifestation: {error:?}"))?;
            Ok((0, Vec::new(), Some(value.get().to_string())))
        }
        conduit_semantic_catalog::COUNT_PRESENTATION_KIND => {
            let encoded: [u8; conduit_semantic_catalog::COUNT_ENCODED_LEN as usize] = manifestation
                .canonical_value
                .as_slice()
                .try_into()
                .map_err(|_| "planned count manifestation is not an exact Count")?;
            Ok((0, Vec::new(), Some(u64::from_le_bytes(encoded).to_string())))
        }
        _ => Err("browser manifestation Kind is not installed in the Tour surface".into()),
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

pub(super) fn receipt(
    disposition: &'static str,
    active_play_id: &conduit_core::ActivePlayId,
    presentation: Option<&PresentationIdentity>,
    sign: &SignIdentity,
    timer_completions: u32,
    manifestation_completions: u32,
) -> TourReceipt {
    TourReceipt {
        schema: "conduit.tour/manifestation-receipt@3",
        disposition,
        active_play_id: active_play_id.as_str().into(),
        presentation_id: presentation.map(|identity| identity.presentation_id.as_str().into()),
        terminal_sign_id: sign.sign_id.as_str().into(),
        timer_completions,
        manifestation_completions,
    }
}
