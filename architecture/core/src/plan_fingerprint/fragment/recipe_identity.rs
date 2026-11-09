//! Stable identity of inert recipe structure, distinct from a sealed fragment.
use super::FragmentSealPiece;
use crate::FragmentScopeRole;
use sha2::{Digest, Sha256};

/// Commits literal bytes, typed substitution slots and their order. This is
/// construction identity only: no supplied bindings, current scope or authority
/// are checked, and the result cannot substitute for a fragment commitment.
pub fn fragment_seal_recipe_identity<'a>(
    pieces: impl IntoIterator<Item = FragmentSealPiece<'a>>,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.core/fragment-seal-recipe@1\0");
    let mut count = 0u64;
    for piece in pieces {
        count += 1;
        match piece {
            FragmentSealPiece::Literal(bytes) => {
                hash.update([0]);
                hash.update((bytes.len() as u64).to_le_bytes());
                hash.update(bytes);
            }
            FragmentSealPiece::Field(field, textual) => {
                hash.update([1, u8::from(textual)]);
                match field.placement {
                    None => hash.update([0]),
                    Some(ordinal) => {
                        hash.update([1]);
                        hash.update(ordinal.to_le_bytes());
                    }
                }
                let role = match field.role {
                    FragmentScopeRole::Host => 0,
                    FragmentScopeRole::Boot => 1,
                    FragmentScopeRole::OfferGeneration => 2,
                    FragmentScopeRole::PlacementId => 3,
                    FragmentScopeRole::ProviderInstance => 4,
                    FragmentScopeRole::ProviderGeneration => 5,
                    FragmentScopeRole::AuthorityGrant => 6,
                    FragmentScopeRole::AuthorityHost => 7,
                    FragmentScopeRole::AuthorityBoot => 8,
                    FragmentScopeRole::ResourcePool => 9,
                    FragmentScopeRole::PoolReference => 10,
                    FragmentScopeRole::ContentOwnerHost => 11,
                    FragmentScopeRole::ContentOwnerBoot => 12,
                };
                hash.update([role]);
                hash.update(field.entry.to_le_bytes());
            }
        }
    }
    hash.update(count.to_le_bytes());
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FragmentSealField;
    #[test]
    fn binds_literal_boundaries_and_typed_slot_schema() {
        let id = |pieces: &[FragmentSealPiece<'_>]| {
            fragment_seal_recipe_identity(pieces.iter().copied())
        };
        assert_ne!(
            id(&[FragmentSealPiece::Literal(b"ab")]),
            id(&[
                FragmentSealPiece::Literal(b"a"),
                FragmentSealPiece::Literal(b"b")
            ])
        );
        let field = FragmentSealField {
            placement: Some(0),
            role: FragmentScopeRole::Boot,
            entry: 0,
        };
        assert_ne!(
            id(&[FragmentSealPiece::Field(field, true)]),
            id(&[FragmentSealPiece::Field(field, false)])
        );
        assert_ne!(
            id(&[FragmentSealPiece::Field(field, true)]),
            id(&[FragmentSealPiece::Field(
                FragmentSealField {
                    role: FragmentScopeRole::Host,
                    ..field
                },
                true
            )])
        );
        assert_ne!(
            id(&[FragmentSealPiece::Field(field, true)]),
            id(&[FragmentSealPiece::Field(
                FragmentSealField {
                    placement: None,
                    ..field
                },
                true
            )])
        );
    }
}
