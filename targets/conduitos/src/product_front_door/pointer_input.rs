//! Physical gesture history survives Face/Show and foreground changes.
use conduit_semantic_catalog::NormalizedPointerSample;

pub(super) struct PointerState {
    x: i64,
    y: i64,
    primary_pressed: bool,
    sequence: u64,
    retired: bool,
}

impl PointerState {
    pub(super) const fn new() -> Self {
        Self {
            x: 500_000,
            y: 500_000,
            primary_pressed: false,
            sequence: 0,
            retired: false,
        }
    }

    pub(super) fn retire(&mut self) {
        self.retired = true;
    }

    pub(super) fn observe(
        &mut self,
        sample: NormalizedPointerSample,
    ) -> Result<bool, &'static str> {
        if self.retired {
            return Err("product-pointer-retired");
        }
        if !(0..=1_000_000).contains(&sample.position_x)
            || !(0..=1_000_000).contains(&sample.position_y)
            || sample.sequence <= self.sequence
        {
            return Err("product-pointer-sample-invalid");
        }
        // A coalesced button history cannot establish a unique press edge.
        if sample.coalesced != 0 || sample.dropped != 0 {
            return Err("product-pointer-gesture-history-gap");
        }
        let activate = sample.primary_pressed && !self.primary_pressed;
        self.x = sample.position_x;
        self.y = sample.position_y;
        self.primary_pressed = sample.primary_pressed;
        self.sequence = sample.sequence;
        Ok(activate)
    }

    pub(super) fn position(
        &self,
        display: &impl crate::display::PixelTarget,
    ) -> Result<(u32, u32), &'static str> {
        let format = display
            .format()
            .validate()
            .map_err(crate::display::DisplayError::as_str)?;
        Ok((
            crate::pointer_coordinates::display_coordinate(self.x, format.width)?,
            crate::pointer_coordinates::display_coordinate(self.y, format.height)?,
        ))
    }
}

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
) -> Result<(), &'static str> {
    if state.retired {
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
            let (observation, play) = face_arrival.next_publication_sequences();
            face_workspace.continue_after_birth(observation, play);
            if journey.foreground_text_editor_active() {
                face_workspace.enter_editor(front_door, presenter, display)?;
            }
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
    if state.retired {
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

#[cfg(test)]
#[path = "pointer_input_tests.rs"]
mod tests;
