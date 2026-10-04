//! Chosen-phone and quantitative preparation from one immutable native intent.
//! Source resolution, phonological consistency and commitment remain separate.
use crate::{
    intent_inventory::{
        resolve_intent_inventory_phone, IntentInventoryRefusal, ResolvedIntentPhone,
    },
    profile_admission::{prepare_binding, ProfileRefusal},
    semantic::*,
    utterance_timing::{
        prepare_utterance_timing, EventTimingReceipt, PreparedUtteranceTiming,
        UtteranceTimingRefusal, UtteranceTimingRenderRefusal,
    },
    Renderer, VoiceEvent, SOURCE_ID,
};
use alloc::vec::Vec;

#[derive(Debug)]
pub enum IntentRealizationRefusal {
    Timing(UtteranceTimingRefusal),
    Phone {
        event: usize,
        reason: IntentInventoryRefusal,
    },
    Profile {
        event: usize,
        reason: ProfileRefusal,
    },
    Renderer(UtteranceTimingRenderRefusal),
}

pub struct IntentPhoneReceipt<'a> {
    source: ResolvedIntentPhone<'a>,
    binding: &'a SpeechFormantPhoneBinding,
    basis: SpeechFormantProfileBasis,
}
impl<'a> IntentPhoneReceipt<'a> {
    pub fn source(&self) -> &ResolvedIntentPhone<'a> {
        &self.source
    }
    pub fn binding(&self) -> &'a SpeechFormantPhoneBinding {
        self.binding
    }
    pub fn checked_profile_basis(&self) -> &SpeechFormantProfileBasis {
        &self.basis
    }
}

/// Bounded preparation owns its compact event tape. Rendering cannot substitute
/// another phone sequence and performs no collection growth.
pub struct PreparedIntentRealization<'a> {
    timing: PreparedUtteranceTiming<'a>,
    profile: &'a SpeechFormantVoiceProfile,
    phones: Vec<IntentPhoneReceipt<'a>>,
    events: Vec<VoiceEvent>,
}
impl<'a> PreparedIntentRealization<'a> {
    pub fn source(&self) -> &'a SpeechUtteranceIntent {
        self.timing.source()
    }
    pub fn timing(&self) -> &PreparedUtteranceTiming<'a> {
        &self.timing
    }
    pub fn profile(&self) -> &'a SpeechFormantVoiceProfile {
        self.profile
    }
    /// Segment receipts carry their original global event index; boundaries
    /// remain in the ordered timing receipts rather than becoming phones.
    pub fn phones(&self) -> &[IntentPhoneReceipt<'a>] {
        &self.phones
    }
    pub fn events(&self) -> &[VoiceEvent] {
        &self.events
    }
    pub fn compiled_source_id(&self) -> &'static str {
        SOURCE_ID
    }
    pub fn renderer(&self) -> Result<Renderer<'_>, UtteranceTimingRenderRefusal> {
        self.timing.renderer(&self.events)
    }
}

pub fn prepare_intent_realization<'a>(
    source: &'a SpeechUtteranceIntent,
    inventory: &'a SpeechInventory,
    profile: &'a SpeechFormantVoiceProfile,
    boundaries: &'a SpeechFormantBoundaryProfile,
) -> Result<PreparedIntentRealization<'a>, IntentRealizationRefusal> {
    let timing =
        prepare_utterance_timing(source, boundaries).map_err(IntentRealizationRefusal::Timing)?;
    let mut phones = Vec::with_capacity(timing.receipts().len());
    let mut events = Vec::with_capacity(timing.receipts().len());
    for (event, receipt) in timing.receipts().iter().enumerate() {
        match receipt {
            EventTimingReceipt::Segment { .. } => {
                let resolved = resolve_intent_inventory_phone(source, event, inventory)
                    .map_err(|reason| IntentRealizationRefusal::Phone { event, reason })?;
                let (binding, basis, compact) = prepare_binding(
                    inventory,
                    resolved.definition(),
                    profile,
                    resolved.segment().stress(),
                    false,
                )
                .map_err(|reason| IntentRealizationRefusal::Profile { event, reason })?;
                phones.push(IntentPhoneReceipt {
                    source: resolved,
                    binding,
                    basis,
                });
                events.push(compact);
            }
            EventTimingReceipt::Boundary { event, .. } => events.push(*event),
        }
    }
    // Validate every renderer domain and the total frame bound before any
    // receipt escapes. No PCM is played by preparation.
    timing
        .renderer(&events)
        .map_err(IntentRealizationRefusal::Renderer)?;
    Ok(PreparedIntentRealization {
        timing,
        profile,
        phones,
        events,
    })
}
