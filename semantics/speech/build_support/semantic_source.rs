//! One declaration source for generated bindings and the public authoring catalog.
extern crate alloc;
use alloc::string::String;

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
    ]
    .join("\n")
}
