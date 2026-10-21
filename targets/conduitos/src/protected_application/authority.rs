//! Root effect admission over genuine input and the exact previously shown view.
//!
//! The caller owns occurrence/provenance admission and single-use dispatch. It
//! supplies the immutable Root-reviewed graph for its explicitly selected target
//! and its latest topology for the still-live Body Plan/Play. Guest replies never
//! establish that custody. This helper neither advances application state nor
//! authorizes installation, execution, editing or Mask replacement by itself.
use conduit_face::{
    ApplicationComponent, ApplicationEvent, ApplicationEventKind, ApplicationNodeState,
    ApplicationView, ApplicationViewNode,
};
use conduitos_application_core::data::{Context, Kind, Proposal, TutorialAction};
use patchbay_application::{PatchbayApplicationRequest, PatchbayMaskMode, PatchbayMaskTopology};
use patchbay_graph::PatchbayGraph;

pub(super) fn admit(
    kind: Kind,
    input: &[u8],
    prior: &ApplicationView,
    context: &Context,
    graph: Option<&PatchbayGraph>,
    topology: Option<&PatchbayMaskTopology>,
    proposal: Option<&Proposal>,
) -> Result<(), ()> {
    validate_context(kind, prior, context)?;
    if input.is_empty() {
        return if proposal.is_none() { Ok(()) } else { Err(()) };
    }
    let event = ApplicationEvent::decode(input, prior).map_err(|_| ())?;
    match kind {
        Kind::Tutorial => {
            let expected = match event.action.as_str() {
                "body.wake" => TutorialAction::Wake,
                "body.inspect-lifecycle" => TutorialAction::InspectLifecycle,
                "body.open-library" => TutorialAction::OpenLibrary,
                "body.invite-host" => TutorialAction::InviteHost,
                "body.use-current" => TutorialAction::UseCurrent,
                _ => return Err(()),
            };
            ready_activation(&event, prior)?;
            if proposal == Some(&Proposal::Tutorial(expected)) {
                Ok(())
            } else {
                Err(())
            }
        }
        Kind::Tour => match event.action.as_str() {
            conduit_tour_model::RUN_ACTION_ID => {
                ready_activation(&event, prior)?;
                let Context::Tour { chapter, stage } = context else {
                    return Err(());
                };
                let chapter_spec = conduit_tour_model::TOUR_CHAPTERS
                    .get(usize::from(*chapter))
                    .ok_or(())?;
                chapter_spec.stages.get(usize::from(*stage)).ok_or(())?;
                if proposal
                    == Some(&Proposal::RunTour {
                        chapter: *chapter,
                        stage: *stage,
                    })
                {
                    Ok(())
                } else {
                    Err(())
                }
            }
            conduit_tour_model::OPEN_PATCHBAY_ACTION_ID => {
                ready_activation(&event, prior)?;
                if proposal == Some(&Proposal::OpenPatchbay) {
                    Ok(())
                } else {
                    Err(())
                }
            }
            _ => {
                if proposal.is_none() {
                    Ok(())
                } else {
                    Err(())
                }
            }
        },
        Kind::Patchbay => match event.action.as_str() {
            patchbay_application::EDIT_CURRENT_ACTION_ID => {
                ready_activation(&event, prior)?;
                let Some(Proposal::EditCurrent(PatchbayApplicationRequest::EditCurrent {
                    expanded_plot_id,
                    subject_identity,
                })) = proposal
                else {
                    return Err(());
                };
                let graph = graph.ok_or(())?;
                let identity = unique_node(prior, "identity", ApplicationComponent::Status)?;
                let (shown, _) = identity
                    .text
                    .strip_prefix("Expanded Plot ")
                    .and_then(|text| text.split_once(" · Plan "))
                    .ok_or(())?;
                if shown != graph.expanded_plot_id.as_str() {
                    return Err(());
                }
                let subject = unique_node(prior, "subject", ApplicationComponent::Definition)?;
                if &graph.expanded_plot_id != expanded_plot_id || subject.value != *subject_identity
                {
                    return Err(());
                }
                graph.subject_ref(subject_identity).map_err(|_| ())?;
                Ok(())
            }
            patchbay_application::CHANGE_MASKS_ACTION_ID => {
                ready_activation(&event, prior)?;
                let Some(Proposal::EditCurrent(PatchbayApplicationRequest::ChangeMasks {
                    body_plan_id,
                    mode,
                })) = proposal
                else {
                    return Err(());
                };
                let topology = topology.ok_or(())?;
                for (key, expected) in [
                    ("face-identity", topology.face_id.as_str()),
                    ("mask-body-plan", topology.body_plan_id.as_str()),
                    ("mask-play", topology.active_play_id.as_str()),
                ] {
                    if unique_node(prior, key, ApplicationComponent::Definition)?.value != expected
                    {
                        return Err(());
                    }
                }
                if topology.face_id.is_empty()
                    || topology.active_play_id.is_empty()
                    || &topology.body_plan_id != body_plan_id
                    || next_mode(topology.mode) != *mode
                {
                    return Err(());
                }
                Ok(())
            }
            _ => {
                if proposal.is_none() {
                    Ok(())
                } else {
                    Err(())
                }
            }
        },
    }
}

/// Validate readonly context before caching it, including responses to commands
/// which cannot propose effects (Init/Refresh/Select/Topology/CompleteTour).
/// Those callers must independently reject any proposal in their reply shape.
pub(super) fn validate_context(
    kind: Kind,
    view: &ApplicationView,
    context: &Context,
) -> Result<(), ()> {
    view.validate().map_err(|_| ())?;
    match (kind, context) {
        (Kind::Tutorial | Kind::Patchbay, Context::None) => Ok(()),
        (Kind::Tour, Context::Tour { chapter, stage }) => {
            let chapter_spec = conduit_tour_model::TOUR_CHAPTERS
                .get(usize::from(*chapter))
                .ok_or(())?;
            let source = unique_node(view, "source", ApplicationComponent::CodeBlock)?;
            let canvas = unique_node(view, "patchbay", ApplicationComponent::PatchbayCanvas)?;
            let specimen = match chapter_spec.stages.get(usize::from(*stage)) {
                Some(stage_spec) => {
                    let expected =
                        conduit_tour_model::tour_stage_source(*chapter, *stage).map_err(|_| ())?;
                    if source.value != expected {
                        return Err(());
                    }
                    stage_spec.identity
                }
                None if *stage == 0 && chapter_spec.stages.is_empty() => {
                    if !source.value.is_empty() {
                        return Err(());
                    }
                    chapter_spec.companion
                }
                None => return Err(()),
            };
            // Bind the complete specimen token, not a prefix such as stage1
            // accidentally accepting stage10. The suffix is ordinary UI state.
            let label = canvas.text.strip_prefix("Active plot ").ok_or(())?;
            let (shown, _) = label.split_once("; ").ok_or(())?;
            if shown == specimen { Ok(()) } else { Err(()) }
        }
        _ => Err(()),
    }
}

fn unique_node<'a>(
    view: &'a ApplicationView,
    key: &str,
    component: ApplicationComponent,
) -> Result<&'a ApplicationViewNode, ()> {
    let mut nodes = view.nodes.iter().filter(|node| node.key == key);
    let node = nodes.next().ok_or(())?;
    if nodes.next().is_some() || node.component != component {
        return Err(());
    }
    Ok(node)
}

fn ready_activation(event: &ApplicationEvent, view: &ApplicationView) -> Result<(), ()> {
    if event.kind != ApplicationEventKind::Activate || !event.value.is_empty() {
        return Err(());
    }
    let index = view
        .actions
        .iter()
        .position(|action| action.id == event.action)
        .ok_or(())?;
    let mut referenced = false;
    for node in view
        .nodes
        .iter()
        .filter(|node| node.action.map(usize::from) == Some(index))
    {
        referenced = true;
        if node.state != ApplicationNodeState::Ready {
            return Err(());
        }
    }
    if referenced { Ok(()) } else { Err(()) }
}

// Exact canonical PatchbayMaskTopology::next_mode law; it is private to the
// canonical crate, so this Root admission comparison cannot call it directly.
fn next_mode(mode: PatchbayMaskMode) -> PatchbayMaskMode {
    match mode {
        PatchbayMaskMode::Graphical => PatchbayMaskMode::GraphicalAndSpeech,
        PatchbayMaskMode::GraphicalAndSpeech => PatchbayMaskMode::Speech,
        PatchbayMaskMode::Speech => PatchbayMaskMode::GraphicalAndSpeech,
    }
}
