//! Bind existing opaque contextual selection to the exact joined material.
use crate::{
    allophone_selection::IntentAllophoneChoice, contextual_gestures::*,
    correspondence::PreparedPhonemePhoneCorrespondence, ipa_shared::*, semantic::*,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum SharedGestureRefusal {
    ForeignInventory,
    ForeignIntent,
    MissingCorrespondence,
    MissingPhonemeMembership,
    MissingSyllable,
    MissingSelectedContext,
    Representation,
    Native(NativeBindingRefusal),
    Gesture(ContextualGestureRefusal),
}
pub struct SharedGesturePhonemeLink<'a, 'material> {
    correspondence: &'a PreparedPhonemePhoneCorrespondence<'material>,
    membership: &'a SharedPhonemeMembership<'material>,
}
impl<'a, 'material> SharedGesturePhonemeLink<'a, 'material> {
    pub fn correspondence(&self) -> &'a PreparedPhonemePhoneCorrespondence<'material> {
        self.correspondence
    }
    pub fn membership(&self) -> &'a SharedPhonemeMembership<'material> {
        self.membership
    }
}
pub struct SharedGestureSyllableLink<'a, 'material> {
    pub syllable: &'a crate::syllable_intent::PreparedSyllableIntent<'material>,
    pub member: &'a SpeechSyllableMemberMatch,
    pub stress: SpeechGestureOriginalStressMatch,
    pub selected_position: Option<SpeechGestureOriginalSyllablePositionMatch>,
}
pub struct PreparedIpaContextualPhoneGestures<
    'a,
    'joined,
    'material,
    Contextual = PreparedContextualPhoneGestures<'a, 'material>,
> {
    joined: &'a PreparedIpaSpeechUtteranceIntent<'joined, 'material>,
    contextual: Contextual,
    phone: &'material SpeechPhoneToken,
    phone_basis: SpeechSequenceReferenceMatch,
    phone_specification: SpeechGestureOriginalPhoneMatch,
    syllables: Vec<SharedGestureSyllableLink<'a, 'material>>,
    links: Vec<SharedGesturePhonemeLink<'a, 'material>>,
}
impl<'a, 'joined, 'material, Contextual>
    PreparedIpaContextualPhoneGestures<'a, 'joined, 'material, Contextual>
where
    'material: 'a,
    'joined: 'a,
{
    pub fn joined(&self) -> &'a PreparedIpaSpeechUtteranceIntent<'joined, 'material> {
        self.joined
    }
    pub fn contextual(&self) -> &Contextual {
        &self.contextual
    }
    pub fn phone(&self) -> &'material SpeechPhoneToken {
        self.phone
    }
    pub fn phone_basis(&self) -> &SpeechSequenceReferenceMatch {
        &self.phone_basis
    }
    pub fn phone_specification(&self) -> &SpeechGestureOriginalPhoneMatch {
        &self.phone_specification
    }
    pub fn phoneme_links(&self) -> &[SharedGesturePhonemeLink<'a, 'material>] {
        &self.links
    }
    pub fn syllable_links(&self) -> &[SharedGestureSyllableLink<'a, 'material>] {
        &self.syllables
    }
    fn prepare_with(
        joined: &'a PreparedIpaSpeechUtteranceIntent<'joined, 'material>,
        choice: &'a IntentAllophoneChoice<'material>,
        lower: impl FnOnce() -> Result<Contextual, ContextualGestureRefusal>,
    ) -> Result<Self, SharedGestureRefusal> {
        use SharedGestureRefusal::*;
        if !core::ptr::eq(choice.inventory(), joined.ipa().inventory()) {
            return Err(ForeignInventory);
        }
        if !core::ptr::eq(choice.occurrence().intent(), joined.shared().original()) {
            return Err(ForeignIntent);
        }
        let occurrence = choice.occurrence().segment().occurrence();
        let phones = joined.shared().components().phones;
        let basis = SpeechSequenceBasisMatch::new(phones.basis().clone(), occurrence.clone())
            .map_err(Native)?;
        let phone_basis = SpeechSequenceReferenceMatch::new(
            basis,
            u32::try_from(phones.tokens().len()).map_err(|_| Representation)?,
        )
        .map_err(Native)?;
        let phone = phones
            .tokens()
            .as_slice()
            .get(usize::try_from(*occurrence.ordinal()).map_err(|_| Representation)?)
            .ok_or(Representation)?;
        let phone_specification = SpeechGestureOriginalPhoneMatch::new(
            choice.occurrence().segment().phone().clone(),
            phone.phone().clone(),
        )
        .map_err(Native)?;
        let mut links = Vec::new();
        let mut correspondence_found = false;
        for prepared in joined.shared().correspondences() {
            let correspondence = prepared.correspondence();
            if !matches!(
                correspondence.kind(),
                SpeechRealizationCorrespondenceKind::Realized
            ) {
                continue;
            }
            if !correspondence
                .phones()
                .as_slice()
                .iter()
                .any(|reference| reference == occurrence)
            {
                continue;
            }
            correspondence_found = true;
            for reference in correspondence.phonemes().as_slice() {
                for membership in joined.phonemes() {
                    if membership.ordinal() == *reference.ordinal()
                        && core::ptr::eq(membership.definition(), choice.phoneme())
                    {
                        links.push(SharedGesturePhonemeLink {
                            correspondence: prepared,
                            membership,
                        });
                    }
                }
            }
        }
        if !correspondence_found {
            return Err(MissingCorrespondence);
        }
        if links.is_empty() {
            return Err(MissingPhonemeMembership);
        }
        let selected_scalar = if matches!(
            choice.state().outcome(),
            SpeechAllophoneChoiceOutcome::SelectedAllophone
        ) {
            Some(
                choice
                    .candidates()
                    .nth(usize::try_from(*choice.state().index()).map_err(|_| Representation)?)
                    .and_then(|candidate| candidate.scalar())
                    .ok_or(MissingSelectedContext)?,
            )
        } else {
            None
        };
        let mut syllables = Vec::new();
        for prepared in joined.shared().syllables() {
            for member in prepared.members() {
                if member.occurrence() != occurrence {
                    continue;
                }
                let stress = SpeechGestureOriginalStressMatch::new(
                    choice.occurrence().segment().stress().clone(),
                    prepared.syllable().stress().clone(),
                )
                .map_err(Native)?;
                let position = prepared
                    .syllable()
                    .phone_positions()
                    .as_slice()
                    .get(usize::try_from(*member.index()).map_err(|_| Representation)?)
                    .ok_or(Representation)?;
                let selected_position = selected_scalar
                    .map(|scalar| {
                        SpeechGestureOriginalSyllablePositionMatch::new(
                            scalar.syllable_position().observation().clone(),
                            SpeechSyllablePositionSpecification::known(*position)?,
                        )
                    })
                    .transpose()
                    .map_err(Native)?;
                syllables.push(SharedGestureSyllableLink {
                    syllable: prepared,
                    member,
                    stress,
                    selected_position,
                });
            }
        }
        if syllables.is_empty() {
            return Err(MissingSyllable);
        }
        let contextual = lower().map_err(Gesture)?;
        Ok(Self {
            joined,
            contextual,
            phone,
            phone_basis,
            phone_specification,
            links,
            syllables,
        })
    }
}

impl<'a, 'joined, 'material> PreparedIpaContextualPhoneGestures<'a, 'joined, 'material>
where
    'material: 'a,
    'joined: 'a,
{
    pub fn prepare(
        joined: &'a PreparedIpaSpeechUtteranceIntent<'joined, 'material>,
        choice: &'a IntentAllophoneChoice<'material>,
        timing: &SpeechGestureTiming,
    ) -> Result<Self, SharedGestureRefusal> {
        Self::prepare_with(joined, choice, || {
            prepare_contextual_phone_gestures(choice, timing)
        })
    }
}
/// Greeting profile uses precisely the same original IPA/correspondence/syllable
/// custody admission as the v1 component profile. Loss policy is explicit.
pub type PreparedIpaContextualGreetingPhoneGestures<'a, 'joined, 'material> =
    PreparedIpaContextualPhoneGestures<
        'a,
        'joined,
        'material,
        crate::PreparedContextualGreetingPhoneGestures<'a, 'material>,
    >;
pub fn prepare_ipa_contextual_greeting_phone_gestures<'a, 'joined, 'material>(
    joined: &'a PreparedIpaSpeechUtteranceIntent<'joined, 'material>,
    choice: &'a IntentAllophoneChoice<'material>,
    timing: &SpeechGestureTiming,
    policy: SpeechGreetingLossPolicy,
) -> Result<PreparedIpaContextualGreetingPhoneGestures<'a, 'joined, 'material>, SharedGestureRefusal>
where
    'material: 'a,
    'joined: 'a,
{
    PreparedIpaContextualGreetingPhoneGestures::prepare_with(joined, choice, || {
        crate::prepare_contextual_greeting_phone_gestures(choice, timing, policy)
    })
}
