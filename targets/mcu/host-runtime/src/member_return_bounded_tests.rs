use super::*;

#[test]
fn temporary_hash_and_credential_allocation_pressure_refuses_without_transport_authority() {
    let (bootstrap, actual, member, owner) = fixture();
    let transaction = RetainedReturn {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        retained: bootstrap.admission.credential().unwrap(),
    };
    for denied in [1, 2] {
        let incoming: Vec<_> = messages(&bootstrap, &actual, &member, &owner)
            .incoming
            .into_iter()
            .collect();
        let mut link = BorrowedMessages {
            incoming: &incoming,
            next: 0,
            sent: 0,
        };
        let mut input = [0; 9216];
        let mut output = [0; 3072];
        let (result, cost) = return_allocation_probe::observe_refusing(denied, || {
            ready(transaction.exchange_with_grants(
                &mut link,
                [IMAGE].into_iter(),
                &[] as &[conduit_core::AuthorityGrant],
                &mut input,
                &mut output,
            ))
        });
        assert_eq!(cost.refused_allocations, 1);
        assert_eq!(
            result.err(),
            Some(if denied == 1 {
                Refusal::Allocation {
                    requested_bytes: 32,
                }
            } else {
                Refusal::Receipt
            })
        );
        if denied == 1 {
            assert_eq!(link.sent, 0);
        }
    }
}

#[test]
fn bounded_context_decoder_refuses_large_known_fields_without_any_heap_request() {
    use serde::de::DeserializeSeed;
    let (_, mut actual, _, _) = fixture();
    actual.resources.push(conduit_core::ResourceOffer {
        pool_id: "pool/plain".into(),
        class_id: "class/plain".into(),
        capacity_units: 1,
        compute: None,
        content: None,
    });
    let mut cases = vec![serde_json::to_value(&actual).unwrap()];
    let mut huge = cases[0].clone();
    huge["host_id"] = "x".repeat(18000).into();
    cases.push(huge);
    let mut extra = cases[0].clone();
    extra["capabilities"] = serde_json::json!([{"huge":"x".repeat(18000)}]);
    cases.push(extra);
    let mut contrary = cases[0].clone();
    contrary["resources"][0]["compute"] = serde_json::json!({"huge":"x".repeat(18000)});
    cases.push(contrary);
    for (index, value) in cases.iter().enumerate() {
        let bytes = serde_json::to_vec(value).unwrap();
        let (result, cost) = return_allocation_probe::observe(|| {
            let mut decoder = serde_json_core::de::Deserializer::new(&bytes, None);
            conduit_core::HostAdvertisementEqualitySeed::new(&actual)
                .unwrap()
                .deserialize(&mut decoder)
        });
        assert_eq!(result.is_ok(), index == 0);
        assert_eq!((cost.allocation_attempts, cost.reallocations), (0, 0));
    }
}

#[test]
fn bounded_host_comparison_preserves_actual_embedded_escape_semantics() {
    use serde::de::DeserializeSeed;
    let (_, mut expected, _, _) = fixture();
    for text in ["plain", "quote\"", "slash\\", "newline\n", "温度"] {
        expected.profile = text.into();
        let bytes = serde_json::to_vec(&expected).unwrap();
        for unescape in [false, true] {
            let ordinary = if unescape {
                serde_json_core::from_slice_escaped::<HostAdvertisement>(&bytes, &mut [0; 512])
            } else {
                serde_json_core::from_slice::<HostAdvertisement>(&bytes)
            }
            .is_ok_and(|(actual, used)| used == bytes.len() && actual == expected);
            let mut scratch = [0; 512];
            let mut decoder = serde_json_core::de::Deserializer::new(
                &bytes,
                unescape.then_some(scratch.as_mut_slice()),
            );
            let result = conduit_core::HostAdvertisementEqualitySeed::new(&expected)
                .unwrap()
                .deserialize(&mut decoder);
            let same = result.is_ok() && decoder.end().is_ok_and(|used| used == bytes.len());
            assert_eq!(same, ordinary, "{text:?}, unescape={unescape}");
        }
    }
}

#[test]
fn hash_reservation_funds_actual_inventory_and_refuses_before_transport() {
    let (bootstrap, actual, member, _) = fixture();
    let transaction = RetainedReturn {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        retained: bootstrap.admission.credential().unwrap(),
    };
    for count in [1, 10, 25, 32] {
        let mut link = BorrowedMessages {
            incoming: &[],
            next: 0,
            sent: 0,
        };
        let mut input = [0; 9216];
        let mut output = [0; 3072];
        let (result, cost) = return_allocation_probe::observe_refusing(1, || {
            ready(transaction.exchange_with_grants(
                &mut link,
                core::iter::repeat_n(IMAGE, count),
                &[] as &[conduit_core::AuthorityGrant],
                &mut input,
                &mut output,
            ))
        });
        assert_eq!(
            result.err(),
            Some(Refusal::Allocation {
                requested_bytes: count * 32
            })
        );
        assert_eq!(cost.allocation_attempts, 1);
        assert_eq!(cost.reallocations, 0);
        assert_eq!(link.sent, 0);
    }
}

#[test]
fn invalid_inventory_refuses_before_any_allocation_or_grant_copy() {
    let (bootstrap, actual, member, _) = fixture();
    let transaction = RetainedReturn {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        retained: bootstrap.admission.credential().unwrap(),
    };
    let oversized = [0u8; 24 * 1024 + 1];
    let cases = [vec![IMAGE; 33], vec![&[][..]], vec![oversized.as_slice()]];
    for images in cases {
        let mut link = BorrowedMessages {
            incoming: &[],
            next: 0,
            sent: 0,
        };
        let copied = core::cell::Cell::new(false);
        let mut input = [0; 9216];
        let mut output = [0; 3072];
        let (result, cost) = return_allocation_probe::observe(|| {
            ready(transaction.exchange(
                &mut link,
                images.iter().copied(),
                || {
                    copied.set(true);
                    Ok(vec![])
                },
                &mut input,
                &mut output,
            ))
        });
        assert_eq!(result.err(), Some(Refusal::Bounds));
        assert_eq!(cost.allocation_attempts, 0);
        assert!(!copied.get());
        assert_eq!(link.sent, 0);
    }
}
