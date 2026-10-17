use conduit_core::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_process::*;

pub fn identity(text: &str) -> PrintIdentity {
    PrintIdentity::new(text.into()).unwrap()
}
pub fn reference() -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([7; 32]),
        content_profile: PRINT_DOCUMENT_CONTENT_PROFILE.into(),
        access_class: PRINT_DOCUMENT_ACCESS_CLASS.into(),
        extent: ResourceExtent {
            bytes: 4096,
            items: Some(2),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([8; 32]),
            expires_at: None,
        },
    }
}
pub fn binding() -> ResourceReferenceBinding {
    let document = reference();
    ResourceReferenceBinding {
        identity: document.identity,
        version: document.lifetime.version,
        content_profile: document.content_profile,
        access_class: document.access_class,
        handle: "host/document-handle".into(),
        authority_contract: PRINT_DOCUMENT_AUTHORITY.into(),
        authority_grant: "grant/print-document".into(),
        maximum_bytes: PRINT_MAXIMUM_DOCUMENT_BYTES,
        maximum_items: Some(1000),
        availability: ResourceReferenceAvailability::Available,
    }
}
pub fn request(media: PrintMedia, color: PrintColor, sides: PrintSides) -> PrintRequest {
    PrintRequest::new(
        PrintAttributes::new(color, 1, media, sides).unwrap(),
        PrintDocument::new(reference()).unwrap(),
        identity("job/one"),
    )
    .unwrap()
}
pub fn support(color: bool) -> PrintSupport {
    PrintSupport::new(
        PrintColorSupport::new(
            BoundedSequence::try_from_iter(if color {
                vec![PrintColor::Monochrome, PrintColor::Color]
            } else {
                vec![PrintColor::Monochrome]
            })
            .unwrap(),
        )
        .unwrap(),
        10,
        PrintMediaSupport::new(
            BoundedSequence::try_from_iter([PrintMedia::A4, PrintMedia::Letter]).unwrap(),
        )
        .unwrap(),
        PrintSidesSupport::new(
            BoundedSequence::try_from_iter([PrintSides::OneSided, PrintSides::DuplexLongEdge])
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
