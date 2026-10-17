use conduitos::protocol_source::*;
pub(super) fn package() -> ProtocolSourcePackage {
    let service = include_str!("../../plots/usb/cdc-ecm-service.conduit");
    let receive = include_str!("../../plots/usb/cdc-ecm-frames.conduit");
    let mut imports = Vec::new();
    for line in service.lines().chain(receive.lines()) {
        if line.starts_with("with ") && !imports.contains(&line) {
            imports.push(line);
        }
    }
    let body = |text: &str| {
        text.lines()
            .filter(|line| !line.starts_with("with "))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let value = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    ProtocolSourcePackage::compile(
        format!(
            "{}\n{}\n{}\n{}\n{}",
            imports.join("\n"),
            include_str!("../../plots/usb/cdc-ecm-types.conduit"),
            include_str!("../../plots/usb/cdc-ecm-profile.conduit"),
            body(receive),
            body(service)
        ),
        &[
            ProtocolSpecializationRequest::Singleton {
                value: value("CdcEcmStart"),
            },
            ProtocolSpecializationRequest::Concat {
                value: ProtocolValueReference {
                    type_name: "CdcEcmOutcome".into(),
                    maximum_bytes: 512,
                },
            },
            ProtocolSpecializationRequest::SeededUntil {
                value: value("CdcEcmState"),
            },
            ProtocolSpecializationRequest::FeedbackZip {
                left: value("CdcEcmState"),
                right: value("CdcEcmEvent"),
            },
            ProtocolSpecializationRequest::Merge {
                value: value("CdcEcmEvent"),
            },
        ],
    )
    .unwrap()
}
