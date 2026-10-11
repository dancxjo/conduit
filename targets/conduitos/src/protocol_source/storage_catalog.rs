//! Reviewed storage semantics only; no initialized provider or authority.
use super::*;
pub(super) fn install(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), ProtocolSourceRefusal> {
    use ProtocolSourceRefusal as E;
    let contract =
        crate::storage_base::transaction_contract::BlockReadTransactionContract::prepare()
            .map_err(E::Contract)?;
    contract
        .block()
        .install_catalogs(startup, profile)
        .map_err(E::Catalog)?;
    startup
        .insert_structured_type("storage/block-read/permit", contract.permit_type().clone())
        .map_err(E::Catalog)?;
    for kind in [contract.authorize_kind(), contract.settle_kind()] {
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .map_err(E::Catalog)?;
        startup
            .insert_fore(kind.kind_id.as_str(), kind.checked_front())
            .map_err(E::Catalog)?;
        profile
            .insert_kind(kind.clone())
            .map_err(|error| E::Catalog(alloc::format!("{error:?}")))?;
    }
    Ok(())
}
