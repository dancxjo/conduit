use heapless::String as HString;

use crate::receipts::{UsbCdc, UsbClueError, RECEIPT_BUFFER_BYTES};

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
    pub clue_id: &'static str,
}

impl UsbCdc {
    pub async fn write_network_attachment(
        &mut self,
        identity: NetworkAttachmentIdentity<'_>,
    ) -> Result<(), UsbClueError> {
        let mut line: HString<RECEIPT_BUFFER_BYTES> = HString::new();
        core::fmt::write(
            &mut line,
            format_args!(
                concat!(
                    "{{",
                    "\"schema\":\"conduit.network/attachment-clue@1\",",
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
                    "\"clue_id\":\"{}\"",
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
                identity.clue_id,
            ),
        )
        .map_err(|_| UsbClueError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    pub async fn write_network_failure(
        &mut self,
        code: &str,
        identity: NetworkAttachmentIdentity<'_>,
    ) -> Result<(), UsbClueError> {
        let mut line: HString<RECEIPT_BUFFER_BYTES> = HString::new();
        core::fmt::write(
            &mut line,
            format_args!(
                concat!(
                    "{{",
                    "\"schema\":\"conduit.network/join-failure-clue@1\",",
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
                    "\"clue_id\":\"{}\",",
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
                identity.clue_id,
                code,
            ),
        )
        .map_err(|_| UsbClueError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }

    pub async fn write_network_recovery_failure(
        &mut self,
        code: &str,
        identity: NetworkAttachmentIdentity<'_>,
    ) -> Result<(), UsbClueError> {
        let mut line: HString<1024> = HString::new();
        core::fmt::write(
            &mut line,
            format_args!(
                concat!(
                    "{{",
                    "\"schema\":\"conduit.network/recovery-failure-clue@1\",",
                    "\"firmware_build_id\":\"{}\",",
                    "\"runtime_boot_id\":\"{}\",",
                    "\"runtime_active_play_id\":\"{}\",",
                    "\"clue_id\":\"{}\",",
                    "\"error_code\":\"{}\"",
                    "}}\n"
                ),
                identity.firmware_build_id,
                identity.boot_id,
                identity.active_play_id,
                identity.clue_id,
                code,
            ),
        )
        .map_err(|_| UsbClueError::FormatOverflow)?;
        self.write_all_mandatory(line.as_bytes()).await
    }
}
