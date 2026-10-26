//! Plan-free Show construction after complete ordinary Mask Plan preparation.
//! A context holds immutable full wire plus only the selected identity fields
//! required by the existing manifestation/correlation rules. No output authority.
use crate::{
    manifestation::PreparedPresenterBasis,
    mask_show::{bind_show_identity, validate_basis},
    mask_show_json::MaskShowPrefix,
    Face, FaceContentId, ManifestationFailure, ManifestationLifecycle, MaskJsonRefusal,
    MaskShowError, MaskShowId, PlannedMaskPlot, PreparedMaskShowJson, Show,
};
use alloc::{boxed::Box, string::String};
use conduit_core::{ActivePlayIdentity, PlanId, PlotIdentity, SignId};
pub struct PreparedMaskShowContext {
    plan: Box<[u8]>,
    mask: PlotIdentity,
    presenter: PreparedPresenterBasis,
}
impl PreparedMaskShowContext {
    /// Dictionary/history and encoder state for complete lossless Plan encoding.
    /// Output capacity, one supplied placement/configuration, allocator overhead
    /// and caller stack remain separate cold preparation admission obligations.
    pub const fn codec_workspace_bytes() -> usize {
        crate::mask_plot::wire::CODEC_WORKSPACE_BYTES
    }

    /// Cold preparation verifies the complete Plan and retains its lossless wire.
    /// The rich Plan may be dropped afterward. This does not admit current Host,
    /// Face, provider, Body route or actual delivery.
    pub fn prepare(
        planned: &PlannedMaskPlot,
        maximum_plan_bytes: usize,
    ) -> Result<Self, MaskJsonRefusal> {
        planned
            .validate()
            .map_err(|_| MaskJsonRefusal::InvalidPlan)?;
        let plan = crate::mask_plot::wire::json_mask_wire_bounded(
            &planned.mask,
            &planned.plan.plan_id,
            &planned.plan,
            maximum_plan_bytes,
        )?;
        if plan.is_empty() || plan.len() > maximum_plan_bytes {
            return Err(MaskJsonRefusal::Capacity);
        }
        let plan = plan.into_boxed_slice();
        Ok(Self {
            plan,
            mask: planned.mask.plot_identity.clone(),
            presenter: PreparedPresenterBasis::from_admitted_mask(planned),
        })
    }
    /// Prepare the same complete lossless context from opaque Core verification.
    /// Configurations are supplied and checked one placement at a time; no whole
    /// rich Mask Plan is rebuilt. Bounded codec workspace, output backing and
    /// caller stack still require separate finite preparation admission.
    /// This grants no current Host/Face/provider/Body route or delivery authority.
    pub fn prepare_verified<F, E>(
        mask: &crate::MaskPlot,
        plan: &conduit_core::VerifiedPlanMetadata,
        configuration: F,
        maximum_plan_bytes: usize,
    ) -> Result<Self, MaskJsonRefusal>
    where
        F: FnMut(
            &conduit_core::PlannedGear,
        ) -> Result<alloc::vec::Vec<conduit_core::ConfigurationEntry>, E>,
    {
        Self::prepare_verified_with_workspace(
            mask,
            plan,
            configuration,
            maximum_plan_bytes,
            &mut [],
        )
    }
    /// Borrow exclusively owned cold scratch. No Value handle or authority is
    /// created. The caller must keep these empty stores unavailable to kernels
    /// and wipe scratch after success/refusal; both passes reuse the same bytes.
    pub fn prepare_verified_with_workspace<F, E>(
        mask: &crate::MaskPlot,
        plan: &conduit_core::VerifiedPlanMetadata,
        configuration: F,
        maximum_plan_bytes: usize,
        workspace: &mut [&mut [u8]],
    ) -> Result<Self, MaskJsonRefusal>
    where
        F: FnMut(
            &conduit_core::PlannedGear,
        ) -> Result<alloc::vec::Vec<conduit_core::ConfigurationEntry>, E>,
    {
        if maximum_plan_bytes == 0 {
            return Err(MaskJsonRefusal::Capacity);
        }
        crate::mask_plot::validate_verified_mask(mask, plan)
            .map_err(|_| MaskJsonRefusal::InvalidPlan)?;
        let placement = plan
            .preparation_metadata()
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .find(|placement| placement.gear_id == mask.show_output.gear_id)
            .ok_or(MaskJsonRefusal::InvalidPlan)?;
        let view = plan.serialize_with_configurations(configuration);
        let wire = crate::mask_plot::wire::json_mask_wire_bounded_with_workspace(
            mask,
            plan.plan_id(),
            &view,
            maximum_plan_bytes,
            workspace,
        )?;
        if wire.is_empty() || wire.len() > maximum_plan_bytes {
            return Err(MaskJsonRefusal::Capacity);
        }
        Ok(Self {
            plan: wire.into_boxed_slice(),
            mask: mask.plot_identity.clone(),
            presenter: PreparedPresenterBasis::from_admitted_placement(plan.plan_id(), placement),
        })
    }
    /// Bounded cold receipt of complete lossless Plan wire. Checks the expanded
    /// extent before decompression, then validates the complete decoded Plan.
    /// Keeps the accepted wire verbatim; it never recompresses or retains the
    /// rich model. Its temporary heap/stack still needs independent admission.
    pub fn from_wire(
        wire: Box<[u8]>,
        maximum_wire_bytes: usize,
        maximum_expanded_bytes: usize,
    ) -> Result<Self, MaskJsonRefusal> {
        Self::from_wire_checked(wire, maximum_wire_bytes, maximum_expanded_bytes, |_| true)
    }
    /// Cold receipt with additional independent installation checks before rich
    /// preparation is dropped. Passing this callback is not Body/effect authority.
    pub fn from_wire_checked(
        wire: Box<[u8]>,
        maximum_wire_bytes: usize,
        maximum_expanded_bytes: usize,
        check: impl FnOnce(&PlannedMaskPlot) -> bool,
    ) -> Result<Self, MaskJsonRefusal> {
        if wire.is_empty() || wire.len() > maximum_wire_bytes || maximum_expanded_bytes == 0 {
            return Err(MaskJsonRefusal::Capacity);
        }
        #[derive(serde::Deserialize)]
        struct Header {
            plan: Extent,
        }
        #[derive(serde::Deserialize)]
        struct Extent {
            expanded_bytes: usize,
        }
        let header: Header = serde_json::from_slice(&wire).map_err(|_| MaskJsonRefusal::Json)?;
        if header.plan.expanded_bytes == 0 || header.plan.expanded_bytes > maximum_expanded_bytes {
            return Err(MaskJsonRefusal::Capacity);
        }
        let planned: PlannedMaskPlot =
            serde_json::from_slice(&wire).map_err(|_| MaskJsonRefusal::Json)?;
        planned
            .validate()
            .map_err(|_| MaskJsonRefusal::InvalidPlan)?;
        if !check(&planned) {
            return Err(MaskJsonRefusal::InvalidPlan);
        }
        let plan = wire;
        Ok(Self {
            plan,
            mask: planned.mask.plot_identity.clone(),
            presenter: PreparedPresenterBasis::from_admitted_mask(&planned),
        })
    }
    pub fn plan_wire(&self) -> &[u8] {
        &self.plan
    }
    pub fn plan_id(&self) -> &PlanId {
        self.presenter.plan_id()
    }
    pub fn prepared_show(
        &self,
        face: &Face,
        active: ActivePlayIdentity,
        front: String,
        target: String,
        sign: SignId,
    ) -> Result<PreparedMaskOccurrence<'_>, MaskShowError> {
        let (face_plot, face_plan_id) = validate_basis(face)?;
        let show = self
            .presenter
            .prepare(face, active, front, target, sign)
            .map_err(MaskShowError::InvalidManifestation)?;
        let show_id = bind_show_identity(&self.mask, self.plan_id(), face, &show);
        Ok(PreparedMaskOccurrence {
            context: self,
            show_id,
            face_plot,
            face_plan_id,
            face_id: face.identity.clone(),
            face_revision: face.revision,
            show,
        })
    }
}
/// Exact immutable basis with controlled manifestation transitions. It borrows
/// only its prepared context, never the Face or the complete rich Plan model.
/// The context borrow prevents pairing this occurrence with another cached Plan.
pub struct PreparedMaskOccurrence<'a> {
    context: &'a PreparedMaskShowContext,
    show_id: MaskShowId,
    face_plot: Option<PlotIdentity>,
    face_plan_id: Option<PlanId>,
    face_id: FaceContentId,
    face_revision: u64,
    show: Show,
}
impl<'a> PreparedMaskOccurrence<'a> {
    pub fn show(&self) -> &Show {
        &self.show
    }
    /// Same semantic lifecycle rule as MaskShow; a metadata transition never
    /// establishes actual availability, effect acceptance or delivery authority.
    pub fn transition(
        &mut self,
        lifecycle: ManifestationLifecycle,
        sign: SignId,
    ) -> Result<(), MaskShowError> {
        let next = self
            .show
            .transition(lifecycle, sign)
            .map_err(MaskShowError::InvalidManifestation)?;
        self.show = next;
        Ok(())
    }
    pub fn fail(
        &mut self,
        failure: ManifestationFailure,
        sign: SignId,
    ) -> Result<(), MaskShowError> {
        let next = self
            .show
            .fail(failure, sign)
            .map_err(MaskShowError::InvalidManifestation)?;
        self.show = next;
        Ok(())
    }
    pub fn validate_face(&self, face: &Face) -> Result<(), MaskShowError> {
        let (plot, plan) = validate_basis(face)?;
        if self.face_id != face.identity
            || self.face_revision != face.revision
            || self.face_plot != plot
            || self.face_plan_id != plan
        {
            return Err(MaskShowError::StaleFace);
        }
        Ok(())
    }
    /// Encode directly into admitted value backing. `serialize` must implement
    /// the same JSON representation as the canonical serializer for each part;
    /// the Host supplies its existing bounded codec (for example JSON-core).
    /// On refusal output may be partial: discard it, never publish it. The
    /// kernel's encoded-value reservation supplies that transactional boundary.
    pub fn encode_json_with(
        &self,
        output: &mut [u8],
        serialize: impl FnMut(crate::MaskJsonPart<'_>, &mut [u8]) -> Result<usize, MaskJsonRefusal>,
    ) -> Result<usize, MaskJsonRefusal> {
        crate::mask_show_encoding::encode(
            MaskShowPrefix {
                show_id: &self.show_id,
                mask_plot: &self.context.mask,
                face_plot: &self.face_plot,
                face_plan_id: &self.face_plan_id,
                face_id: &self.face_id,
                face_revision: self.face_revision,
            },
            &self.context.plan,
            &self.show,
            output,
            serialize,
        )
    }
    /// Snapshot-only representation. The caller still owns current Face/custody
    /// admission before an effect; historical serialization remains possible.
    pub fn json(&self, maximum_bytes: usize) -> Result<PreparedMaskShowJson<'a>, MaskJsonRefusal> {
        crate::mask_show_json::bind_wire_parts(
            &self.context.plan,
            MaskShowPrefix {
                show_id: &self.show_id,
                mask_plot: &self.context.mask,
                face_plot: &self.face_plot,
                face_plan_id: &self.face_plan_id,
                face_id: &self.face_id,
                face_revision: self.face_revision,
            },
            &self.show,
            maximum_bytes,
        )
    }
}
