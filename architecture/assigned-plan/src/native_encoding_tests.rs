use super::*;
fn binding(boot: &str, instance: &str, generation: u64) -> AssignedNativeBinding {
    AssignedNativeBinding {
        capability: Id::from_text("capability"),
        kind_revision: Id::from_text("kind/1"),
        execution_profile: Id::from_text("profile"),
        maximum_instances: 2,
        queue_items: 3,
        queue_bytes: 1234,
        base: Id::from_text("base"),
        instance: Id::from_text(instance),
        generation,
        base_implementation: Id::from_text("implementation"),
        mechanism: Id::from_text("mechanism"),
        enforcement: 0,
        grant: Id::from_text(boot),
        authority: Id::from_text("authority"),
        call: Id::from_text("call"),
        subject: Id::from_text("subject"),
        host: Id::from_text("host"),
        boot: Id::from_text(boot),
        authority_capability: Id::from_text("authority-capability"),
        resource: 7,
        pool: Id::from_text("pool"),
        resource_class: Id::from_text("class"),
        units: 19,
    }
}
#[test]
fn exact_native_encoding_roundtrips_actual_scope_fields_on_two_boots() {
    let prefix = core::array::from_fn(|i| i as u8);
    let mut prior = [0; ASSIGNED_NATIVE_CALL_BYTES];
    for (index, (boot, provider)) in [("boot/one", "provider/one"), ("boot/two", "provider/two")]
        .into_iter()
        .enumerate()
    {
        let binding = binding(boot, provider, index as u64 + 1);
        let mut output = [0x5a; ASSIGNED_NATIVE_CALL_BYTES + 1];
        assert_eq!(
            encode_assigned_native_binding(&prefix, Some(&binding), &mut output),
            Ok(326)
        );
        assert_eq!(&output[..46], &prefix);
        assert_eq!(output[326], 0x5a);
        assert_eq!(assigned_native_binding(&output[..326]), Ok(Some(binding)));
        if index == 0 {
            prior.copy_from_slice(&output[..326]);
        } else {
            assert_ne!(prior, output[..326]);
        }
    }
}
#[test]
fn no_binding_preserves_prefix_and_uses_exact_existing_short_shape() {
    let mut output = [0x5a; 326];
    assert_eq!(
        encode_assigned_native_binding(&[7; 46], None, &mut output),
        Ok(47)
    );
    assert_eq!(&output[..46], &[7; 46]);
    assert_eq!(output[46], 0);
    assert_eq!(assigned_native_binding(&output[..47]), Ok(None));
    assert!(output[47..].iter().all(|b| *b == 0x5a));
}
#[test]
fn every_short_output_and_invalid_binding_refuses_without_partial_publication() {
    let valid = binding("boot/one", "provider/one", 1);
    for length in 0..326 {
        let mut output = [0x5a; 326];
        assert_eq!(
            encode_assigned_native_binding(&[7; 46], Some(&valid), &mut output[..length]),
            Err(E::WrongLength)
        );
        assert_eq!(output, [0x5a; 326]);
    }
    for field in 0..3 {
        let mut invalid = valid;
        match field {
            0 => invalid.enforcement = 1,
            1 => invalid.generation = 0,
            _ => invalid.units = 0,
        }
        let mut output = [0x5a; 326];
        assert_eq!(
            encode_assigned_native_binding(&[7; 46], Some(&invalid), &mut output),
            Err(E::MalformedRecord)
        );
        assert_eq!(output, [0x5a; 326]);
    }
}
#[test]
fn parser_rejects_tampered_presence_generation_units_and_enforcement() {
    let mut payload = [0; 326];
    encode_assigned_native_binding(
        &[7; 46],
        Some(&binding("boot/one", "provider/one", 1)),
        &mut payload,
    )
    .unwrap();
    // Offsets follow the existing schema6 layout; structural validation cannot
    // authenticate arbitrary identity changes, which still need outer digests.
    for (start, length) in [(135, 8), (322, 4)] {
        let mut changed = payload;
        changed[start..start + length].fill(0);
        assert_eq!(assigned_native_binding(&changed), Err(E::MalformedRecord));
    }
    for offset in [46, 175] {
        let mut changed = payload;
        changed[offset] = 2;
        assert_eq!(assigned_native_binding(&changed), Err(E::MalformedRecord));
    }
    let mut identity = payload;
    identity[47] ^= 1;
    assert_ne!(
        assigned_native_binding(&identity),
        assigned_native_binding(&payload)
    );
}
