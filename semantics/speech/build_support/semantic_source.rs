//! One declaration source for generated bindings and the public authoring catalog.
extern crate alloc;
use alloc::string::String;

/// Build-time Native generation and public authoring import one owner contract.
pub fn language_identities() -> Result<conduit_plot::CheckedSyntaxDocument, String> {
    conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(include_str!("../../language/identity.conduit")),
        &conduit_plot::StartupCatalog::new(),
    )
    .map_err(|error| alloc::format!("{error:?}"))
}

pub fn source() -> String {
    [
        include_str!("../types.conduit"),
        include_str!("../rule_status.conduit"),
        include_str!("../selection.conduit"),
        include_str!("../listening.conduit"),
        include_str!("../translation.conduit"),
        include_str!("../timing.conduit"),
        include_str!("../intent.conduit"),
        include_str!("../inventory.conduit"),
        include_str!("../profile_phones.conduit"),
        include_str!("../voice_profile.conduit"),
        include_str!("../context_match.conduit"),
        include_str!("../linguistic_prosody.conduit"),
        include_str!("../pitch_trajectory.conduit"),
        include_str!("../ipa.conduit"),
        include_str!("../ipa_syntax.conduit"),
        include_str!("../ipa_inventory.conduit"),
        include_str!("../ipa_constructors.conduit"),
    ]
    .join("\n")
}
