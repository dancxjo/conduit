//! Explicit supplied formant-profile realization of an exact inventory phone.
//! This retains source metadata and declarations; it infers no IPA or phonology.
use crate::{
    generated, inventory_admission::InventoryPhone, semantic, EnglishStress, SpeechPhoneInput,
    VoiceEvent, SOURCE_ID,
};
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum ProfileRefusal {
    Basis(NativeBindingRefusal),
    UnsupportedPhone,
    UnsupportedDefinitionFeatures,
    UnsupportedTokenFeatures,
    AmbiguousBinding,
    DefinitionSnapshot,
    UnsupportedStress(semantic::StressSpecification),
}

pub struct PreparedProfilePhone<'a, 'inventory, 'material> {
    source: &'a InventoryPhone<'inventory, 'material>,
    profile: &'a semantic::SpeechFormantVoiceProfile,
    binding: &'a semantic::SpeechFormantPhoneBinding,
    stress: &'a semantic::StressSpecification,
    basis: semantic::SpeechFormantProfileBasis,
    event: VoiceEvent,
}
impl<'a, 'inventory, 'material> PreparedProfilePhone<'a, 'inventory, 'material> {
    pub fn source(&self) -> &'a InventoryPhone<'inventory, 'material> {
        self.source
    }
    pub fn profile(&self) -> &'a semantic::SpeechFormantVoiceProfile {
        self.profile
    }
    pub fn binding(&self) -> &'a semantic::SpeechFormantPhoneBinding {
        self.binding
    }
    pub fn stress(&self) -> &'a semantic::StressSpecification {
        self.stress
    }
    pub fn checked_basis(&self) -> &semantic::SpeechFormantProfileBasis {
        &self.basis
    }
    pub fn event(&self) -> VoiceEvent {
        self.event
    }
    pub fn compiled_source_id(&self) -> &'static str {
        SOURCE_ID
    }
}

pub fn prepare_profile_phone<'a, 'inventory, 'material>(
    source: &'a InventoryPhone<'inventory, 'material>,
    profile: &'a semantic::SpeechFormantVoiceProfile,
    stress: &'a semantic::StressSpecification,
) -> Result<PreparedProfilePhone<'a, 'inventory, 'material>, ProfileRefusal> {
    let basis = semantic::SpeechFormantProfileBasis::new(
        source.inventory().identity().clone(),
        source.inventory().language().clone(),
        profile.inventory_id().clone(),
        profile.language().clone(),
    )
    .map_err(ProfileRefusal::Basis)?;
    let mut matches = profile
        .phones()
        .as_slice()
        .iter()
        .filter(|binding| binding.definition().identity() == source.definition().identity());
    let binding = matches.next().ok_or(ProfileRefusal::UnsupportedPhone)?;
    if matches.next().is_some() {
        return Err(ProfileRefusal::AmbiguousBinding);
    }
    // A declared relation names a complete definition, not only a reusable ID.
    if binding.definition() != source.definition() {
        return Err(ProfileRefusal::DefinitionSnapshot);
    }
    // Feature-to-acoustics lowering is unsupported by this exact compact profile.
    // Do not silently discard supplied feature constraints when realizing a phone.
    if !source.definition().features().get().as_slice().is_empty() {
        return Err(ProfileRefusal::UnsupportedDefinitionFeatures);
    }
    if !source
        .material()
        .token()
        .features()
        .get()
        .as_slice()
        .is_empty()
    {
        return Err(ProfileRefusal::UnsupportedTokenFeatures);
    }
    let compact_stress = match stress {
        semantic::StressSpecification::Known(value) => match value {
            semantic::SpeechStress::Primary => EnglishStress::primary,
            semantic::SpeechStress::Secondary => EnglishStress::secondary,
            semantic::SpeechStress::Unstressed => EnglishStress::unstressed,
            semantic::SpeechStress::Reduced => EnglishStress::reduced,
        },
        semantic::StressSpecification::Unknown => EnglishStress::unknown,
        semantic::StressSpecification::Unspecified => EnglishStress::unspecified,
        state => return Err(ProfileRefusal::UnsupportedStress(state.clone())),
    };
    let event = VoiceEvent::phone(SpeechPhoneInput {
        phone: generated::compact_profile_phone(binding.phone()),
        stress: compact_stress,
    });
    Ok(PreparedProfilePhone {
        source,
        profile,
        binding,
        stress,
        basis,
        event,
    })
}
