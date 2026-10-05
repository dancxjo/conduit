//! Complete native source coverage and global realization of one immutable intent.
use crate::{
    global_intent_realization::{
        prepare_global_intent, GlobalIntentRefusal, GlobalSegmentPreparation, PreparedGlobalIntent,
    },
    intent_sources::{
        resolve_intent_sources, IntentSourceMaterial, IntentSourcesRefusal, PreparedIntentSources,
    },
    semantic::*,
    utterance_timing::UtteranceTimingRenderRefusal,
    Renderer,
};

pub struct PreparedSourcedGlobalIntent<'a> {
    sources: PreparedIntentSources<'a>,
    realization: PreparedGlobalIntent<'a>,
}
impl<'a> PreparedSourcedGlobalIntent<'a> {
    pub fn source(&self) -> &'a SpeechUtteranceIntent {
        self.sources.intent()
    }
    pub fn sources(&self) -> &PreparedIntentSources<'a> {
        &self.sources
    }
    pub fn realization(&self) -> &PreparedGlobalIntent<'a> {
        &self.realization
    }
    pub fn renderer(&self) -> Result<Renderer<'_>, UtteranceTimingRenderRefusal> {
        self.realization.renderer()
    }
}
#[derive(Debug)]
pub enum SourcedGlobalRefusal<'a> {
    Sources(IntentSourcesRefusal),
    Realization(GlobalIntentRefusal<'a>),
}
/// Materials follow exact event/source order. Native reference resolution runs
/// before realization; either failure prevents a playable aggregate escaping.
/// Resolved references do not establish causality, authority or commitment.
#[allow(clippy::too_many_arguments)] // Explicit immutable bases; no ambient lookup.
pub fn prepare_sourced_global_intent<'a>(
    source: &'a SpeechUtteranceIntent,
    materials: &[IntentSourceMaterial<'a>],
    inventory: &'a SpeechInventory,
    profile: &'a SpeechFormantVoiceProfile,
    boundaries: &'a SpeechFormantBoundaryProfile,
    rules: &'a SpeechAllophoneRuleProfile,
    policy: &'a SpeechAllophoneChoicePolicy,
    evidence: &[Option<GlobalSegmentPreparation<'a>>],
) -> Result<PreparedSourcedGlobalIntent<'a>, SourcedGlobalRefusal<'a>> {
    let sources =
        resolve_intent_sources(source, materials).map_err(SourcedGlobalRefusal::Sources)?;
    let realization = prepare_global_intent(
        source, inventory, profile, boundaries, rules, policy, evidence,
    )
    .map_err(SourcedGlobalRefusal::Realization)?;
    Ok(PreparedSourcedGlobalIntent {
        sources,
        realization,
    })
}
