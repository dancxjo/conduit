//! Trusted checked Fore material for indexed Source. No Host, resource, Line or
//! runtime permission is created by decoding or retaining this description.
use super::*;
use conduit_core::{CheckedFront, PlotIdentity};
use conduit_plot::AuthoringFrontBinding;

pub struct IndexedAuthoringBoundary {
    source: PlotIdentity,
    front: CheckedFront,
    inputs: Vec<AuthoringFrontBinding>,
    outputs: Vec<AuthoringFrontBinding>,
}
impl IndexedAuthoringBoundary {
    /// Retain the checked Fore projection from the same authenticated Make
    /// artifact as its indexed graph. Ordinary admission still checks each
    /// selected Back, exact value contract, track and finite queue capacity.
    ///
    /// # Safety
    /// Root must authenticate all fields against the independently checked
    /// authoring Source and validated boundary provenance. Received descriptors
    /// or a claimed Source identity cannot establish that origin. Origin spans
    /// may be omitted from this projection: they do not affect Fore realization.
    pub unsafe fn from_make(
        source: PlotIdentity,
        front: CheckedFront,
        inputs: Vec<AuthoringFrontBinding>,
        outputs: Vec<AuthoringFrontBinding>,
    ) -> Result<Self, PlannerError> {
        if inputs.len() > 32 || outputs.len() > 32 {
            return Err(PlannerError::InvalidPlotIdentity(
                "indexed Fore capacity".into(),
            ));
        }
        Ok(Self {
            source,
            front,
            inputs,
            outputs,
        })
    }
    /// Inspect declared bindings to admit exact caller-owned queue bounds.
    /// This does not grant access to the resulting Fore.
    pub fn bindings(
        &self,
    ) -> impl Iterator<
        Item = (
            conduit_core::PortDirection,
            &conduit_core::PortId,
            conduit_core::ConnectionTrack,
        ),
    > {
        self.inputs
            .iter()
            .map(|b| {
                (
                    conduit_core::PortDirection::Input,
                    &b.front_port_id,
                    b.track,
                )
            })
            .chain(self.outputs.iter().map(|b| {
                (
                    conduit_core::PortDirection::Output,
                    &b.front_port_id,
                    b.track,
                )
            }))
    }
    pub(super) fn attach(
        &self,
        identity: &PlotIdentity,
        fragments: &mut [conduit_core::PlanFragment],
        limits: &BTreeMap<crate::ForeBoundaryKey, ConnectionQueueLimits>,
    ) -> Result<(), PlannerError> {
        if &self.source != identity {
            return Err(PlannerError::InvalidPlotIdentity(
                "indexed Fore Source scope".into(),
            ));
        }
        crate::canonical::authoring::attach_fore(
            &self.front,
            &self.inputs,
            &self.outputs,
            fragments,
            limits,
        )
    }
}
