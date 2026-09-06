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

/// One ordinary runtime realization carried by a reviewed fabrication entry.
/// The outer artifact is the exact BrowserBundle WASM; `runtime_artifact_id`
/// is the identity retained in the resulting CapabilityOffer and Plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserRealizationDescriptor {
    pub fabrication_implementation_id: &'static str,
    pub portable_kind: &'static str,
    pub runtime_implementation_id: &'static str,
    pub runtime_artifact_id: &'static str,
    pub host_operation: &'static str,
    pub maximum_in_flight: u16,
    pub maximum_queue_items: u32,
    pub maximum_queue_bytes: u32,
}

/// Exact reviewed binding for the bounded application-state realization.
/// Host identity uses a separate object store and is not part of this offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserStorageRealizationDescriptor {
    pub fabrication_implementation_id: &'static str,
    pub portable_kind: &'static str,
    pub implementation_revision: u32,
    pub artifact_id: &'static str,
    pub database_name: &'static str,
    pub application_store: &'static str,
    pub host_identity_store: &'static str,
    pub maximum_records_per_application: u32,
    pub maximum_key_bytes: u32,
    pub maximum_value_bytes: u32,
    pub maximum_bytes_per_application: u32,
    pub maximum_applications_per_host: u32,
    pub maximum_records_per_host: u32,
    pub maximum_bytes_per_host: u32,
}

pub const BROWSER_DURABLE_STORAGE_REALIZATION: BrowserStorageRealizationDescriptor =
    BrowserStorageRealizationDescriptor {
        fabrication_implementation_id: "browser/indexeddb@1",
        portable_kind: "storage/durable",
        implementation_revision: 1,
        artifact_id: "browser-application-storage.mjs@1",
        database_name: "conduit-browser-host-applications",
        application_store: "application-state",
        host_identity_store: "browser-host-identity",
        maximum_records_per_application: 64,
        maximum_key_bytes: 256,
        maximum_value_bytes: 64 * 1024,
        maximum_bytes_per_application: 1024 * 1024,
        maximum_applications_per_host: 16,
        maximum_records_per_host: 16 * 64,
        maximum_bytes_per_host: 16 * 1024 * 1024,
    };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BrowserRealizationLimits {
    maximum_in_flight: u16,
    maximum_queue_items: u32,
    maximum_queue_bytes: u32,
}

pub const BROWSER_HUMAN_PRESENTATION_REALIZATIONS: &[BrowserRealizationDescriptor] = &[
    realization(
        "browser/dom-presentation@1",
        "presentation/text",
        "browser/presentation-text@1",
        "conduit-browser-runtime/installed-text@1",
        "conduit.host/browser-present-text@1",
        limits(1, 4, 4_096),
    ),
    realization(
        "browser/dom-presentation@1",
        "presentation/structured-info",
        "browser/presentation-structured-info@1",
        "conduit-browser-runtime/installed-linguistics@1",
        "conduit.host/browser-linguistics@1",
        limits(1, 1, 4_096),
    ),
    realization(
        "browser/dom-presentation@1",
        "presentation/count",
        "browser/presentation-count@1",
        "conduit-browser-runtime/installed-state-time@1",
        "conduit.host/browser-present-count@1",
        limits(1, 5, 4_096),
    ),
    realization(
        "browser/dom-presentation@1",
        "presentation/indicator",
        "browser/dom-indicator@2",
        "conduit-browser-runtime/installed-presentation@1",
        "conduit.host/browser-present-indicator@1",
        limits(1, 1, 4_096),
    ),
    realization(
        "browser/dom-presentation@1",
        "presentation/bool",
        "browser/presentation-bool@1",
        "conduit-browser-runtime/installed-presentation@1",
        "conduit.host/browser-present-current-bool@1",
        limits(1, 1, 4_096),
    ),
    realization(
        "browser/dom-presentation@1",
        "presentation/patchbay",
        "browser/patchbay-surface@1",
        "conduit-browser-runtime/installed-presentation@1",
        "conduit.host/browser-present-patchbay@1",
        limits(1, 4, 4_096),
    ),
    realization(
        "browser/dom-presentation@1",
        "presentation/scalar",
        "browser/presentation-scalar@1",
        "conduit-browser-runtime/installed-values@1",
        "browser/presentation-scalar@1",
        limits(1, 1, 4_096),
    ),
    realization(
        "browser/dom-presentation@1",
        "presentation/bool-value",
        "browser/presentation-bool-value@1",
        "conduit-browser-runtime/installed-values@1",
        "browser/presentation-bool-value@1",
        limits(1, 1, 4_096),
    ),
    realization(
        "browser/keyboard-events@1",
        "input/keyboard",
        "browser/window-keyboard@1",
        "conduit-browser-runtime/installed-input@1",
        "conduit.host/browser-key-event@1",
        limits(1, 8, 4_096),
    ),
    realization(
        "browser/pointer-events@1",
        "input/pointer-source",
        "browser/dom-pointer-source@1",
        "conduit-browser-runtime/pointer-source@1",
        "browser.host/pointer-source@1",
        limits(1, 1, 65_536),
    ),
];

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

const fn realization(
    fabrication_implementation_id: &'static str,
    portable_kind: &'static str,
    runtime_implementation_id: &'static str,
    runtime_artifact_id: &'static str,
    host_operation: &'static str,
    limits: BrowserRealizationLimits,
) -> BrowserRealizationDescriptor {
    BrowserRealizationDescriptor {
        fabrication_implementation_id,
        portable_kind,
        runtime_implementation_id,
        runtime_artifact_id,
        host_operation,
        maximum_in_flight: limits.maximum_in_flight,
        maximum_queue_items: limits.maximum_queue_items,
        maximum_queue_bytes: limits.maximum_queue_bytes,
    }
}

const fn limits(
    maximum_in_flight: u16,
    maximum_queue_items: u32,
    maximum_queue_bytes: u32,
) -> BrowserRealizationLimits {
    BrowserRealizationLimits {
        maximum_in_flight,
        maximum_queue_items,
        maximum_queue_bytes,
    }
}
