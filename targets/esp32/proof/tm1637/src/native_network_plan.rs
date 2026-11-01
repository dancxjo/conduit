//! Retire current offers before canonical numeric lowering.
use super::{c, NativeRoot};
impl NativeRoot {
    /// A prepared Plan remains uninstalled; admission and kernel storage are
    /// separate. Each call replaces preparation against this same actual Boot.
    pub fn prepare_network_plan(&mut self) -> Result<(), ()> {
        self.network_preparation = None;
        let host = self.network_advertisement()?;
        let local_bases = [c::BaseImplementationId::from("conduit.base/local@1")];
        let grants = match (self.network.ready(), self.credentials.as_ref()) {
            (Some(join), Some(source)) => Some([
                self.issuer.grant(
                    source.native_identity(),
                    crate::native_authority::Operation::Credentials,
                )?,
                self.issuer.grant(
                    join.native_identity(),
                    crate::native_authority::Operation::Join,
                )?,
            ]),
            _ => None,
        };
        let source = crate::native_source::load();
        let plan = crate::network_planning::PreparedVerifiedNetworkSetup::from_indexed_for_identity(
            &self.network_source,
            &source,
            host,
            &local_bases,
            grants.as_ref().map_or(&[], |v| v.as_slice()),
        );
        drop(grants);
        self.network_preparation = Some(plan);
        Ok(())
    }
}
