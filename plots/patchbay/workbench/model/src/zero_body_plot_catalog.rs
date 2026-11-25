//! Reviewed starting Plots for the actual Host advertisement.
use crate::zero_body_front_door::PlotCandidate;
use conduit_core::{HostAdvertisement, SignId};

pub(super) fn reviewed_plots(host: &HostAdvertisement) -> Result<Vec<PlotCandidate>, String> {
    let morse_network = PlotCandidate::from_source(
        "Morse Network",
        "plots/morse-network/main.conduit",
        include_str!("../../../../../plots/morse-network/main.conduit"),
        "reviewed plot inventory; opening is inert and BIRTH remains explicit",
        SignId::from("patchbay/front-door/plot-available"),
        1,
    )?;
    let clock = PlotCandidate::from_source(
        "Interval ticker",
        "plots/clock/main.conduit",
        include_str!("../../../../../plots/clock/main.conduit"),
        "reviewed plot inventory; opening is inert and BIRTH remains explicit",
        SignId::from("patchbay/front-door/clock-available"),
        2,
    )?;
    let mut plots = vec![morse_network, clock];
    if host.capabilities.iter().any(|offer| {
        offer.implementation.implementation_id.as_str()
            == conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION
    }) {
        plots.push(PlotCandidate::from_source(
            "Todo list",
            "plots/todo/checkpoint-once.conduit",
            include_str!("../../../../../plots/todo/checkpoint-once.conduit"),
            "reviewed Todo Plot; requires this Host's selected checkpoint residence",
            SignId::from("patchbay/front-door/todo-checkpoint-available"),
            3,
        )?);
    }
    if [
        conduit_std_offers::TIME_EVERY_IMPLEMENTATION,
        conduit_std_offers::AMBIENT_HOME_SOURCE_IMPLEMENTATION,
        conduit_std_offers::AMBIENT_HOME_FACE_IMPLEMENTATION,
    ]
    .iter()
    .all(|implementation| {
        host.capabilities
            .iter()
            .any(|offer| offer.implementation.implementation_id.as_str() == *implementation)
    }) {
        plots.push(PlotCandidate::from_source(
            "Ambient Home",
            "plots/ambient-home/main.conduit",
            include_str!("../../../../../plots/ambient-home/main.conduit"),
            "reviewed Ambient Home Plot with selected clock, observation and Face Backs",
            SignId::from("patchbay/front-door/ambient-home-available"),
            4,
        )?);
    }
    if host.capabilities.iter().any(|offer| {
        offer.implementation.implementation_id.as_str()
            == conduit_std_offers::PATCHBAY_INTERACTION_IMPLEMENTATION
    }) {
        plots.push(PlotCandidate::from_source(
            "Patchbay",
            "plots/patchbay/main.conduit",
            include_str!("../../../../../plots/patchbay/main.conduit"),
            "reviewed Patchbay Plot; requires the installed interaction Back",
            SignId::from("patchbay/front-door/resident-available"),
            5,
        )?);
    }
    if [
        conduit_std_offers::TIME_EVERY_IMPLEMENTATION,
        conduit_std_offers::AMBIENT_CLOCK_SOURCE_IMPLEMENTATION,
        conduit_std_offers::AMBIENT_CLOCK_FACE_IMPLEMENTATION,
    ]
    .iter()
    .all(|implementation| {
        host.capabilities
            .iter()
            .any(|offer| offer.implementation.implementation_id.as_str() == *implementation)
    }) {
        plots.push(PlotCandidate::from_source(
            "Seattle clock and weather",
            "plots/resident-ambient-clock-weather/main.conduit",
            include_str!("../../../../../plots/resident-ambient-clock-weather/main.conduit"),
            "reviewed clock and weather Plot with selected timer, observation and Face Backs",
            SignId::from("patchbay/front-door/ambient-clock-available"),
            6,
        )?);
    }
    if [
        conduit_std_offers::TIME_EVERY_IMPLEMENTATION,
        conduit_std_offers::AMBIENT_WALLPAPER_SOURCE_IMPLEMENTATION,
        conduit_std_offers::AMBIENT_WALLPAPER_FACE_IMPLEMENTATION,
    ]
    .iter()
    .all(|implementation| {
        host.capabilities
            .iter()
            .any(|offer| offer.implementation.implementation_id.as_str() == *implementation)
    }) {
        plots.push(PlotCandidate::from_source(
            "Rotating wallpaper",
            "plots/resident-ambient-wallpaper/main.conduit",
            include_str!("../../../../../plots/resident-ambient-wallpaper/main.conduit"),
            "reviewed wallpaper Plot with selected timer, observation and Face Backs",
            SignId::from("patchbay/front-door/ambient-wallpaper-available"),
            7,
        )?);
    }
    if [
        conduit_std_offers::BODY_WAKE_IMPLEMENTATION,
        conduit_std_offers::STARTUP_CHIME_IMPLEMENTATION,
    ]
    .iter()
    .all(|implementation| {
        host.capabilities
            .iter()
            .any(|offer| offer.implementation.implementation_id.as_str() == *implementation)
    }) {
        plots.push(PlotCandidate::from_source(
            "Startup Chime",
            "plots/startup-chime/main.conduit",
            include_str!("../../../../../plots/startup-chime/main.conduit"),
            "reviewed Startup Chime Plot; playback requires an authorized Wake",
            SignId::from("patchbay/front-door/startup-chime-available"),
            8,
        )?);
    }
    Ok(plots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PatchbayHostProfile;

    #[test]
    fn wallpaper_is_selectable_only_with_its_observation_and_face_backs() {
        let mut host = crate::host_adapter::test_host_adapter()
            .advertisement(
                "fixture/wallpaper-host".into(),
                "fixture/wallpaper-boot".into(),
                conduit_core::OfferGeneration(1),
                PatchbayHostProfile::Authoring,
            )
            .unwrap();
        host.capabilities = vec![
            conduit_std_offers::time_every_offer(),
            conduit_std_offers::ambient_wallpaper_source_offer().unwrap(),
        ];
        assert!(reviewed_plots(&host)
            .unwrap()
            .iter()
            .all(|plot| plot.label != "Rotating wallpaper"));

        host.capabilities.push(
            conduit_std_offers::AmbientFaceProjection::Wallpaper
                .offer()
                .unwrap(),
        );
        let plots = reviewed_plots(&host).unwrap();
        let wallpaper = plots
            .iter()
            .find(|plot| plot.label == "Rotating wallpaper")
            .unwrap();
        assert_eq!(
            wallpaper.source_name,
            "plots/resident-ambient-wallpaper/main.conduit"
        );
        assert_eq!(
            wallpaper.editor.view().checked.plots[0].name,
            "resident-ambient-wallpaper"
        );
    }

    #[test]
    fn ambient_home_requires_the_complete_host_offer_and_checks_its_source() {
        let mut host = crate::host_adapter::test_host_adapter()
            .advertisement(
                "fixture/home-host".into(),
                "fixture/home-boot".into(),
                conduit_core::OfferGeneration(1),
                PatchbayHostProfile::Authoring,
            )
            .unwrap();
        host.capabilities = vec![
            conduit_std_offers::time_every_offer(),
            conduit_std_offers::ambient_home_source_offer().unwrap(),
        ];
        assert!(reviewed_plots(&host)
            .unwrap()
            .iter()
            .all(|plot| plot.label != "Ambient Home"));

        host.capabilities
            .push(conduit_std_offers::resident_ambient_home_face_offer().unwrap());
        let plots = reviewed_plots(&host).unwrap();
        let home = plots
            .iter()
            .find(|plot| plot.label == "Ambient Home")
            .unwrap();
        assert_eq!(home.source_name, "plots/ambient-home/main.conduit");
        assert_eq!(home.editor.view().checked.plots[0].name, "ambient-resident");
    }
}
