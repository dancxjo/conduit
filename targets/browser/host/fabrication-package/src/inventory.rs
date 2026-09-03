use conduit_host_fabrication::ConfigurationBase;
use std::collections::BTreeSet;

pub const REVIEWED_DISTRIBUTION_ID: &str = "conduit.browser/reviewed-distribution@1";
pub const REVIEWED_RUNTIME_ARTIFACT: &str = "browser-runtime-superset.wasm";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserRuntimePrerequisite {
    pub kind: &'static str,
    pub detail: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserImplementationDescriptor {
    pub group: &'static str,
    pub label: &'static str,
    pub base_kind: &'static str,
    pub implementation_id: &'static str,
    pub implementation_revision: u32,
    pub artifact: &'static str,
    pub maximum_instances: u32,
    pub maximum_buffered_bytes: u64,
    pub prerequisites: &'static [BrowserRuntimePrerequisite],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserInventoryDiagnostic {
    DuplicateImplementation(&'static str),
    UnknownBase(&'static str),
    MissingArtifactBinding(&'static str),
    InvalidTargetBinding(&'static str),
    ContradictoryPrerequisites(&'static str),
}

const SECURE_CONTEXT: BrowserRuntimePrerequisite = BrowserRuntimePrerequisite {
    kind: "secure-context",
    detail: "requires a secure browser context at runtime",
};
const USER_ACTIVATION: BrowserRuntimePrerequisite = BrowserRuntimePrerequisite {
    kind: "user-activation",
    detail: "requires an explicit current user activation at acquisition time",
};
const PERMISSION: BrowserRuntimePrerequisite = BrowserRuntimePrerequisite {
    kind: "permission",
    detail: "permission is runtime truth and is not granted by BUILD",
};
const DEVICE_ACQUISITION: BrowserRuntimePrerequisite = BrowserRuntimePrerequisite {
    kind: "device-acquisition",
    detail: "a selected live device is required before the realization can be offered",
};

pub const BROWSER_IMPLEMENTATIONS: &[BrowserImplementationDescriptor] = &[
    descriptor(
        "Presentation",
        "Browser document",
        "browser/dom",
        "browser/dom@1",
        4,
        4 * 1024 * 1024,
        &[],
    ),
    descriptor(
        "Presentation",
        "Graphical and text presentation",
        "presentation/graphical",
        "browser/dom-presentation@1",
        4,
        4 * 1024 * 1024,
        &[],
    ),
    descriptor(
        "Human input",
        "Keyboard",
        "human/keyboard",
        "browser/keyboard-events@1",
        1,
        256 * 1024,
        &[],
    ),
    descriptor(
        "Human input",
        "Pointer",
        "human/pointer",
        "browser/pointer-events@1",
        4,
        256 * 1024,
        &[],
    ),
    descriptor(
        "Storage",
        "Durable browser storage",
        "storage/durable",
        "browser/indexeddb@1",
        8,
        2 * 1024 * 1024,
        &[SECURE_CONTEXT],
    ),
    descriptor(
        "Lines",
        "WebSocket",
        "line/websocket",
        "browser/websocket@1",
        16,
        4 * 1024 * 1024,
        &[SECURE_CONTEXT],
    ),
    descriptor(
        "Lines",
        "WebRTC data channel",
        "line/webrtc-datachannel",
        "browser/webrtc-datachannel@1",
        16,
        4 * 1024 * 1024,
        &[SECURE_CONTEXT],
    ),
    descriptor(
        "Media",
        "Camera",
        "media/camera",
        "browser/media-devices-camera@1",
        2,
        8 * 1024 * 1024,
        &[
            SECURE_CONTEXT,
            USER_ACTIVATION,
            PERMISSION,
            DEVICE_ACQUISITION,
        ],
    ),
    descriptor(
        "Media",
        "Microphone",
        "media/microphone",
        "browser/media-devices-microphone@1",
        2,
        2 * 1024 * 1024,
        &[
            SECURE_CONTEXT,
            USER_ACTIVATION,
            PERMISSION,
            DEVICE_ACQUISITION,
        ],
    ),
    descriptor(
        "Media",
        "Audio output",
        "audio/output",
        "browser/web-audio-output@1",
        4,
        2 * 1024 * 1024,
        &[USER_ACTIVATION],
    ),
    descriptor(
        "Devices",
        "WebSerial",
        "device/serial",
        "browser/webserial@1",
        4,
        1024 * 1024,
        &[
            SECURE_CONTEXT,
            USER_ACTIVATION,
            PERMISSION,
            DEVICE_ACQUISITION,
        ],
    ),
    descriptor(
        "Devices",
        "WebUSB",
        "device/usb",
        "browser/webusb@1",
        4,
        1024 * 1024,
        &[
            SECURE_CONTEXT,
            USER_ACTIVATION,
            PERMISSION,
            DEVICE_ACQUISITION,
        ],
    ),
];

pub fn default_configuration_bases() -> Vec<ConfigurationBase> {
    [
        "browser/dom@1",
        "browser/keyboard-events@1",
        "browser/pointer-events@1",
    ]
    .into_iter()
    .map(|implementation| {
        let descriptor = BROWSER_IMPLEMENTATIONS
            .iter()
            .find(|item| item.implementation_id == implementation)
            .expect("default browser implementation is reviewed");
        ConfigurationBase {
            kind: descriptor.base_kind.into(),
            implementation: Some(descriptor.implementation_id.into()),
            implementations: Vec::new(),
        }
    })
    .collect()
}

pub fn validate_browser_inventory(
    descriptors: &[BrowserImplementationDescriptor],
) -> Result<(), Vec<BrowserInventoryDiagnostic>> {
    const BASES: &[&str] = &[
        "browser/dom",
        "presentation/graphical",
        "human/keyboard",
        "human/pointer",
        "storage/durable",
        "line/websocket",
        "line/webrtc-datachannel",
        "media/camera",
        "media/microphone",
        "audio/output",
        "device/serial",
        "device/usb",
    ];
    let mut diagnostics = Vec::new();
    let mut identities = BTreeSet::new();
    for descriptor in descriptors {
        if !identities.insert(descriptor.implementation_id) {
            diagnostics.push(BrowserInventoryDiagnostic::DuplicateImplementation(
                descriptor.implementation_id,
            ));
        }
        if !BASES.contains(&descriptor.base_kind) {
            diagnostics.push(BrowserInventoryDiagnostic::UnknownBase(
                descriptor.base_kind,
            ));
        }
        if descriptor.artifact != REVIEWED_RUNTIME_ARTIFACT {
            diagnostics.push(BrowserInventoryDiagnostic::MissingArtifactBinding(
                descriptor.implementation_id,
            ));
        }
        if descriptor.implementation_revision == 0
            || descriptor.maximum_instances == 0
            || descriptor.maximum_buffered_bytes == 0
        {
            diagnostics.push(BrowserInventoryDiagnostic::InvalidTargetBinding(
                descriptor.implementation_id,
            ));
        }
        let prerequisites = descriptor
            .prerequisites
            .iter()
            .map(|item| item.kind)
            .collect::<BTreeSet<_>>();
        if prerequisites.len() != descriptor.prerequisites.len()
            || (prerequisites.contains("device-acquisition")
                && (!prerequisites.contains("permission")
                    || !prerequisites.contains("user-activation")))
        {
            diagnostics.push(BrowserInventoryDiagnostic::ContradictoryPrerequisites(
                descriptor.implementation_id,
            ));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

const fn descriptor(
    group: &'static str,
    label: &'static str,
    base_kind: &'static str,
    implementation_id: &'static str,
    maximum_instances: u32,
    maximum_buffered_bytes: u64,
    prerequisites: &'static [BrowserRuntimePrerequisite],
) -> BrowserImplementationDescriptor {
    BrowserImplementationDescriptor {
        group,
        label,
        base_kind,
        implementation_id,
        implementation_revision: 1,
        artifact: REVIEWED_RUNTIME_ARTIFACT,
        maximum_instances,
        maximum_buffered_bytes,
        prerequisites,
    }
}
