//! Button meaning over the already admitted native window input lane.
use conduit_core::HostAdvertisement;

pub fn append_offers(advertisement: &mut HostAdvertisement) -> Result<(), String> {
    let keyboard = advertisement
        .capabilities
        .iter()
        .find(|offer| {
            offer.implementation.implementation_id.as_str() == super::NATIVE_KEYBOARD_IMPLEMENTATION
        })
        .ok_or("native button requires the native keyboard installation")?;
    let mut source = conduit_std_offers::button::offer();
    // Both sources consume one exclusive input/operation lane. They cannot
    // independently drain copies of the same native queue in concurrent Plays.
    source.resource_requirements = keyboard.resource_requirements.clone();
    let offers = [
        source,
        conduit_std_offers::button::mapper_offer(),
        conduit_std_offers::button::indicator_offer(),
    ];
    if offers.iter().any(|new| {
        advertisement.capabilities.iter().any(|old| {
            old.capability_id == new.capability_id
                || old.implementation.implementation_id == new.implementation.implementation_id
        })
    }) {
        return Err("native button installation duplicates an existing offer".into());
    }
    advertisement.capabilities.extend(offers);
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_button_adapter_executes_unchanged_form() {
        use conduit_std_host::{RunControl, StdHost, TimerAdapter};
        struct Timer;
        impl TimerAdapter for Timer {
            fn wait(&mut self, _: std::time::Duration) {}
        }
        let mut advertisement = StdHost::new().advertisement().clone();
        super::super::append_offer(&mut advertisement).unwrap();
        append_offers(&mut advertisement).unwrap();
        let mut host = StdHost::from_advertisement(advertisement).unwrap();
        let source = include_str!("../../../../../forms/button-across-room/main.conduit");
        let editor = patchbay_model::FormEditor::from_source(
            "button-across-room.conduit".into(),
            source.into(),
        )
        .unwrap();
        let mut keyboard = super::super::NativeKeyboardInput::new();
        let mut control = crate::control::NativeControl::for_advertisement(
            host.advertisement().clone(),
            keyboard.reader(),
        )
        .unwrap();
        control.request_plan(&editor).unwrap();
        let plan = control.plan().unwrap().clone();
        for state in [
            winit::event::ElementState::Pressed,
            winit::event::ElementState::Released,
        ] {
            keyboard
                .observe(
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Space),
                    state,
                    false,
                )
                .unwrap();
        }
        let mut output = Vec::new();
        let report = host
            .run_fragment_controlled_with_keyboard_to(
                plan.fragments[0].clone(),
                &mut output,
                &mut Timer,
                &RunControl::default(),
                Some(&mut keyboard.reader()),
            )
            .unwrap();
        let output = String::from_utf8(output).unwrap();
        assert_eq!(
            output
                .lines()
                .filter_map(|line| line.strip_prefix("bool value="))
                .collect::<Vec<_>>(),
            ["true", "false"]
        );
        assert!(matches!(
            report.observations.last().map(|item| &item.kind),
            Some(conduit_core::ObservationKind::PlanTerminal {
                disposition: conduit_core::TerminalDisposition::Completed
            })
        ));
    }

    #[test]
    fn native_button_shares_exact_input_reservations_and_refuses_duplicates() {
        let mut advertisement = conduit_std_host::StdHost::new().advertisement().clone();
        assert!(append_offers(&mut advertisement).is_err());
        super::super::append_offer(&mut advertisement).unwrap();
        let resources = advertisement.resources.clone();
        append_offers(&mut advertisement).unwrap();
        let keyboard = advertisement
            .capabilities
            .iter()
            .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::KEYBOARD_KIND)
            .unwrap();
        let button = advertisement
            .capabilities
            .iter()
            .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::BUTTON_SOURCE_KIND)
            .unwrap();
        assert_eq!(button.resource_requirements, keyboard.resource_requirements);
        assert_eq!(advertisement.resources, resources);
        let before = advertisement.clone();
        assert!(append_offers(&mut advertisement).is_err());
        assert_eq!(advertisement, before);
    }
}
