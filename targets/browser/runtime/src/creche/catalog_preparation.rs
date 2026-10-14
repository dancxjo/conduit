//! One preparation-local installed catalog; no user Source receipts or Host truth cached.
use crate::installed_browser::{catalogs_for_face, FaceProfile};
use conduit_plot::{CanonicalBackCatalog, ProfileCatalog, StartupCatalog};

#[derive(Default)]
pub(super) struct CatalogPreparation {
    current: Option<(
        FaceProfile,
        StartupCatalog,
        ProfileCatalog,
        Option<CanonicalBackCatalog>,
    )>,
}
impl CatalogPreparation {
    pub(super) fn get(
        &mut self,
        face: FaceProfile,
    ) -> Result<(&StartupCatalog, &ProfileCatalog), String> {
        if self.current.as_ref().is_none_or(|entry| entry.0 != face) {
            // Drop the previous profile before preparing a replacement. The
            // retained bound is one pair even for mixed face bundles.
            self.current = None;
            let (startup, profile) = catalogs_for_face(face)?;
            self.current = Some((face, startup, profile, None));
        }
        let (_, startup, profile, _) = self.current.as_ref().expect("prepared catalog");
        Ok((startup, profile))
    }

    pub(super) fn get_with_backs(
        &mut self,
        face: FaceProfile,
    ) -> Result<(&StartupCatalog, &ProfileCatalog, &CanonicalBackCatalog), String> {
        self.get(face)?;
        let (_, startup, profile, backs) = self.current.as_mut().expect("prepared catalog");
        if backs.is_none() {
            *backs = Some(crate::installed_browser::backs(startup, profile)?);
        }
        Ok((startup, profile, backs.as_ref().expect("prepared backs")))
    }
}
