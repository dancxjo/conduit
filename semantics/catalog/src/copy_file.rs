use super::{KindTerminalBehavior, StandardKindContract};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, CapabilityLimits, PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
};
use conduit_plot::rust_binding::NativeRustBinding;

pub const COPY_FILE_KIND: &str = "file/copy";
pub const COPY_FILE_CONTRACT_REVISION: &str = "conduit.std/file-copy@1";
pub const PROTECTED_FILE_RESOURCE_CLASS: &str = "conduit.resource/protected-file@1";
pub const COPY_SOURCE_ROLE: &str = "source";
pub const COPY_DESTINATION_ROLE: &str = "destination";
pub const COPY_CHUNK_BYTES: u32 = 4_096;
pub const COPY_RESULT_TYPE: &str = "FileCopyResult";

pub fn copy_result_type() -> StructuredInfoType {
    conduit_data::FileCopyResult::semantic_type().expect("checked file-copy result Type")
}

pub fn copy_success_value(bytes_copied: u64) -> Result<conduit_core::StructuredInfoValue, String> {
    let bytes_copied = i64::try_from(bytes_copied)
        .map_err(|_| "copied byte count exceeds the quantity profile".to_string())?;
    let quantity = conduit_core::Quantity::new(bytes_copied, conduit_core::QuantityUnit::Byte);
    let outcome = conduit_data::FileCopyOutcome::success(quantity)
        .map_err(|error| format!("construct file copy result: {error:?}"))?;
    conduit_data::FileCopyResult::new(outcome)
        .and_then(NativeRustBinding::into_structured)
        .map_err(|error| format!("encode file copy result: {error:?}"))
}

fn result_port(direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: conduit_core::port_id(if direction == PortDirection::Output {
            "result"
        } else {
            "input"
        }),
        value_kind: copy_result_type().profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}

pub fn copy_file_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(COPY_FILE_KIND),
        plain_name: "Copy a file".to_string(),
        summary: "Copy one protected source into one protected destination in bounded steps."
            .to_string(),
        inputs: Vec::new(),
        outputs: vec![result_port(PortDirection::Output)],
        configuration: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
        terminal_behavior: KindTerminalBehavior::CompletesAfterFixedCount { count: 1 },
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "task: file/copy".to_string(),
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_copy_file_catalog(catalog: &mut conduit_plot::ProfileCatalog) -> Result<(), String> {
    for definition in [
        copy_file_contract().into_semantic_contract(COPY_FILE_CONTRACT_REVISION),
        crate::structured_presentation_contract(COPY_RESULT_TYPE, &copy_result_type()).into(),
    ] {
        catalog
            .insert_kind(definition)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
