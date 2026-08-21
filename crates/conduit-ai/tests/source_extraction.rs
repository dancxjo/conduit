use conduit_ai::{
    deterministic_source_extraction_offer, extract_source, ExtractedSourceValue,
    ResourceMetadataEntry, SourceExtractionCodecRefusal, SourceExtractionLimits,
    SourceExtractionProfile, SourceExtractionReceipt, SourceExtractionRefusal, SourcePayload,
    SourceRef,
};
use conduit_core::{
    AuthorityContractId, AuthorityGrantId, BoundedResourceRef, KindId,
    ResourceDereferenceRequirement, ResourceExtent, ResourceHandleId, ResourceLifetime,
    ResourceReferenceAccessRefusal, ResourceReferenceAvailability, ResourceReferenceBinding,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};

const ACCESS_CLASS: &str = "resource/read-authorized@1";
const AUTHORITY: &str = conduit_ai::SOURCE_READ_AUTHORITY;

fn source(profile: &str, identity: u8, version: u8, bytes: u64, items: Option<u64>) -> SourceRef {
    SourceRef {
        resource: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([identity; 32]),
            content_profile: KindId::from(profile),
            access_class: conduit_core::ResourceClassId::from(ACCESS_CLASS),
            extent: ResourceExtent { bytes, items },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([version; 32]),
                expires_at: None,
            },
        },
    }
}

fn requirement(source: &SourceRef) -> ResourceDereferenceRequirement {
    ResourceDereferenceRequirement {
        content_profile: source.resource.content_profile.clone(),
        access_class: source.resource.access_class.clone(),
        authority_contract: AuthorityContractId::from(AUTHORITY),
        maximum_bytes: source.resource.extent.bytes,
        maximum_items: source.resource.extent.items,
    }
}

fn binding(source: &SourceRef) -> ResourceReferenceBinding {
    ResourceReferenceBinding {
        identity: source.resource.identity,
        version: source.resource.lifetime.version,
        content_profile: source.resource.content_profile.clone(),
        access_class: source.resource.access_class.clone(),
        handle: ResourceHandleId::from("host-local/source-7"),
        authority_contract: AuthorityContractId::from(AUTHORITY),
        authority_grant: AuthorityGrantId::from("grant/read-source-7"),
        maximum_bytes: source.resource.extent.bytes,
        maximum_items: source.resource.extent.items,
        availability: ResourceReferenceAvailability::Available,
    }
}

fn limits(chunk_bytes: u32) -> SourceExtractionLimits {
    SourceExtractionLimits {
        maximum_source_bytes: 1_024,
        maximum_source_items: 32,
        maximum_chunk_bytes: chunk_bytes,
        maximum_chunks: 32,
        maximum_output_bytes: 2_048,
        maximum_work_units: 3_072,
    }
}

#[test]
fn portable_offer_is_finite_and_contains_no_source_or_provider_identity() {
    let offer = deterministic_source_extraction_offer("pid-7").unwrap();
    assert_eq!(offer.kind_id.as_str(), "retrieval/extract-source");
    assert_eq!(offer.inputs[0].value_kind.as_str(), "value/resource-ref@1");
    assert_eq!(
        offer.outputs[0].value_kind.as_str(),
        "retrieval/source-chunks@1"
    );
    assert_eq!(offer.host_operations.len(), 1);
    assert_eq!(offer.resource_requirements.len(), 1);
    assert_eq!(offer.authority_requirements.len(), 1);
    assert_eq!(
        offer.authority_requirements[0].contract_id.as_str(),
        conduit_ai::SOURCE_READ_AUTHORITY
    );
    assert!(offer.limits.max_queue_bytes > 0);
    for forbidden in ["file", "path", "url", "database", "provider", "credential"] {
        assert!(!offer.kind_id.as_str().contains(forbidden));
        assert!(!offer.inputs[0].value_kind.as_str().contains(forbidden));
        assert!(!offer.outputs[0].value_kind.as_str().contains(forbidden));
    }
}

#[test]
fn utf8_text_extraction_is_deterministic_bounded_and_lineage_exact() {
    let payload = SourcePayload::Text("alpha βeta gamma".as_bytes().to_vec());
    let source = source("document/text-utf8@1", 7, 3, 17, None);
    let first = extract_source(
        &source,
        &requirement(&source),
        &binding(&source),
        SourceExtractionProfile::TextUtf8 { overlap_bytes: 2 },
        limits(8),
        &payload,
    )
    .unwrap();
    let second = extract_source(
        &source,
        &requirement(&source),
        &binding(&source),
        SourceExtractionProfile::TextUtf8 { overlap_bytes: 2 },
        limits(8),
        &payload,
    )
    .unwrap();
    assert_eq!(first, second);
    assert_eq!(first.proof_class, "deterministic-source-extraction");
    assert!(first.chunks.len() > 1);
    assert!(first.output_bytes <= 2_048);
    assert!(first.work_units <= 3_072);
    for chunk in &first.chunks {
        chunk.validate().unwrap();
        assert_eq!(chunk.lineage.source, source);
        assert_eq!(chunk.lineage.extraction_profile, "extract/text-utf8@1");
        assert!(chunk.lineage.span.end - chunk.lineage.span.start <= 8);
        let ExtractedSourceValue::Text(value) = &chunk.value else {
            panic!("text profile emitted another value family");
        };
        assert!(core::str::from_utf8(value).is_ok());
    }
}

#[test]
fn extraction_receipt_codec_is_canonical_bounded_and_fail_closed() {
    let source = source("document/text-utf8@1", 7, 3, 17, None);
    let receipt = extract_source(
        &source,
        &requirement(&source),
        &binding(&source),
        SourceExtractionProfile::TextUtf8 { overlap_bytes: 2 },
        limits(8),
        &SourcePayload::Text("alpha βeta gamma".as_bytes().to_vec()),
    )
    .unwrap();
    let encoded = receipt.encode().unwrap();
    assert_eq!(SourceExtractionReceipt::decode(&encoded).unwrap(), receipt);
    assert_eq!(receipt.encode().unwrap(), encoded);

    let mut malformed = encoded.clone();
    malformed.push(0);
    assert_eq!(
        SourceExtractionReceipt::decode(&malformed),
        Err(SourceExtractionCodecRefusal::Malformed)
    );

    let mut wrong_accounting = receipt.clone();
    wrong_accounting.output_bytes += 1;
    assert_eq!(
        wrong_accounting.encode(),
        Err(SourceExtractionCodecRefusal::AccountingMismatch)
    );
}

#[test]
fn structured_and_non_text_metadata_profiles_preserve_item_ranges() {
    let records = vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()];
    let structured_source = source("records/project-status@1", 8, 1, 11, Some(3));
    let structured = extract_source(
        &structured_source,
        &requirement(&structured_source),
        &binding(&structured_source),
        SourceExtractionProfile::StructuredItems { overlap_items: 1 },
        limits(8),
        &SourcePayload::StructuredItems(records),
    )
    .unwrap();
    assert_eq!(structured.chunks.len(), 2);
    assert_eq!(structured.chunks[0].lineage.span.start, 0);
    assert_eq!(structured.chunks[0].lineage.span.end, 2);
    assert_eq!(structured.chunks[1].lineage.span.start, 1);
    assert_eq!(structured.chunks[1].lineage.span.end, 3);

    let metadata = vec![
        ResourceMetadataEntry {
            field: "media-type".into(),
            value: "image/png".into(),
        },
        ResourceMetadataEntry {
            field: "width".into(),
            value: "320".into(),
        },
    ];
    let metadata_source = source("resource/image-metadata@1", 9, 2, 27, Some(2));
    let receipt = extract_source(
        &metadata_source,
        &requirement(&metadata_source),
        &binding(&metadata_source),
        SourceExtractionProfile::ResourceMetadata { overlap_items: 0 },
        limits(32),
        &SourcePayload::ResourceMetadata(metadata),
    )
    .unwrap();
    assert_eq!(receipt.chunks.len(), 1);
    assert_eq!(
        receipt.chunks[0].lineage.extraction_profile,
        "extract/resource-metadata@1"
    );
    assert!(matches!(
        receipt.chunks[0].value,
        ExtractedSourceValue::ResourceMetadata(_)
    ));
}

#[test]
fn source_mutation_deletion_duplicate_text_and_range_shift_remain_exact() {
    let payload = SourcePayload::Text(b"same text".to_vec());
    let original = source("document/text-utf8@1", 4, 1, 9, None);
    let changed = source("document/text-utf8@1", 4, 2, 9, None);
    let duplicate = source("document/text-utf8@1", 5, 1, 9, None);
    let extract = |source: &SourceRef, chunk_bytes| {
        extract_source(
            source,
            &requirement(source),
            &binding(source),
            SourceExtractionProfile::TextUtf8 { overlap_bytes: 0 },
            limits(chunk_bytes),
            &payload,
        )
        .unwrap()
    };
    let original_chunks = extract(&original, 9);
    assert_ne!(
        original_chunks.chunks[0].identity,
        extract(&changed, 9).chunks[0].identity
    );
    assert_ne!(
        original_chunks.chunks[0].identity,
        extract(&duplicate, 9).chunks[0].identity
    );
    assert_ne!(
        original_chunks.chunks[0].lineage.span,
        extract(&original, 5).chunks[1].lineage.span
    );

    let stale_binding = binding(&original);
    assert_eq!(
        extract_source(
            &changed,
            &requirement(&changed),
            &stale_binding,
            SourceExtractionProfile::TextUtf8 { overlap_bytes: 0 },
            limits(9),
            &payload,
        ),
        Err(SourceExtractionRefusal::ResourceAccess(
            ResourceReferenceAccessRefusal::StaleVersion
        ))
    );
    let mut lost = binding(&original);
    lost.availability = ResourceReferenceAvailability::Lost;
    assert_eq!(
        extract_source(
            &original,
            &requirement(&original),
            &lost,
            SourceExtractionProfile::TextUtf8 { overlap_bytes: 0 },
            limits(9),
            &payload,
        ),
        Err(SourceExtractionRefusal::ResourceAccess(
            ResourceReferenceAccessRefusal::ResourceLost
        ))
    );
}

#[test]
fn malformed_extent_profile_and_every_finite_bound_fail_closed() {
    let source_value = source("document/text-utf8@1", 7, 3, 4, None);
    let data = SourcePayload::Text(b"data".to_vec());
    let malformed = SourcePayload::Text(vec![0xff, 0xff, 0xff, 0xff]);
    let run = |profile, limits, payload| {
        extract_source(
            &source_value,
            &requirement(&source_value),
            &binding(&source_value),
            profile,
            limits,
            payload,
        )
    };
    assert_eq!(
        run(
            SourceExtractionProfile::TextUtf8 { overlap_bytes: 0 },
            limits(4),
            &malformed,
        ),
        Err(SourceExtractionRefusal::InvalidUtf8)
    );
    assert_eq!(
        run(
            SourceExtractionProfile::StructuredItems { overlap_items: 0 },
            limits(4),
            &data,
        ),
        Err(SourceExtractionRefusal::PayloadProfileMismatch)
    );
    let mut too_small = limits(4);
    too_small.maximum_source_bytes = 3;
    assert_eq!(
        run(
            SourceExtractionProfile::TextUtf8 { overlap_bytes: 0 },
            too_small,
            &data,
        ),
        Err(SourceExtractionRefusal::SourceBoundExceeded)
    );
    let mut one_chunk = limits(2);
    one_chunk.maximum_chunks = 1;
    assert_eq!(
        run(
            SourceExtractionProfile::TextUtf8 { overlap_bytes: 0 },
            one_chunk,
            &data,
        ),
        Err(SourceExtractionRefusal::ChunkCountExceeded)
    );
    let mut output = limits(3);
    output.maximum_output_bytes = 3;
    assert_eq!(
        run(
            SourceExtractionProfile::TextUtf8 { overlap_bytes: 1 },
            output,
            &data,
        ),
        Err(SourceExtractionRefusal::OutputBoundExceeded)
    );
    let mut work = limits(4);
    work.maximum_work_units = 7;
    assert_eq!(
        run(
            SourceExtractionProfile::TextUtf8 { overlap_bytes: 0 },
            work,
            &data,
        ),
        Err(SourceExtractionRefusal::WorkBoundExceeded)
    );
    assert_eq!(
        run(
            SourceExtractionProfile::TextUtf8 { overlap_bytes: 4 },
            limits(4),
            &data,
        ),
        Err(SourceExtractionRefusal::InvalidOverlap)
    );

    let empty_source = source("document/text-utf8@1", 10, 1, 0, None);
    assert_eq!(
        extract_source(
            &empty_source,
            &requirement(&empty_source),
            &binding(&empty_source),
            SourceExtractionProfile::TextUtf8 { overlap_bytes: 0 },
            limits(4),
            &SourcePayload::Text(Vec::new()),
        ),
        Err(SourceExtractionRefusal::EmptySource)
    );
}

#[cfg(feature = "form-catalog")]
#[test]
fn ordinary_authored_form_checks_and_expands_without_realization_facts() {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_ai::install_source_extraction_catalog(&mut startup, &mut profile).unwrap();
    let source = "form chunk {\n extract: retrieval/extract-source(\"text-utf8\", 4096, 32, 8192, 512, 16, 16384)\n}\n";
    let checked =
        conduit_form::check_syntax_document(&conduit_form::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded = conduit_form::expand_canonical_form(&checked, "chunk", &profile).unwrap();
    assert_eq!(
        expanded.gears[0].kind_id.as_str(),
        "retrieval/extract-source"
    );
    for forbidden in ["host", "provider", "file", "path", "url", "database"] {
        assert!(!source.contains(forbidden));
    }
}
