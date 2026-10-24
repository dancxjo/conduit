// Generated from mechanisms/implementations/bounded-lucide/gui-vocabulary.json.
// Lucide notices: mechanisms/implementations/bounded-lucide/LICENSE.
use conduit_face::{FaceIconKey as Icon, FaceRole};
pub(in crate::native_face_scene) fn for_role(role: &FaceRole) -> Option<Icon> {
    Some(match role {
        FaceRole::Body => Icon::Body,
        FaceRole::Plot => Icon::Face,
        FaceRole::Document => Icon::Type,
        FaceRole::Host => Icon::Host,
        FaceRole::Gear => Icon::GenericGear,
        FaceRole::Port => Icon::Port,
        FaceRole::Line => Icon::Line,
        FaceRole::Cord => Icon::Line,
        FaceRole::Plan => Icon::Plan,
        FaceRole::Play => Icon::Play,
        FaceRole::Status => Icon::Status,
        FaceRole::Diagnostic => Icon::Warning,
        FaceRole::Collection => Icon::Collection,
        _ => return None,
    })
}
pub(in crate::native_face_scene) fn for_intent(intent: &str) -> Option<Icon> {
    Some(match intent {
        "conduit.intent/birth@1" => Icon::Wake,
        "conduit.intent/creche.birth@1" => Icon::Wake,
        "conduit.intent/wake@1" => Icon::Wake,
        "conduit.intent/lull@1" => Icon::Lull,
        "conduit.intent/creche.name@1" => Icon::Type,
        "patchbay/edit-subject@1" => Icon::Type,
        "conduit.intent/creche.naming@1" => Icon::GenericGear,
        "conduit.intent/creche.suggest@1" => Icon::Repeat2,
        "todo/reopen@1" => Icon::Repeat2,
        "conduit.intent/creche.search@1" => Icon::Search,
        "conduit.intent/open-inspection@1" => Icon::Inspect,
        "conduit.intent/inspect@1" => Icon::Inspect,
        "conduit.intent/fulfill@1" => Icon::Confirm,
        "todo/complete@1" => Icon::Confirm,
        "conduit.intent/open-body-overview@1" => Icon::Body,
        "conduit.intent/open-resident-plot@1" => Icon::Face,
        "patchbay/select-plot@1" => Icon::Face,
        "conduit.intent/open-plot-library@1" => Icon::Library,
        _ => return None,
    })
}
