use super::{commitment, constructors, material, transport};
use conduit_language::lexical::prepare_lexical_tape;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::ipa_constructors::IpaConstructor;
use conduit_speech::ipa_inventory::PreparedIpaInventory;

fn many<T: NativeRustBinding>(items: Vec<T>) -> Vec<u8> {
    postcard::to_allocvec(
        &items
            .into_iter()
            .map(|v| v.encode().unwrap())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
/// Evidence-only packet list of existing Native values; no production IR schema.
pub fn serialize() -> Vec<u8> {
    static ORIGINAL: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    ORIGINAL.get_or_init(build).clone()
}
fn build() -> Vec<u8> {
    let source = material::source();
    let lexical = prepare_lexical_tape(&source, &material::lexical_profile(), None).unwrap();
    let inventory = material::inventory();
    let profile = material::profile();
    let pb = material::phone_bindings();
    let phb = material::phoneme_bindings();
    let admitted = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &pb,
        &phb,
    )
    .unwrap();
    let intended = admitted
        .phonemic_from_ipa("wʊdjuweɪ".into(), material::evidence())
        .unwrap();
    let request = commitment::request();
    let result = commitment::execute(&request);
    assert!(*result.accepted());
    let original = material::intent();
    let intent = transport::execute(&original);
    assert_eq!(intent, original);
    let phonemic = constructors::scoped(
        IpaConstructor::Phonemic,
        "wʊdjuweɪ",
        material::evidence(),
        &inventory,
        admitted.basis(),
        &pb,
        &phb,
    );
    let phoneme = constructors::scoped(
        IpaConstructor::Phoneme,
        "eɪ",
        material::evidence(),
        &inventory,
        admitted.basis(),
        &pb,
        &phb,
    );
    assert_eq!(phonemic, intended.transcription().clone().encode().unwrap());
    let packets = vec![
        source.encode().unwrap(),
        lexical.tape().clone().encode().unwrap(),
        intent.encode().unwrap(),
        material::phonemes().encode().unwrap(),
        material::phones().encode().unwrap(),
        inventory.clone().encode().unwrap(),
        profile.clone().encode().unwrap(),
        many(pb),
        many(phb),
        material::alignment().encode().unwrap(),
        many(material::syllables()),
        many(material::boundaries()),
        request.encode().unwrap(),
        result.encode().unwrap(),
        transport::SOURCE.as_bytes().to_vec(),
        commitment::source().into_bytes(),
        phonemic,
        constructors::universal(IpaConstructor::Phone, "j", material::evidence()),
        phoneme,
        many(material::word_phonemes()),
    ];
    postcard::to_allocvec(&packets).unwrap()
}
