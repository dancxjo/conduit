//! Physical gesture history survives Face/Show and foreground changes.
use conduit_semantic_catalog::NormalizedPointerSample;

pub(super) use crate::pointer_state::PointerState;

#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch(
    state: &mut PointerState,
    sample: NormalizedPointerSample,
    provisioned_guest: bool,
    face_arrival: &mut super::FaceArrival,
    front_door: &mut super::FrontDoor,
    journey: &mut super::ProductJourney,
    presenter: &mut super::FrontDoorPresenter,
    face_workspace: &mut super::FaceWorkspace,
    display: &mut impl crate::display::PixelTarget,
    make: &crate::make::MakeRecord,
    identities: &crate::identity::BootIdentities,
    offer: &crate::offer::HostOffer<'_>,
) -> Result<(), &'static str> {
    if state.is_retired() {
        return Ok(());
    }
    let _native_timing = crate::native_observation::Interval::begin(
        crate::native_observation::Stage::PointerFeedback,
    );
    let activate = match state.observe(sample) {
        Ok(activate) => activate,
        Err(reason) => {
            state.retire();
            crate::arch::early_write(alloc::format!("CONDUIT_POINTER_LOST {reason}\n").as_bytes());
            return Ok(());
        }
    };
    render(
        state,
        provisioned_guest,
        face_arrival,
        front_door,
        presenter,
        face_workspace,
        display,
    )?;
    if provisioned_guest || front_door.creche_open() {
        let (x, y) = state.position(display)?;
        face_arrival.observe_pointer(sample, x, y, activate)?;
    }
    if activate && (provisioned_guest || !front_door.creche_open()) {
        crate::arch::early_write(
            b"CONDUIT_POINTER_UNSUPPORTED current-entrance-has-no-pointer-action-route\n",
        );
    }
    if activate && !provisioned_guest && front_door.creche_open() {
        let (x, y) = state.position(display)?;
        if let super::FaceArrivalInput::Born =
            face_arrival.accept_pointer(x, y, front_door, journey, presenter, display, make)?
        {
            super::startup::after_birth(
                face_arrival,
                front_door,
                journey,
                presenter,
                face_workspace,
                display,
                identities,
                offer,
                make,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    state: &PointerState,
    provisioned_guest: bool,
    face_arrival: &mut super::FaceArrival,
    front_door: &super::FrontDoor,
    presenter: &mut super::FrontDoorPresenter,
    face_workspace: &mut super::FaceWorkspace,
    display: &mut impl crate::display::PixelTarget,
) -> Result<(), &'static str> {
    if state.is_retired() {
        return Ok(());
    }
    let (x, y) = state.position(display)?;
    if provisioned_guest || front_door.creche_open() {
        face_arrival.pointer_motion(x, y, display)?;
    } else if face_workspace.active() && !front_door.home_open() {
        face_workspace.pointer_motion(x, y, display)?;
    } else {
        presenter
            .route_pointer(x, y, false)
            .map_err(|error| error.as_str())?;
        presenter.compose(display).map_err(|error| error.as_str())?;
    }
    Ok(())
}
