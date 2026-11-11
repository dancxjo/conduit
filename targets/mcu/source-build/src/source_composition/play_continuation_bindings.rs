// One backing slot per actual binding, independent of recipe stream repetition.
pub(super) fn distinct_fields(recipe: &[conduit_core::FragmentSealPiece<'_>]) -> usize {
    use conduit_core::FragmentSealPiece::Field;
    recipe
        .iter()
        .enumerate()
        .filter(|(index, piece)| {
            let Field(field, _) = piece else {
                return false;
            };
            !recipe[..*index]
                .iter()
                .any(|earlier| matches!(earlier, Field(previous, _) if previous == field))
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        FragmentScopeRole as Role, FragmentSealBinding as Binding, FragmentSealField as Field,
        FragmentSealPiece as Piece, FragmentSealValue as Value,
    };
    #[test]
    fn repeated_fields_and_literals_keep_the_same_canonical_binding_values() {
        let host = Field {
            placement: None,
            role: Role::Host,
            entry: 0,
        };
        let boot = Field {
            placement: None,
            role: Role::Boot,
            entry: 0,
        };
        let recipe = [
            Piece::Literal(b"prefix"),
            Piece::Field(host, true),
            Piece::Field(boot, true),
            Piece::Literal(b"middle"),
            Piece::Field(host, true),
            Piece::Field(boot, true),
            Piece::Literal(b"suffix"),
        ];
        let reference = [
            Binding {
                field: host,
                value: Value::Text("actual-host"),
            },
            Binding {
                field: boot,
                value: Value::Text("actual-boot"),
            },
        ];
        let mut bindings = Vec::with_capacity(distinct_fields(&recipe));
        for piece in &recipe {
            let Piece::Field(field, _) = piece else {
                continue;
            };
            if !bindings
                .iter()
                .any(|binding: &Binding<'_>| binding.field == *field)
            {
                bindings.push(
                    *reference
                        .iter()
                        .find(|binding| binding.field == *field)
                        .unwrap(),
                );
            }
        }
        assert_eq!(distinct_fields(&recipe), 2);
        assert_eq!(bindings.len(), bindings.capacity());
        assert!(bindings.capacity() < recipe.len());
        assert_eq!(
            conduit_core::fragment_seal_recipe_digest(&recipe, &bindings).unwrap(),
            conduit_core::fragment_seal_recipe_digest(&recipe, &reference).unwrap()
        );
        assert_eq!(distinct_fields(&[Piece::Literal(b"only literal")]), 0);
    }
}
