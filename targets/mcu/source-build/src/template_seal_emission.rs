//! Emit immutable representation of Core's canonical stream, never permissions.
use conduit_core::{FragmentSealPiece, FragmentSealRecipe};
use std::fmt::Write;
pub(super) fn emit(recipe: &FragmentSealRecipe) -> String {
    let mut text = String::new();
    text.push_str("// Canonical identity bytes only; actual scope, lawful sealing and admission remain mandatory.\npub static FRAGMENT_SEAL_RECIPE:&[conduit_core::FragmentSealPiece<'static>]=&[\n");
    for piece in recipe.pieces() {
        match piece {
            FragmentSealPiece::Literal(bytes) => {
                writeln!(
                    text,
                    "conduit_core::FragmentSealPiece::Literal(&{bytes:?}),"
                )
                .unwrap();
            }
            FragmentSealPiece::Field(field, textual) => {
                writeln!(text,"conduit_core::FragmentSealPiece::Field(conduit_core::FragmentSealField{{placement:{:?},role:conduit_core::FragmentScopeRole::{:?},entry:{}}},{}),",field.placement,field.role,field.entry,textual).unwrap();
            }
        }
    }
    text.push_str("];\n");
    text
}
