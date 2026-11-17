//! Authenticated presentation replacement; application lifecycle is unchanged.
use super::*;

impl Owner {
    pub(crate) fn select_native_component_mask(
        &mut self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        receipt: &PortableAdmissionReceipt,
        look: conduit_face::ComponentMaskLook,
        recipe: conduit_face::TodoComponentRecipe,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        self.validate_native_mask_show(request, show)?;
        let previous_route = self
            .pending_native_mask
            .clone()
            .ok_or("native-mask-route-not-selected")?;
        if previous_route.credential != receipt.credential
            || previous_route.host_offer != receipt.host_advertisement
        {
            return Err("native-mask-route-credential-mismatch".into());
        }
        let previous_wardrobe = self.face_wardrobe.clone();
        let result = self
            .seal_native_mask_route_with_selection(
                receipt,
                &previous_route.face_line,
                &previous_route.return_line,
                previous_route.expires_at_millis,
                SelectedRemoteMask::Native(Some(NativeComponentSelection::Todo(look, recipe))),
            )
            .and_then(|route| {
                self.select_component_face_route(&route)?;
                Ok(route)
            });
        if result.is_err() {
            self.pending_native_mask = Some(previous_route);
            self.face_wardrobe = previous_wardrobe;
        }
        result
    }

    /// Explicit borrowed prepared contexts are required for every ambient reseal.
    /// The route stores only the selection; it cannot manufacture preparation.
    pub(crate) fn seal_native_ambient_mask_route(
        &mut self,
        receipt: &PortableAdmissionReceipt,
        face_line: &LineOffer,
        return_line: &LineOffer,
        expires_at_millis: u64,
        encounter: conduit_face::AmbientClockEncounter,
        prepared: &PreparedNativeContexts,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        self.seal_native_mask_route_with_selection(
            receipt,
            face_line,
            return_line,
            expires_at_millis,
            SelectedRemoteMask::Ambient(prepared, encounter),
        )
    }

    pub(crate) fn select_native_ambient_mask(
        &mut self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        receipt: &PortableAdmissionReceipt,
        encounter: conduit_face::AmbientClockEncounter,
        prepared: &PreparedNativeContexts,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        self.validate_native_mask_show(request, show)?;
        let previous_route = self
            .pending_native_mask
            .clone()
            .ok_or("native-mask-route-not-selected")?;
        if previous_route.credential != receipt.credential
            || previous_route.host_offer != receipt.host_advertisement
        {
            return Err("native-mask-route-credential-mismatch".into());
        }
        let previous_wardrobe = self.face_wardrobe.clone();
        let result = self
            .seal_native_ambient_mask_route(
                receipt,
                &previous_route.face_line,
                &previous_route.return_line,
                previous_route.expires_at_millis,
                encounter,
                prepared,
            )
            .and_then(|route| {
                self.select_component_face_route(&route)?;
                Ok(route)
            });
        if result.is_err() {
            self.pending_native_mask = Some(previous_route);
            self.face_wardrobe = previous_wardrobe;
        }
        result
    }
}
