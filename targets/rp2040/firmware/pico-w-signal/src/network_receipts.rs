use conduit_core::LineOffer;
use heapless::String as HString;

use crate::receipts::{UsbCdc, UsbSignError, RECEIPT_BUFFER_BYTES};

#[derive(Clone, Copy)]
pub struct NetworkAttachmentIdentity<'a> {
    pub firmware_build_id: &'static str,
    pub source_document_id: &'static str,
    pub checked_form_id: &'static str,
    pub expanded_form_id: &'static str,
    pub plan_id: &'static str,
    pub fragment_id: &'static str,
    pub host_id: &'static str,
    pub boot_id: &'a str,
    pub active_play_id: &'a str,
    pub attachment_id: &'a str,
    pub interface_pool_id: &'static str,
    pub generation: u64,
    pub sign_id: &'static str,
}

#[derive(Clone, Copy)]
pub struct WebSocketRouteIdentity<'a> {
    pub firmware_build_id: &'static str,
    pub attachment_id: &'static str,
    pub interface_pool_id: &'static str,
    pub usb_link: &'a LineOffer,
    pub websocket_link: &'a LineOffer,
    pub address: [u8; 4],
    pub port: u16,
    pub sign_id: &'static str,
}

impl UsbCdc {
    pub async fn write_websocket_endpoint(
        &mut self,
        identity: WebSocketRouteIdentity<'_>,
    ) -> Result<(), UsbSignError> {
        let mut line: HString<1024> = HString::new();
        core::fmt::write(
            &mut line,
            format_args!(
                concat!(
                    "{{",
                    "\"schema\":\"conduit.network/websocket-endpoint-sign@1\",",
                    "\"firmware_build_id\":\"{}\",",
                    "\"host_id\":\"{}\",",
                    "\"runtime_boot_id\":\"{}\",",
                    "\"attachment_id\":\"{}\",",
                    "\"interface_pool_id\":\"{}\",",
                    "\"base_instance_id\":\"{}\",",
                    "\"sink_endpoint_id\":\"{}\",",
                    "\"ipv4\":[{},{},{},{}],",
                    "\"port\":{},",
                    "\"maximum_frame_bytes\":{}",
                    "}}\n"
                ),
                identity.firmware_build_id,
                identity.websocket_link.binding.sink.host_id.as_str(),
                identity.websocket_link.binding.sink.boot_id.as_str(),
                identity.attachment_id,
                identity.interface_pool_id,
                identity.websocket_link.binding.base_instance_id.as_str(),
                identity.websocket_link.binding.sink.endpoint_id.as_str(),
                identity.address[0],
                identity.address[1],
                identity.address[2],
                identity.address[3],
                identity.port,
                conduit_r1_network_conformance::R1_MAXIMUM_FRAME_BYTES,
            ),
        )
        .map_err(|_| UsbSignError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    pub async fn write_websocket_link(
        &mut self,
        identity: WebSocketRouteIdentity<'_>,
        websocket_active_play_id: &str,
    ) -> Result<(), UsbSignError> {
        let mut line: HString<1024> = HString::new();
        core::fmt::write(
            &mut line,
            format_args!(
                concat!(
                    "{{",
                    "\"schema\":\"conduit.network/websocket-link-sign@1\",",
                    "\"firmware_build_id\":\"{}\",",
                    "\"host_id\":\"{}\",",
                    "\"runtime_boot_id\":\"{}\",",
                    "\"websocket_active_play_id\":\"{}\",",
                    "\"attachment_id\":\"{}\",",
                    "\"usb_link_binding_id\":\"{}\",",
                    "\"websocket_link_binding_id\":\"{}\",",
                    "\"base_instance_id\":\"{}\",",
                    "\"source_endpoint_id\":\"{}\",",
                    "\"sink_endpoint_id\":\"{}\",",
                    "\"maximum_frame_bytes\":{},",
                    "\"handshake\":true,",
                    "\"sign_id\":\"{}\"",
                    "}}\n"
                ),
                identity.firmware_build_id,
                identity.websocket_link.binding.sink.host_id.as_str(),
                identity.websocket_link.binding.sink.boot_id.as_str(),
                websocket_active_play_id,
                identity.attachment_id,
                identity.usb_link.binding.binding_id.as_str(),
                identity.websocket_link.binding.binding_id.as_str(),
                identity.websocket_link.binding.base_instance_id.as_str(),
                identity.websocket_link.binding.source.endpoint_id.as_str(),
                identity.websocket_link.binding.sink.endpoint_id.as_str(),
                conduit_r1_network_conformance::R1_MAXIMUM_FRAME_BYTES,
                identity.sign_id,
            ),
        )
        .map_err(|_| UsbSignError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    pub async fn write_network_attachment(
        &mut self,
        identity: NetworkAttachmentIdentity<'_>,
    ) -> Result<(), UsbSignError> {
        let mut line: HString<RECEIPT_BUFFER_BYTES> = HString::new();
        core::fmt::write(
            &mut line,
            format_args!(
                concat!(
                    "{{",
                    "\"schema\":\"conduit.network/attachment-sign@1\",",
                    "\"firmware_build_id\":\"{}\",",
                    "\"source_document_id\":\"{}\",",
                    "\"checked_form_id\":\"{}\",",
                    "\"expanded_form_id\":\"{}\",",
                    "\"plan_id\":\"{}\",",
                    "\"fragment_id\":\"{}\",",
                    "\"host_id\":\"{}\",",
                    "\"boot_id\":\"{}\",",
                    "\"active_play_id\":\"{}\",",
                    "\"attachment_id\":\"{}\",",
                    "\"interface_pool_id\":\"{}\",",
                    "\"generation\":{},",
                    "\"sign_id\":\"{}\"",
                    "}}\n"
                ),
                identity.firmware_build_id,
                identity.source_document_id,
                identity.checked_form_id,
                identity.expanded_form_id,
                identity.plan_id,
                identity.fragment_id,
                identity.host_id,
                identity.boot_id,
                identity.active_play_id,
                identity.attachment_id,
                identity.interface_pool_id,
                identity.generation,
                identity.sign_id,
            ),
        )
        .map_err(|_| UsbSignError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    pub async fn write_network_failure(
        &mut self,
        code: &str,
        identity: NetworkAttachmentIdentity<'_>,
    ) -> Result<(), UsbSignError> {
        let mut line: HString<RECEIPT_BUFFER_BYTES> = HString::new();
        core::fmt::write(
            &mut line,
            format_args!(
                concat!(
                    "{{",
                    "\"schema\":\"conduit.network/join-failure-sign@1\",",
                    "\"firmware_build_id\":\"{}\",",
                    "\"source_document_id\":\"{}\",",
                    "\"checked_form_id\":\"{}\",",
                    "\"expanded_form_id\":\"{}\",",
                    "\"plan_id\":\"{}\",",
                    "\"fragment_id\":\"{}\",",
                    "\"host_id\":\"{}\",",
                    "\"boot_id\":\"{}\",",
                    "\"active_play_id\":\"{}\",",
                    "\"interface_pool_id\":\"{}\",",
                    "\"sign_id\":\"{}\",",
                    "\"error_code\":\"{}\"",
                    "}}\n"
                ),
                identity.firmware_build_id,
                identity.source_document_id,
                identity.checked_form_id,
                identity.expanded_form_id,
                identity.plan_id,
                identity.fragment_id,
                identity.host_id,
                identity.boot_id,
                identity.active_play_id,
                identity.interface_pool_id,
                identity.sign_id,
                code,
            ),
        )
        .map_err(|_| UsbSignError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    pub async fn write_network_recovery_failure(
        &mut self,
        code: &str,
        identity: NetworkAttachmentIdentity<'_>,
    ) -> Result<(), UsbSignError> {
        let mut line: HString<1024> = HString::new();
        core::fmt::write(
            &mut line,
            format_args!(
                concat!(
                    "{{",
                    "\"schema\":\"conduit.network/recovery-failure-sign@1\",",
                    "\"firmware_build_id\":\"{}\",",
                    "\"runtime_boot_id\":\"{}\",",
                    "\"runtime_active_play_id\":\"{}\",",
                    "\"sign_id\":\"{}\",",
                    "\"error_code\":\"{}\"",
                    "}}\n"
                ),
                identity.firmware_build_id,
                identity.boot_id,
                identity.active_play_id,
                identity.sign_id,
                code,
            ),
        )
        .map_err(|_| UsbSignError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }
}
