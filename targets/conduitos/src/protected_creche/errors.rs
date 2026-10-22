use super::TransportError;
use conduit_birth_plot::{BirthDraftRefusal as E, names::NamingRefusal as N};
use conduitos_creche_core::wire::Refusal as R;
pub(super) fn draft(error: TransportError) -> E {
    match error {
        TransportError::Refused(error) => match error {
            R::NamingCatalog => E::Naming(N::Catalog),
            R::NamingUuid => E::Naming(N::Uuid),
            R::NamingSystem => E::Naming(N::UnknownSystem),
            R::NamingBounds => E::Naming(N::Bounds),
            R::InvalidName => E::InvalidName,
            R::InvalidInventory => E::InvalidInventory,
            R::EmptySelection => E::EmptySelection,
            R::UnavailablePlot => E::UnavailablePlot,
            R::StaleFace => E::StaleFace,
            R::RevisionExhausted => E::RevisionExhausted,
            R::UnknownChoice => E::UnknownChoice,
            R::UnknownAction => E::UnknownAction,
            R::InvalidActionValue => E::InvalidActionValue,
            R::InvalidSearch => E::InvalidSearch,
            _ => E::StaleFace,
        },
        _ => E::StaleFace,
    }
}
