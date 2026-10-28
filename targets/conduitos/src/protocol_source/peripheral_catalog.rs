//! Inert peripheral definitions for ordinary Source checking; no native offers.
use super::*;
pub(super) fn install(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), ProtocolSourceRefusal> {
    crate::spi_base::contract::SpiContract::prepare()
        .map_err(ProtocolSourceRefusal::Contract)?
        .install_catalogs(startup, profile)
        .map_err(ProtocolSourceRefusal::Catalog)?;
    crate::digital_base::contract::DigitalContract::prepare()
        .map_err(ProtocolSourceRefusal::Contract)?
        .install_catalogs(startup, profile)
        .map_err(ProtocolSourceRefusal::Catalog)
}
