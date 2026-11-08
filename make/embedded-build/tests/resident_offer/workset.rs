//! One signed complete local set reaches the actual Root profile preflight.
use super::*;
use sha2::Digest;

fn raw(documents: &[&[u8]]) -> Vec<u8> {
    let mut bytes = b"CBWSET01".to_vec();
    bytes.extend_from_slice(&(documents.len() as u16).to_le_bytes());
    for document in documents {
        bytes.extend_from_slice(&(document.len() as u32).to_le_bytes());
        bytes.extend_from_slice(document);
    }
    bytes
}
pub fn run(
    a: PreparedBodyFragment<'_>,
    b: PreparedBodyFragment<'_>,
    first: &crate::support::Assignment,
    second: &crate::support::Assignment,
) {
    let left = document(a, first);
    let right = document(b, second);
    let bytes = encode_body_workset_document(&[left.bytes(), right.bytes()], 8192).unwrap();
    let receiver = Receiver::new(a);
    let mut resolved = Vec::new();
    let generated = b::generate_prepared_body_workset(a, &receiver.actual, 8192, |scope| {
        resolved.push(scope.fragment().fragment_id.clone());
        let assignment = if scope.fragment().fragment_id == a.fragment().fragment_id {
            first
        } else {
            assert_eq!(scope.fragment().fragment_id, b.fragment().fragment_id);
            second
        };
        Ok(selection(scope, assignment))
    })
    .unwrap();
    assert_eq!(
        resolved.len(),
        2,
        "Plot and selected Mask must both be derived"
    );
    assert_eq!(generated.bytes(), bytes);
    let from_mask = b::generate_prepared_body_workset(b, &receiver.actual, 8192, |scope| {
        Ok(selection(
            scope,
            if scope.fragment().fragment_id == a.fragment().fragment_id {
                first
            } else {
                second
            },
        ))
    })
    .unwrap();
    assert_eq!(
        from_mask.bytes(),
        generated.bytes(),
        "anchor cannot omit the other fragment"
    );
    for change in 0..3 {
        let mut stale = receiver.actual.clone();
        match change {
            0 => stale.host_id = "fixture/foreign-host".into(),
            1 => stale.boot_id = "fixture/foreign-boot".into(),
            _ => stale.offer_generation.0 += 1,
        }
        assert!(matches!(
            b::generate_prepared_body_workset(a, &stale, 8192, |_| panic!(
                "stale identity reached resolver"
            )),
            Err(b::BodyFragmentGenerationRefusal::Scope)
        ));
    }
    assert!(matches!(
        b::generate_prepared_body_workset(a, &receiver.actual, 0, |_| panic!(
            "zero bound reached resolver"
        )),
        Err(b::BodyFragmentGenerationRefusal::Workset(
            BodyWorksetDocumentRefusal::Bounds
        ))
    ));
    assert!(matches!(
        b::generate_prepared_body_workset(a, &receiver.actual, bytes.len() as u32 - 1, |scope| {
            Ok(selection(
                scope,
                if scope.fragment().fragment_id == a.fragment().fragment_id {
                    first
                } else {
                    second
                },
            ))
        }),
        Err(b::BodyFragmentGenerationRefusal::Workset(
            BodyWorksetDocumentRefusal::Bounds
        ))
    ));
    let mut library = crate::library::Initialized::prepare().unwrap();
    let original = library.offers().map(|image| image.unwrap().as_ptr());
    let (result, cost) = crate::probe::observe(|| {
        crate::root_preparer::validate_workset_with_inventory(
            &library,
            Some(&policy()),
            receiver.context(),
            receiver.command_with_contract(&bytes, HOST_PREPARED_WORKSET_CONTRACT),
            first.maxima,
        )
    });
    assert_eq!(
        result,
        Ok(()),
        "structural bytes: {} + {}",
        first.tables.identity.runtime_state_bytes,
        second.tables.identity.runtime_state_bytes
    );
    assert!(
        bytes.len() <= 4096,
        "this complete workset must fit the actual Root receive slice"
    );
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    root_storage::run(&receiver, &bytes, first, &mut library);
    assert_eq!(
        library.offers().map(|image| image.unwrap().as_ptr()),
        original
    );
    assert_eq!(
        encode_body_workset_document(&[left.bytes()], 8192).err(),
        Some(BodyWorksetDocumentRefusal::IncompleteScope)
    );
    assert_eq!(
        encode_body_workset_document(&[left.bytes(), left.bytes()], 8192).err(),
        Some(BodyWorksetDocumentRefusal::IncompleteScope)
    );
    assert_eq!(
        encode_body_workset_document(&[left.bytes(), right.bytes()], bytes.len() as u32 - 1).err(),
        Some(BodyWorksetDocumentRefusal::Bounds)
    );

    let (token, cost) = crate::probe::observe(|| {
        receive_delegated_body_workset::<2>(
            Some(&policy()),
            receiver.context(),
            receiver.command_with_contract(&bytes, HOST_PREPARED_WORKSET_CONTRACT),
        )
        .unwrap()
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(token.fragment_count(), 2);
    let one = first
        .tables
        .identity
        .runtime_state_bytes
        .max(second.tables.identity.runtime_state_bytes);
    assert!(token.declared_runtime_state_bytes() > u32::from(one));
    let mut narrow = policy();
    narrow.maximum_runtime_state_bytes = u32::from(one);
    assert!(matches!(
        receive_delegated_body_workset::<2>(
            Some(&narrow),
            receiver.context(),
            receiver.command_with_contract(&bytes, HOST_PREPARED_WORKSET_CONTRACT),
        ),
        Err(DelegatedWorksetRefusal::Policy(
            PreparationDelegationRefusal::Bounds
        ))
    ));
    let mut narrow = first.maxima;
    narrow.runtime_state_bytes = one;
    assert_eq!(
        crate::root_preparer::validate_workset_with_inventory(
            &library,
            Some(&policy()),
            receiver.context(),
            receiver.command_with_contract(&bytes, HOST_PREPARED_WORKSET_CONTRACT),
            narrow,
        ),
        Err(crate::root_preparer::ValidationRefusal::Bounds)
    );
    assert_eq!(token.document(0).unwrap().as_ptr(), bytes[14..].as_ptr());
    let pinned = token
        .receive_fragment_with_pinned_fore::<2>(
            0,
            Some(&policy()),
            receiver.context(),
            profile(first),
        )
        .unwrap();
    assert_eq!(pinned.1.as_slice(), first.fore.as_slice());
    assert!(token
        .receive_fragment_with_approved_fore::<2>(
            0,
            Some(&policy()),
            receiver.context(),
            profile(first)
        )
        .is_err());
    let mut wrong_fore = first.fore.clone();
    if !wrong_fore.is_empty() {
        wrong_fore.clear();
        assert!(token
            .receive_fragment_with_pinned_fore::<2>(
                0,
                Some(&policy()),
                receiver.context(),
                AssignedBodyPreparationProfile {
                    fore: Some(&wrong_fore),
                    ..profile(first)
                }
            )
            .is_err());
    }
    let (l, cost) = crate::probe::observe(|| {
        token
            .receive_fragment_with_approved_fore::<2>(
                0,
                Some(&policy()),
                receiver.context(),
                AssignedBodyPreparationProfile {
                    fore: None,
                    ..profile(first)
                },
            )
            .unwrap()
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(l.0.metadata().body_play, a.play().active_play_id.as_str());
    let mut expired = receiver.context();
    expired.now_millis = 20;
    assert!(matches!(
        token.receive_fragment_with_approved_fore::<2>(
            1,
            Some(&policy()),
            expired,
            AssignedBodyPreparationProfile {
                fore: None,
                ..profile(second)
            },
        ),
        Err(DelegatedWorksetRefusal::Policy(
            PreparationDelegationRefusal::Clock
        ))
    ));
    assert!(matches!(
        receive_delegated_body_workset::<2>(
            Some(&policy()),
            receiver.context(),
            receiver.command(&bytes),
        ),
        Err(DelegatedWorksetRefusal::Policy(
            PreparationDelegationRefusal::Contract
        ))
    ));
    assert!(receive_delegated_body_workset::<1>(
        Some(&policy()),
        receiver.context(),
        receiver.command_with_contract(&bytes, HOST_PREPARED_WORKSET_CONTRACT),
    )
    .is_err());
    let bad = b"not a workset";
    assert!(matches!(
        receive_delegated_body_workset::<2>(
            None,
            receiver.context(),
            receiver.command_with_contract(bad, HOST_PREPARED_WORKSET_CONTRACT),
        ),
        Err(DelegatedWorksetRefusal::Policy(
            PreparationDelegationRefusal::MissingDelegation
        ))
    ));

    let mut candidates = vec![raw(&[left.bytes()]), raw(&[left.bytes(), left.bytes()])];
    let mut trailing = bytes.clone();
    trailing.push(0);
    candidates.push(trailing);
    let mut truncated = bytes.clone();
    truncated.pop();
    candidates.push(truncated);
    let mut overflow = bytes.clone();
    overflow[10..14].copy_from_slice(&u32::MAX.to_le_bytes());
    candidates.push(overflow);
    let (mut m, projection) = decode_body_fragment_document(right.bytes()).unwrap();
    m.host = "fixture/another-host";
    let foreign = encode_body_fragment_document(&m, projection, 8192).unwrap();
    candidates.push(raw(&[left.bytes(), &foreign]));
    // A re-signed late artifact mismatch must not consume the earlier owner.
    let (mut m, original_projection) = decode_body_fragment_document(right.bytes()).unwrap();
    let mut projection = original_projection.to_vec();
    projection[c::ASSIGNED_PLAN_HEADER_BYTES + 3 + 36] ^= 1;
    let digest = c::assigned_plan_payload_digest(&projection[c::ASSIGNED_PLAN_HEADER_BYTES..]);
    projection[93..125].copy_from_slice(&digest);
    m.projection_digest = sha2::Sha256::digest(&projection).into();
    let changed = encode_body_fragment_document(&m, &projection, 8192).unwrap();
    candidates.push(encode_body_workset_document(&[left.bytes(), &changed], 8192).unwrap());
    for candidate in &candidates {
        let (result, cost) = crate::probe::observe(|| {
            crate::root_preparer::validate_workset_with_inventory(
                &library,
                Some(&policy()),
                receiver.context(),
                receiver.command_with_contract(candidate, HOST_PREPARED_WORKSET_CONTRACT),
                first.maxima,
            )
        });
        assert!(result.is_err());
        assert_eq!((cost.allocations, cost.reallocations), (0, 0));
        assert_eq!(
            library.offers().map(|image| image.unwrap().as_ptr()),
            original
        );
    }
    let reversed = encode_body_workset_document(&[right.bytes(), left.bytes()], 8192).unwrap();
    assert_eq!(
        crate::root_preparer::validate_workset_with_inventory(
            &library,
            Some(&policy()),
            receiver.context(),
            receiver.command_with_contract(&reversed, HOST_PREPARED_WORKSET_CONTRACT),
            first.maxima,
        ),
        Ok(())
    );
    // The complete set may not assign the same one-instance initialized Back twice.
    let second_id = decode_body_fragment_document(right.bytes())
        .unwrap()
        .0
        .fragment;
    let (mut m, original_projection) = decode_body_fragment_document(left.bytes()).unwrap();
    m.fragment = second_id;
    let mut projection = original_projection.to_vec();
    projection[32..48].copy_from_slice(&c::AssignedIdentity::from_text(second_id).0);
    m.projection_digest = sha2::Sha256::digest(&projection).into();
    let copied = encode_body_fragment_document(&m, &projection, 8192).unwrap();
    let reused = encode_body_workset_document(&[left.bytes(), &copied], 8192).unwrap();
    assert_eq!(
        crate::root_preparer::validate_workset_with_inventory(
            &library,
            Some(&policy()),
            receiver.context(),
            receiver.command_with_contract(&reused, HOST_PREPARED_WORKSET_CONTRACT),
            first.maxima,
        ),
        Err(crate::root_preparer::ValidationRefusal::Selection)
    );
    // Late unavailable providers preserve all other initialized entries.
    let r = token
        .receive_fragment_with_approved_fore::<2>(
            1,
            Some(&policy()),
            receiver.context(),
            AssignedBodyPreparationProfile {
                fore: None,
                ..profile(second)
            },
        )
        .unwrap();
    let (_, back) = library
        .borrow_entry_1(second.selected_scope(AssignedPlayScope::Delegated(&r.0)))
        .unwrap();
    drop(back);
    assert!(crate::root_preparer::validate_workset_with_inventory(
        &library,
        Some(&policy()),
        receiver.context(),
        receiver.command_with_contract(&bytes, HOST_PREPARED_WORKSET_CONTRACT),
        first.maxima,
    )
    .is_err());
    assert!(library.offers()[0].is_some());
    println!("one signed complete local workset: {} bytes; allocation-free actual Root profile preflight; incomplete/duplicate/mixed/late-invalid/unavailable/double-assigned sets refused before provider transfer", bytes.len());
}

#[path = "root_storage.rs"]
mod root_storage;
