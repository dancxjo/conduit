//! ConduitOS selection of the Tour-owned renderer-neutral workspace.

use conduit_presentation::{ApplicationView, SemanticPresentationRefusal};
use conduit_tour_model::{TourWorkspacePhase, TourWorkspaceState};

pub fn application_view(
    revision: u32,
    phase: TourWorkspacePhase,
) -> Result<ApplicationView, SemanticPresentationRefusal> {
    TourWorkspaceState::canonical(revision, phase)
        .presentation()?
        .lower()
}

#[cfg(test)]
mod tests {
    use conduit_presentation::ApplicationComponent;

    use super::*;

    #[test]
    fn conduitos_consumes_the_tour_owned_portable_view() {
        let view = application_view(11, TourWorkspacePhase::PatchbayOpen).unwrap();
        assert_eq!(view.revision, 11);
        assert!(view.nodes.iter().any(|node| {
            node.key == "patchbay" && node.component == ApplicationComponent::PatchbayCanvas
        }));
        assert!(view.actions.iter().any(|action| action.id == "tour.run"));
    }
}
