#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
//! G1 joins contextual selection with executed IPA and Language commitment custody.
#[path = "common/contextual_ipa_profile/carrier.rs"]
mod carrier;
#[path = "common/committed_carrier/commitment.rs"]
mod commitment;
#[path = "common/contextual_ipa_profile/declaration.rs"]
mod declaration;
use conduit_language::lexical::prepare_lexical_tape;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    allophone_selection::{select_intent_allophone, AllophoneSelectionRefusal},
    chosen_allophone_profile::prepare_chosen_allophone_profile,
    complete_carrier::{CompleteCarrierAcoustics, PreparedCompleteSpeechCarrier},
    correspondence::PreparedSpeechCorrespondence,
    declared_context::ExplicitAllophoneContext,
    ipa_inventory::PreparedIpaInventory,
    semantic::*,
    syllable_structure::PreparedSyllableStructure,
};
use declaration::*;
struct Context {
    style: SpeechCarefulStyleSpecification,
    syllable: SpeechSyllablePositionSpecification,
    prosody: SpeechProsodicContextSpecification,
}
impl Context {
    fn new() -> Self {
        Self {
            style: SpeechCarefulStyleSpecification::known(false).unwrap(),
            syllable: SpeechSyllablePositionSpecification::unknown(),
            prosody: SpeechProsodicContextSpecification::unknown(),
        }
    }
    fn get(&self) -> ExplicitAllophoneContext<'_> {
        ExplicitAllophoneContext {
            careful_style: &self.style,
            syllable_position: &self.syllable,
            prosodic_context: &self.prosody,
        }
    }
}
fn policy() -> SpeechAllophoneChoicePolicy {
    SpeechAllophoneChoicePolicy::new(true, false, false, false, true, false).unwrap()
}
#[test]
fn declared_g1_selects_initial_aspiration_and_medial_plain_without_rewriting_committed_carrier() {
    let definitions = inventory(false);
    let notation = profile();
    let pb = phone_bindings();
    let phb = phoneme_bindings();
    let admitted = PreparedIpaInventory::prepare(
        &definitions,
        &notation,
        notation.variety(),
        notation.revision(),
        &pb,
        &phb,
    )
    .unwrap();
    let source = source();
    let intent = carrier::intent();
    let phones = carrier::phones();
    let phonemes = carrier::phonemes();
    let alignment = carrier::alignment();
    let correspondence = PreparedSpeechCorrespondence::prepare(
        &intent,
        &phonemes,
        &phones,
        source.material(),
        &alignment,
        &admitted,
    )
    .unwrap();
    let syllables = carrier::syllables();
    let boundaries = [];
    let structure = PreparedSyllableStructure::prepare(
        &intent,
        &phones,
        source.material(),
        &syllables,
        &boundaries,
    )
    .unwrap();
    let lexical = prepare_lexical_tape(&source, &lexical_profile(), None).unwrap();
    let words = carrier::words();
    let request = commitment::request();
    let result = commitment::execute(&request);
    let complete = PreparedCompleteSpeechCarrier::prepare(
        &correspondence,
        &structure,
        &lexical,
        &words,
        &request,
        &result,
        CompleteCarrierAcoustics::not_supplied(),
    )
    .unwrap();
    let before = complete.canonical_bytes().to_vec();
    let identity = complete.identity();
    let original_intent = intent.clone().encode().unwrap();
    let request_bytes = request.clone().encode().unwrap();
    let result_bytes = result.clone().encode().unwrap();
    let voice = formant(&definitions);
    let context = Context::new();
    let policy = policy();
    let boundary_profile = SpeechFormantBoundaryProfile::new(seq([])).unwrap();
    let contexts = [Some(context.get()); 12];
    let contextual = conduit_speech::contextual_intent_realization::prepare_contextual_intent(
        complete.correspondence().intent(),
        &definitions,
        &voice,
        &boundary_profile,
        &policy,
        &contexts,
    )
    .unwrap();
    assert!(core::ptr::eq(
        contextual.source(),
        complete.correspondence().intent()
    ));
    assert_eq!(contextual.phones().len(), 12);
    for n in 0..12 {
        let choice = select_intent_allophone(
            complete.correspondence().intent(),
            n,
            &definitions,
            &policy,
            context.get(),
        )
        .unwrap();
        let prepared = prepare_chosen_allophone_profile(&choice, &voice).unwrap();
        let SpeechUtteranceIntentEvent::Segment(original) = &intent.events()[n] else {
            panic!("segment")
        };
        assert!(core::ptr::eq(choice.occurrence().segment(), original));
        assert_eq!(choice.selected_phone(), Some(&pid(phone_name(n))));
        assert_eq!(
            contextual.phones()[n].choice().selected_phone(),
            choice.selected_phone()
        );
        assert_eq!(
            prepared.definition().ipa(),
            if n.is_multiple_of(4) {
                "tʰ"
            } else if n.is_multiple_of(2) {
                "t"
            } else {
                "ɑ"
            }
        );
        assert_eq!(
            original.phone(),
            &PhoneSpecification::known(pid(phone_name(n))).unwrap()
        );
        assert_eq!(original.stress(), &stress(n));
        assert_eq!(
            choice.state().outcome(),
            if n.is_multiple_of(4) {
                &SpeechAllophoneChoiceOutcome::SelectedAllophone
            } else {
                &SpeechAllophoneChoiceOutcome::SelectedDefault
            }
        );
        assert_eq!(prepared.profile().inventory_id(), intent.inventory_id());
    }
    assert_eq!(admitted.basis().variety(), notation.variety());
    assert_eq!(admitted.basis().revision(), notation.revision());
    assert_eq!(
        complete.commitment_request().clone().encode().unwrap(),
        request_bytes
    );
    assert_eq!(
        complete.commitment_result().clone().encode().unwrap(),
        result_bytes
    );
    assert_eq!(
        complete.correspondence().intent().clone().encode().unwrap(),
        original_intent
    );
    assert_eq!(complete.canonical_bytes(), before);
    assert_eq!(complete.identity(), identity);
}
#[test]
fn eligible_unsupported_context_and_foreign_inventory_refuse_at_the_same_selection_entrance() {
    let intent = carrier::intent();
    let context = Context::new();
    let policy = policy();
    let unsupported = inventory(true);
    assert!(matches!(
        select_intent_allophone(&intent, 0, &unsupported, &policy, context.get()),
        Err(AllophoneSelectionRefusal::Context { index: 0, .. })
    ));
    let original = inventory(false);
    let foreign = SpeechInventory::new(
        SpeechInventoryId::new("g1/foreign".into()).unwrap(),
        original.language().clone(),
        original.phonemes().clone(),
        original.phones().clone(),
    )
    .unwrap();
    assert!(matches!(
        select_intent_allophone(&intent, 0, &foreign, &policy, context.get()),
        Err(AllophoneSelectionRefusal::Basis(_))
    ));
    // The original Known aspirated phone is a constraint: disabling its rule
    // cannot silently replace it with the default plain [t].
    let disabled =
        SpeechAllophoneChoicePolicy::new(true, false, false, false, false, false).unwrap();
    let choice = select_intent_allophone(&intent, 0, &original, &disabled, context.get()).unwrap();
    assert_eq!(choice.selected_phone(), None);
    let voice = formant(&original);
    assert!(matches!(
        prepare_chosen_allophone_profile(&choice, &voice),
        Err(conduit_speech::chosen_allophone_profile::ChosenProfileRefusal::NoChosenPhone)
    ));
    assert_eq!(intent, carrier::intent());
}
