//! Original-counter custody for acknowledged Show descendants and validation.
use super::*;
use std::mem::size_of;

impl Owner {
    pub(in crate::durable_host::owner::controller) fn acknowledge_image_native_show(
        &mut self,
        request: &conduit_face::OwnerFaceSnapshotRequest,
        show: &conduit_face::MaskShow,
    ) -> Result<(), String> {
        let memory = self.host.root_preparation()?.clone();
        let route = self
            .pending_native_mask
            .as_ref()
            .ok_or("native-mask-route-not-selected")?;
        memory.verify_retention(
            route
                .memory
                .as_ref()
                .ok_or("native view lacks original IMAGE custody")?,
        )?;
        let wardrobe = self
            .face_wardrobe
            .as_ref()
            .ok_or("owner face wardrobe is not admitted")?;
        let show_owner = size_of::<conduit_face::MaskShow>()
            .checked_add(
                super::super::backing::show_payload_bytes(show)
                    .ok_or("unsupported Show backing")?,
            )
            .ok_or("Show backing overflow")?;
        let seal_clone = size_of::<RemoteOwnerMaskRouteSeal>()
            .checked_add(
                super::super::backing::seal_payload_bytes(&route.seal)
                    .ok_or("unsupported route backing")?,
            )
            .ok_or("Show backing overflow")?;
        let wardrobe_clone = size_of::<OwnerFaceWardrobe>()
            .checked_add(
                wardrobe
                    .retained_backing_bytes()
                    .ok_or("unsupported wardrobe backing")?,
            )
            .ok_or("Show backing overflow")?;
        let mut bytes = 0usize;
        // Two stored Show clones (route and wardrobe), plus selected_show's
        // returned validation clone. The previous acknowledgement remains
        // funded by its old lease until replacement actually drops it.
        for named in [
            show_owner,
            show_owner,
            show_owner,
            seal_clone,
            wardrobe_clone,
            super::super::backing::native_verification_workspace(&show.planned_mask)
                .ok_or("unsupported Show verifier")?,
            show.planned_mask
                .json_workspace()
                .ok_or("unsupported Show codec")?
                .codec_bytes,
            route.return_line.binding.limits.maximum_payload_bytes as usize,
            conduit_face::OwnerFacePlan::identity_encoding_limit(),
            route.seal.route_plan_id.0.capacity(),
        ] {
            bytes = bytes.checked_add(named).ok_or("Show backing overflow")?;
        }
        // SAFETY: named actual Show owners, ordinary reconciliation clones,
        // closed-plan verification and bounded codec/output scratch above.
        // Existing Face projection input retains its independent caller custody.
        let prepared = unsafe {
            memory.prepare_value(bytes, bytes, || {
                self.acknowledge_native_show_inner(request, show)
            })
        }?;
        let backing = prepared.retain_memory();
        self.pending_native_mask
            .as_mut()
            .ok_or("native-mask-route-not-selected")?
            .show_memory = Some(backing.clone());
        self.face_wardrobe
            .as_mut()
            .ok_or("owner face wardrobe is not admitted")?
            .retain_native_show_memory(backing);
        Ok(())
    }
}
