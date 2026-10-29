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
    /// Recheck selected local Fores against this independently authenticated
    /// checked boundary. No graph clone, permission, caller or Line is created.
    pub fn check_selected_fore(
        &self,
        plan: &conduit_core::VerifiedPlanMetadata,
    ) -> Result<(), PlannerError> {
        let metadata = plan.preparation_metadata();
        if metadata.source_document_id != self.source.source_document_id
            || metadata.checked_plot_id != self.source.checked_plot_id
            || metadata.expanded_plot_id != self.source.expanded_plot_id
        {
            return Err(PlannerError::InvalidPlotIdentity(
                "indexed Fore Source scope".into(),
            ));
        }
        let mut limits = BTreeMap::new();
        let mut count = 0;
        for port in metadata.fragments.iter().flat_map(|f| &f.fore_ports) {
            let key = crate::ForeBoundaryKey {
                direction: port.direction,
                front_port_id: port.front_port_id.clone(),
                track: port.track,
            };
            let value = ConnectionQueueLimits {
                item_capacity: port.item_capacity,
                byte_capacity: port.byte_capacity,
            };
            if limits.insert(key, value).is_some_and(|old| old != value) {
                return Err(PlannerError::InvalidPlotIdentity(
                    "inconsistent Fore bounds".into(),
                ));
            }
            count += 1;
        }
        let mut matched = 0;
        crate::canonical::authoring::visit_fore(
            &self.front,
            &self.inputs,
            &self.outputs,
            &metadata.fragments,
            &limits,
            |index, expected| {
                if metadata.fragments[index]
                    .fore_ports
                    .iter()
                    .filter(|actual| **actual == expected)
                    .count()
                    != 1
                {
                    return Err(PlannerError::InvalidPlotIdentity(
                        "selected Fore differs from checked boundary".into(),
                    ));
                }
                matched += 1;
                Ok(())
            },
        )?;
        if matched != count {
            return Err(PlannerError::InvalidPlotIdentity(
                "selected Fore count".into(),
            ));
        }
        Ok(())
    }
}
