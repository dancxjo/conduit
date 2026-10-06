//! USB exchange contracts available to checked Source, without native offers.
use super::*;

pub(super) fn install(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), ProtocolSourceRefusal> {
    use crate::usb_base::control_contract::{CONTROL_KIND, ControlContract};
    use ProtocolSourceRefusal as Error;
    let control = ControlContract::prepare().map_err(Error::Contract)?;
    for (path, schema) in [
        ("machine/usb/control/request", control.request_type()),
        ("machine/usb/control/result", control.result_type()),
    ] {
        startup
            .insert_structured_type(path, schema.clone())
            .map_err(Error::Catalog)?;
    }
    startup
        .insert(conduit_plot::KindSignature {
            kind: CONTROL_KIND.into(),
            startup_parameters: vec![],
        })
        .map_err(Error::Catalog)?;
    startup
        .insert_fore(CONTROL_KIND, control.kind().checked_front())
        .map_err(Error::Catalog)?;
    profile
        .insert_kind(control.kind().clone())
        .map_err(|error| Error::Catalog(alloc::format!("{error:?}")))?;
    crate::usb_base::endpoint_read_contract::EndpointReadContract::prepare()
        .map_err(Error::Contract)?
        .install_catalogs(startup, profile)
        .map_err(Error::Catalog)
}
