//! Emit immutable representation of Core's canonical stream, never permissions.
use conduit_core::{FragmentSealPiece, FragmentSealRecipe};
use std::fmt::Write;
pub(super) fn emit(recipe: &FragmentSealRecipe) -> String {
    let mut text = String::new();
    text.push_str("// Canonical identity bytes only; actual scope, lawful sealing and admission remain mandatory.\npub static FRAGMENT_SEAL_RECIPE:&[conduit_core::FragmentSealPiece<'static>]=&[\n");
    for piece in recipe.pieces() {
        match piece {
            FragmentSealPiece::Literal(bytes) => {
                text.push_str("conduit_core::FragmentSealPiece::Literal(");
                byte_literal(&mut text, bytes);
                text.push_str("),\n");
            }
            FragmentSealPiece::Field(field, textual) => {
                writeln!(text,"conduit_core::FragmentSealPiece::Field(conduit_core::FragmentSealField{{placement:{:?},role:conduit_core::FragmentScopeRole::{:?},entry:{}}},{}),",field.placement,field.role,field.entry,textual).unwrap();
            }
        }
    }
    text.push_str("];\n");
    text
}

// Preserve every canonical byte and the original piece boundaries. A byte
// string avoids decimal-array expansion in the build artifact; it changes no
// recipe identity, flash payload, authority check or runtime representation.
fn byte_literal(text: &mut String, bytes: &[u8]) {
    text.push_str("b\"");
    for byte in bytes {
        for escaped in std::ascii::escape_default(*byte) {
            text.push(char::from(escaped));
        }
    }
    text.push('"');
}

#[cfg(test)]
#[path = "template_seal_emission_tests.rs"]
mod tests;
