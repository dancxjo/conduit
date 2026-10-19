//! Closed native face selection on the existing finite return bearer.
use super::*;
use conduit_std_host::browser_admission::{BrowserComponentLook, BrowserComponentRecipe};

pub(super) const SCHEMA: &str = "conduit.body/native-owner-component-selection@1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    schema: String,
    token: [u8; 32],
    sequence: u8,
    request: OwnerFaceSnapshotRequest,
    show: MaskShow,
    look: BrowserComponentLook,
    recipe: BrowserComponentRecipe,
}

pub(super) fn serve(
    line: &mut conduit_std_host::secure_websocket::SecureWebSocketLine,
    listener: &SecureWebSocketListener,
    state_dir: &Path,
    old: &Grant,
    next_sequence: u8,
    document: serde_json::Value,
) -> Result<show_ack::ShowGate, String> {
    let selection: Selection =
        serde_json::from_value(document).map_err(|_| "native component selection frame invalid")?;
    if selection.schema != SCHEMA
        || selection.sequence != 0
        || current_time_millis()? >= old.expires_at_millis
        || !old.matches_request(selection.token, &selection.request)
    {
        return Err("native component selection grant or basis invalid".into());
    }
    line.complete_bounded_input();
    let look = match selection.look {
        BrowserComponentLook::Compact => conduit_face::ComponentMaskLook::Compact,
        BrowserComponentLook::Spacious => conduit_face::ComponentMaskLook::Spacious,
    };
    let recipe = match selection.recipe {
        BrowserComponentRecipe::Flat => conduit_face::TodoComponentRecipe::Flat,
        BrowserComponentRecipe::Grouped => conduit_face::TodoComponentRecipe::Grouped,
        BrowserComponentRecipe::RequiredItem => {
            return Err("native-component-recipe-unsupported".into())
        }
    };
    // The serialized owner validates the acknowledged old Show before replacing
    // its route. Refresh then issues the fresh bounded bearer and exact Face;
    // the guest still has to render and acknowledge the replacement Show.
    crate::durable_host_control::select_native_component_mask(
        state_dir,
        old.receipt.clone(),
        selection.request.clone(),
        selection.show,
        look,
        recipe,
    )?;
    refresh::serve(
        line,
        listener,
        state_dir,
        old,
        next_sequence,
        serde_json::json!({
            "schema": refresh::REFRESH_SCHEMA, "token": selection.token,
            "sequence": 0, "request": selection.request,
        }),
    )
}
