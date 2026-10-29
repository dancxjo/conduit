//! Package the exact authored diagnostic entry; no native authority is created.
use crate::protocol_source::{
    PreparedProtocolEntry, ProtocolSourcePackage, ProtocolSourceRefusal,
    ProtocolSpecializationRequest as Request, ProtocolValueReference,
};

/// Immutable Host workspace for this finite diagnostic. Each chunk contributes
/// one ingress record and offered/accepted/delivered output records; one final
/// output and each Fore closure are also retained. Local computation receives
/// a separate finite reservation, never allocation or discarded mandatory logs.
pub(crate) fn sign_storage(
    chunks: u64,
    fore_count: u16,
) -> Result<conduit_composite::KernelCompositeSignStorage, &'static str> {
    let chunks = u16::try_from(chunks).map_err(|_| "minipitft-sign-envelope-overflow")?;
    Ok(conduit_composite::KernelCompositeSignStorage {
        additional_local_items: 50_000,
        additional_remote_items: chunks
            .checked_mul(4)
            .and_then(|items| items.checked_add(3))
            .and_then(|items| items.checked_add(fore_count))
            .ok_or("minipitft-sign-envelope-overflow")?,
    })
}

pub(crate) fn entry() -> Result<PreparedProtocolEntry, ProtocolSourceRefusal> {
    let source = [
        include_str!("../../../../plots/device-protocols/st7789-autonomous-imports.conduit"),
        include_str!("../../../../plots/device-protocols/st7789-types.conduit"),
        include_str!("../../../../plots/device-protocols/st7789-control.conduit"),
        include_str!("../../../../plots/device-protocols/st7789-transition.conduit"),
        include_str!("../../../../plots/device-protocols/st7789-autonomous-helpers.conduit"),
        include_str!("../../../../plots/device-protocols/st7789-autonomous.conduit"),
    ]
    .join("\n");
    let value = |name: &str, maximum_bytes| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes,
    };
    let package = ProtocolSourcePackage::compile(
        source,
        &[
            Request::Singleton {
                value: value("St7789Begin", 512),
            },
            Request::SeededUntil {
                value: value("St7789Frame", 4096),
            },
            Request::FeedbackZip {
                left: value("St7789State", 4096),
                right: value("St7789Event", 4096),
            },
            Request::Zip {
                left: value("St7789Context", 256),
                right: value("St7789Outcome", 4096),
            },
            Request::MergeInstance {
                alias: "st7789-contexts".into(),
                value: value("St7789Context", 256),
            },
            Request::MergeInstance {
                alias: "st7789-outcomes".into(),
                value: value("St7789Outcome", 4096),
            },
        ],
    )?;
    let bytes = serde_json::to_vec(&package).map_err(|_| ProtocolSourceRefusal::Encoding)?;
    PreparedProtocolEntry::prepare(&bytes, "st7789-autonomous")
}
