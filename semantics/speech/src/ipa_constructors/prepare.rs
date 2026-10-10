//! Executed finite parsing; candidate decoding never substitutes for membership.
use super::IpaConstructor;
use crate::{
    ipa_diagnostic::IpaSourceSpan, ipa_inventory::*, ipa_notation::IpaNotationRefusal,
    ipa_phone::phone_from_ipa, ipa_phonetic::phonetic_from_ipa, semantic::*,
};
use alloc::{boxed::Box, vec::Vec};
use conduit_core::{ConfigurationEntry, ConfigurationValue, KindId, StructuredInfoValue};
use conduit_plot::rust_binding::{record_field_value, NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum IpaConstructorRefusal {
    Native(NativeBindingRefusal),
    Notation(IpaNotationRefusal),
    Inventory(IpaInventoryRefusal),
    Startup(conduit_plot::CanonicalExpansionDiagnostic),
    SourceCorrelation,
    Configuration,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpaConstructorLocation {
    Request,
    Field(&'static str),
    Original(IpaSourceSpan),
}
#[derive(Debug)]
pub struct IpaConstructorDiagnostic {
    pub refusal: IpaConstructorRefusal,
    pub location: Box<IpaConstructorLocation>,
}
impl IpaConstructorDiagnostic {
    fn native(error: NativeBindingRefusal) -> Self {
        Self::field(error, "request")
    }
    fn field(error: NativeBindingRefusal, path: &'static str) -> Self {
        Self {
            refusal: IpaConstructorRefusal::Native(error),
            location: Box::new(IpaConstructorLocation::Field(path)),
        }
    }
    fn inventory(error: IpaInventoryRefusal, path: &'static str) -> Self {
        Self {
            refusal: IpaConstructorRefusal::Inventory(error),
            location: Box::new(IpaConstructorLocation::Field(path)),
        }
    }
    fn configuration() -> Self {
        Self {
            refusal: IpaConstructorRefusal::Configuration,
            location: Box::new(IpaConstructorLocation::Request),
        }
    }
}
/// Only executed parsing and inventory admission can produce this prepared value.
/// Its complete immutable arguments remain in the planned Gear configuration.
pub struct PreparedIpaValue {
    value_kind: KindId,
    bytes: Vec<u8>,
}
impl PreparedIpaValue {
    pub fn value_kind(&self) -> &KindId {
        &self.value_kind
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub fn prepare_configuration(
    constructor: IpaConstructor,
    configuration: &[ConfigurationEntry],
) -> Result<PreparedIpaValue, IpaConstructorDiagnostic> {
    use IpaConstructorDiagnostic as D;
    let parameters = constructor.parameters();
    if configuration.len() != parameters.len()
        || parameters.iter().any(|(name, _, _)| {
            configuration
                .iter()
                .filter(|entry| entry.key == *name)
                .count()
                != 1
        })
    {
        return Err(D::configuration());
    }
    for (name, _, ty) in &parameters {
        let entry = configuration
            .iter()
            .find(|entry| entry.key == *name)
            .unwrap();
        let ConfigurationValue::Structured(value) = &entry.value else {
            return Err(D::configuration());
        };
        if value.profile() != ty.profile().expect("finite parameter").value_kind() {
            return Err(D::configuration());
        }
    }
    let request: SpeechIpaUniversalRequest = decode(configuration, "request")?;
    let original = request.original();
    let whole = || {
        IpaConstructorLocation::Original(
            IpaSourceSpan::from_bytes(original, 0, original.len()).expect("whole body"),
        )
    };
    let output = match constructor {
        IpaConstructor::Phone => phone_from_ipa(original.clone(), request.provenance().clone())
            .map_err(|refusal| D {
                refusal: IpaConstructorRefusal::Notation(refusal),
                location: Box::new(whole()),
            })?
            .notation()
            .clone()
            .encode()
            .map_err(D::native)?,
        IpaConstructor::Phonetic => {
            phonetic_from_ipa(original.clone(), request.provenance().clone())
                .map_err(|error| D {
                    refusal: IpaConstructorRefusal::Notation(error.refusal),
                    location: Box::new(IpaConstructorLocation::Original(error.span)),
                })?
                .transcription()
                .clone()
                .encode()
                .map_err(D::native)?
        }
        IpaConstructor::Phoneme | IpaConstructor::Phonemic => {
            let inventory: SpeechInventory = decode(configuration, "inventory")?;
            let basis_value =
                StructuredInfoValue::from_canonical_bytes(configured(configuration, "basis")?)
                    .map_err(|e| D::field(NativeBindingRefusal::InvalidValue(e), "basis"))?;
            check_basis(&inventory, &basis_value)?;
            let basis = SpeechIpaInventoryNotationBasis::from_structured(basis_value)
                .map_err(|e| D::field(e, "basis"))?;
            let phones: SpeechIpaPhoneBindings = decode(configuration, "phone-bindings")?;
            let phonemes: SpeechIpaPhonemeBindings = decode(configuration, "phoneme-bindings")?;
            if phones.values().len() != inventory.phones().len() {
                return Err(D::inventory(
                    IpaInventoryRefusal::BindingCoverage,
                    "phone-bindings",
                ));
            }
            if phonemes.values().len() != inventory.phonemes().len() {
                return Err(D::inventory(
                    IpaInventoryRefusal::BindingCoverage,
                    "phoneme-bindings",
                ));
            }
            let prepared = PreparedIpaInventory::prepare(
                &inventory,
                basis.profile(),
                basis.variety(),
                basis.revision(),
                phones.values().as_slice(),
                phonemes.values().as_slice(),
            )
            .map_err(|error| {
                let path = match error {
                    IpaInventoryRefusal::ForeignPhoneReference => "inventory",
                    IpaInventoryRefusal::Notation(_) => "basis.profile",
                    _ => "phoneme-bindings",
                };
                D::inventory(error, path)
            })?;
            if constructor == IpaConstructor::Phonemic {
                prepared
                    .phonemic_from_ipa(original.clone(), request.provenance().clone())
                    .map_err(|error| D {
                        refusal: IpaConstructorRefusal::Inventory(error.refusal),
                        location: Box::new(IpaConstructorLocation::Original(error.span)),
                    })?
                    .transcription()
                    .clone()
                    .encode()
                    .map_err(D::native)?
            } else {
                let admitted = prepared
                    .phoneme_from_ipa(original.clone(), request.provenance().clone())
                    .map_err(|error| D {
                        refusal: IpaConstructorRefusal::Inventory(error),
                        location: Box::new(whole()),
                    })?;
                let binding = prepared
                    .phonemes()
                    .iter()
                    .find(|(binding, _)| binding.phoneme() == admitted.definition().identity())
                    .expect("admitted definition binding");
                SpeechPhonemeNotation::new(
                    admitted.basis().clone(),
                    binding.0.clone(),
                    admitted.definition().clone(),
                    admitted.notation().transcription().clone(),
                )
                .map_err(D::native)?
                .encode()
                .map_err(D::native)?
            }
        }
    };
    Ok(PreparedIpaValue {
        value_kind: constructor
            .output_type()
            .profile()
            .expect("finite output")
            .value_kind()
            .clone(),
        bytes: output,
    })
}
fn configured<'a>(
    configuration: &'a [ConfigurationEntry],
    name: &'static str,
) -> Result<&'a [u8], IpaConstructorDiagnostic> {
    configuration
        .iter()
        .find_map(|entry| match &entry.value {
            ConfigurationValue::Structured(value) if entry.key == name => {
                Some(value.canonical_value())
            }
            _ => None,
        })
        .ok_or_else(IpaConstructorDiagnostic::configuration)
}
fn decode<T: NativeRustBinding>(
    configuration: &[ConfigurationEntry],
    name: &'static str,
) -> Result<T, IpaConstructorDiagnostic> {
    T::decode(configured(configuration, name)?)
        .map_err(|e| IpaConstructorDiagnostic::field(e, name))
}
// Locate explicit scope substitutions before root Native invariant refusal.
fn check_basis(
    inventory: &SpeechInventory,
    notation: &StructuredInfoValue,
) -> Result<(), IpaConstructorDiagnostic> {
    use IpaConstructorDiagnostic as D;
    let field = |name, path| record_field_value(notation, name).map_err(|e| D::field(e, path));
    let profile = SpeechIpaNotationProfile::from_structured(field("profile", "basis.profile")?)
        .map_err(|e| D::field(e, "basis.profile"))?;
    let id = SpeechInventoryId::from_structured(field("inventory_id", "basis.inventory_id")?)
        .map_err(|e| D::field(e, "basis.inventory_id"))?;
    if inventory.identity() != &id || profile.inventory_id() != &id {
        return Err(D::inventory(
            IpaInventoryRefusal::Basis,
            "basis.inventory_id",
        ));
    }
    let language =
        conduit_language::LanguageId::from_structured(field("language", "basis.language")?)
            .map_err(|e| D::field(e, "basis.language"))?;
    if inventory.language() != &language {
        return Err(D::inventory(IpaInventoryRefusal::Basis, "basis.language"));
    }
    let variety =
        conduit_language::LanguageVariety::from_structured(field("variety", "basis.variety")?)
            .map_err(|e| D::field(e, "basis.variety"))?;
    if variety.language() != &language || &variety != profile.variety() {
        return Err(D::inventory(IpaInventoryRefusal::Basis, "basis.variety"));
    }
    let revision = SpeechSegmentRevisionId::from_structured(field("revision", "basis.revision")?)
        .map_err(|e| D::field(e, "basis.revision"))?;
    if &revision != profile.revision() {
        return Err(D::inventory(IpaInventoryRefusal::Basis, "basis.revision"));
    }
    Ok(())
}

// The generic literal entrance executes the ordinary owner's preparation. It
// cannot infer membership by decoding the request or by inspecting delimiters.
impl conduit_plot::StaticValueConstructor for IpaConstructor {
    type Refusal = IpaConstructorDiagnostic;
    fn contract(&self) -> conduit_core::Kind {
        super::contract(*self)
    }
    fn result_type(&self) -> conduit_core::StructuredInfoType {
        self.output_type()
    }
    fn prepare_configuration(
        &self,
        configuration: &[ConfigurationEntry],
    ) -> Result<StructuredInfoValue, Self::Refusal> {
        let prepared = prepare_configuration(*self, configuration)?;
        StructuredInfoValue::from_canonical_bytes(prepared.bytes()).map_err(|error| {
            IpaConstructorDiagnostic::native(NativeBindingRefusal::InvalidValue(error))
        })
    }
}

impl conduit_plot::LiteralValueConstructor for IpaConstructor {
    fn context_types(&self) -> Vec<(alloc::string::String, conduit_core::StructuredInfoType)> {
        let mut fields = alloc::vec![(
            "provenance".into(),
            SpeechEvidenceProvenance::semantic_type().expect("checked provenance")
        )];
        fields.extend(
            self.parameters()
                .into_iter()
                .filter(|(name, _, _)| *name != "request")
                .map(|(name, _, ty)| (name.into(), ty)),
        );
        fields
    }
    fn parser_contract(&self) -> &str {
        super::REVISION
    }
    fn lexical_policy(&self) -> conduit_plot::TypedLiteralLexicalPolicy {
        conduit_plot::TypedLiteralLexicalPolicy::RawUnicode
    }
    fn literal_configuration(
        &self,
        literal: &conduit_plot::TypedGlyphLiteralSyntax,
        context: &[ConfigurationEntry],
    ) -> Result<Vec<ConfigurationEntry>, Self::Refusal> {
        let parameters = self.parameters();
        if context.len() != parameters.len()
            || context
                .iter()
                .filter(|entry| entry.key == "provenance")
                .count()
                != 1
            || parameters
                .iter()
                .skip(1)
                .any(|(name, _, _)| context.iter().filter(|entry| entry.key == *name).count() != 1)
        {
            return Err(IpaConstructorDiagnostic::configuration());
        }
        let provenance = context
            .iter()
            .find(|entry| entry.key == "provenance")
            .unwrap();
        let ConfigurationValue::Structured(provenance) = &provenance.value else {
            return Err(IpaConstructorDiagnostic::configuration());
        };
        let provenance = SpeechEvidenceProvenance::decode(provenance.canonical_value())
            .map_err(|error| IpaConstructorDiagnostic::field(error, "provenance"))?;
        let request = SpeechIpaUniversalRequest::new(literal.raw_payload.text.clone(), provenance)
            .map_err(IpaConstructorDiagnostic::native)?;
        let ty = self.request_type();
        let encoded = request.encode().map_err(IpaConstructorDiagnostic::native)?;
        let configured = conduit_core::StructuredConfigurationValue::new(
            ty.profile()
                .map_err(|_| IpaConstructorDiagnostic::configuration())?
                .value_kind()
                .clone(),
            encoded,
        )
        .ok_or_else(IpaConstructorDiagnostic::configuration)?;
        let mut configuration = alloc::vec![ConfigurationEntry {
            key: "request".into(),
            value: ConfigurationValue::Structured(configured),
        }];
        configuration.extend(
            context
                .iter()
                .filter(|entry| entry.key != "provenance")
                .cloned(),
        );
        Ok(configuration)
    }
}
