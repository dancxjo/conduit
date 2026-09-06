/// Exact reviewed binding from selectable media-acquisition machinery to the
/// already-accepted two-Plan browser realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserMediaRealizationDescriptor {
    pub fabrication_implementation_id: &'static str,
    pub implementation_revision: u32,
    pub acquisition_offer_id: &'static str,
    pub acquired_resource_class: &'static str,
    pub value_kind: &'static str,
    pub output_port: &'static str,
    pub adapter_artifact_id: &'static str,
    pub runtime_artifact_id: &'static str,
    pub host_operation: &'static str,
    pub request_authority: &'static str,
    pub use_authority: &'static str,
    pub maximum_acquisitions_in_flight: u16,
    pub maximum_result_bytes: u32,
    pub maximum_value_bytes: u32,
    pub maximum_queue_items: u16,
    pub maximum_queue_bytes: u32,
    pub stable_physical_device_identity: bool,
    pub requires_subsequent_use_plan: bool,
}

pub const BROWSER_MEDIA_REALIZATIONS: &[BrowserMediaRealizationDescriptor] = &[
    media(
        "browser/media-devices-camera@1",
        "media/acquire-camera@1",
        "conduit.resource/acquired-camera@1",
        "media/camera-frame@1",
        "frame",
    ),
    media(
        "browser/media-devices-microphone@1",
        "media/acquire-microphone@1",
        "conduit.resource/acquired-microphone@1",
        "media/microphone-frame@1",
        "chunk",
    ),
];

const fn media(
    fabrication_implementation_id: &'static str,
    acquisition_offer_id: &'static str,
    acquired_resource_class: &'static str,
    value_kind: &'static str,
    output_port: &'static str,
) -> BrowserMediaRealizationDescriptor {
    BrowserMediaRealizationDescriptor {
        fabrication_implementation_id,
        implementation_revision: 1,
        acquisition_offer_id,
        acquired_resource_class,
        value_kind,
        output_port,
        adapter_artifact_id: "browser-host/media-host.mjs@1",
        runtime_artifact_id: "conduit-browser-runtime/human-media@1",
        host_operation: "conduit.host/acquire-human-media@1",
        request_authority: "conduit.authority/request-human-media@1",
        use_authority: "conduit.authority/use-human-media@1",
        maximum_acquisitions_in_flight: 1,
        maximum_result_bytes: 1024,
        maximum_value_bytes: 64 * 1024,
        maximum_queue_items: 1,
        maximum_queue_bytes: 64 * 1024,
        stable_physical_device_identity: false,
        requires_subsequent_use_plan: true,
    }
}
