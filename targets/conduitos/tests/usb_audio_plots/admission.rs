//! Full Source composition retains PCM across exact class setup and admission.
use super::*;

#[test]
fn setup_and_pcm_admission_prepare_before_one_playback_offer() {
    use conduitos::protocol_source::{
        PreparedProtocolEntry, PreparedProtocolSource, ProtocolSourcePackage,
        ProtocolSpecializationRequest as R, ProtocolValueReference,
    };
    let value = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    let setup = include_str!("../../plots/usb/audio-negotiation-back.conduit");
    let text = [
        "with flow/zip/feedback/paired as UsbAudioSetupPair",
        &source(),
        &setup[setup.find("plot usb-audio-setup-start").unwrap()
            ..setup.find("plot usb-audio-setup-observation").unwrap()],
        &setup[setup.find("plot usb-audio-setup-accept").unwrap()..],
        include_str!("../../plots/usb/audio-playback-back.conduit"),
        include_str!("../../plots/usb/audio-admission-back.conduit"),
    ]
    .join("\n");
    let package = ProtocolSourcePackage::compile(
        text,
        &[
            R::Singleton {
                value: value("UsbAudioSetupState"),
            },
            R::SeededUntil {
                value: value("UsbAudioSetupState"),
            },
            R::FeedbackZip {
                left: value("UsbAudioSetupState"),
                right: value("UsbAudioSetupEvent"),
            },
            R::Merge {
                value: value("UsbAudioPlaybackState"),
            },
            R::SeededUntilInstance {
                alias: "UsbAudioPlaybackCell".into(),
                value: value("UsbAudioPlaybackState"),
            },
            R::FeedbackZipInstance {
                alias: "UsbAudioPlaybackJoin".into(),
                paired_alias: "UsbAudioPlaybackPair".into(),
                left: value("UsbAudioPlaybackState"),
                right: value("UsbIsochronousOutResult"),
            },
            R::FeedbackZipInstance {
                alias: "UsbAudioInitializationJoin".into(),
                paired_alias: "UsbAudioInitializationPair".into(),
                left: value("UsbAudioPlaybackState"),
                right: value("UsbAudioAdmission"),
            },
        ],
    )
    .unwrap();
    let packaged = serde_json::to_vec(&package).unwrap();
    let prepared = PreparedProtocolSource::prepare(package).unwrap();
    let expanded = prepared.expand("usb-audio-playback").unwrap();
    PreparedProtocolEntry::prepare(&packaged, "usb-audio-playback").unwrap();
    for kind in ["machine/usb/control", "machine/usb/isochronous-out"] {
        assert_eq!(
            expanded
                .expanded
                .gears
                .iter()
                .filter(|gear| gear.kind_id.as_str() == kind)
                .count(),
            1,
            "{kind}"
        );
    }
}
