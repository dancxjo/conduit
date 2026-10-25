//! Independently retained complete native presentation intent, before Plan.
use super::*;
use conduit_face::{ComponentMaskLook, LookInfo, PlannedMaskPlot, TodoComponentRecipe};
use conduit_plot::rust_binding::NativeRustBinding;

#[derive(Clone)]
pub(super) enum NativeSelection {
    Todo(ComponentMaskLook, TodoComponentRecipe),
    History(Box<LookInfo>),
}

pub(super) fn plan(
    host: &HostAdvertisement,
    face: &Face,
    selection: Option<&NativeSelection>,
) -> Result<PlannedMaskPlot, String> {
    if let Some(NativeSelection::History(requested)) = selection {
        let requested = requested.as_ref();
        // A typed field deserialized by a transport must still pass the exact
        // canonical Type/laws before it becomes retained startup intent.
        let bytes = requested
            .clone()
            .encode()
            .map_err(|e| format!("native-history-look-invalid:{e:?}"))?;
        if bytes.len() > conduit_face::MAX_LOOK_BYTES {
            return Err("native-history-look-too-large".into());
        }
        let look =
            LookInfo::decode(&bytes).map_err(|e| format!("native-history-look-invalid:{e:?}"))?;
        let (stage, configuration) =
            conduit_conduitos_mask_offer::prepare_history_component_stage_with_look_from_offer(
                host, &look,
            )
            .map_err(|e| format!("native-history-mask-plan-refused:{e:?}"))?;
        let recipe = conduit_core::StructuredInfoValue::from_canonical_bytes(
            configuration.recipe.canonical_value(),
        )
        .map_err(|e| format!("native-history-recipe-invalid:{e:?}"))?;
        // Selection does not create environmental facts. Absent or differently
        // typed history remains unavailable on this actual current owner Face.
        conduit_face::prepare_component_recipe(recipe, face)
            .map_err(|e| format!("native-history-face-unavailable:{e:?}"))?;
        return Ok(stage.planned_mask);
    }
    let todo = match selection {
        Some(NativeSelection::Todo(look, recipe)) => Some(((*look).into(), *recipe)),
        None => None,
        Some(NativeSelection::History(_)) => None, // History returned above.
    };
    conduit_conduitos_mask_offer::prepare_advertised_native_stage_from_offer(host, todo)
        .map(|stage| stage.planned_mask)
        .map_err(|e| format!("native-mask-plan-refused:{e:?}"))
}

impl Owner {
    /// Existing Todo API retains its current Show, credential and rollback gates.
    pub(crate) fn select_native_component_mask(
        &mut self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        receipt: &PortableAdmissionReceipt,
        look: ComponentMaskLook,
        recipe: TodoComponentRecipe,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        self.select_native_presentation(request, show, receipt, NativeSelection::Todo(look, recipe))
    }

    /// Select a complete independently supplied Look on this Body's current
    /// history Face. This does not retrieve, record or synthesize history.
    pub(crate) fn select_native_history_mask(
        &mut self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        receipt: &PortableAdmissionReceipt,
        look: LookInfo,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        self.select_native_presentation(
            request,
            show,
            receipt,
            NativeSelection::History(Box::new(look)),
        )
    }

    fn select_native_presentation(
        &mut self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        receipt: &PortableAdmissionReceipt,
        selection: NativeSelection,
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
                SelectedRemoteMask::Native(Some(selection)),
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

#[cfg(test)]
#[path = "selection/tests.rs"]
mod tests;
