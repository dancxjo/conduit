#![cfg(feature = "semantic-bindings")]
//! Actual per-program custody capacity; serialized identity sizes are not heap usage.
use conduit_plot::rust_binding::{NativeLawRegistry, PreparedNativeBounds};
use conduit_plot::*;
use std::collections::BTreeSet;
#[path = "common/native_law_program.rs"]
mod native_law_program;
use native_law_program::*;

#[test]
fn each_original_g2_program_fits_one_sealed_registry() {
    let original = conduit_speech::authoring::checked_document_with_law_custody().unwrap();
    let speech_metadata = conduit_speech::authoring::checked_types().unwrap();
    let audio_metadata = conduit_audio::authoring::checked_types().unwrap();
    assert_eq!(original.native_types, speech_metadata);
    let mut failures = Vec::new();
    for short in ["advance", "phases", "curve", "energy"] {
        let name = format!("speech/phone-gesture-{short}");
        let encoded = program(&original, &name);
        let value = PortableExpressionProgram::from_canonical_hex(&encoded).unwrap();
        let mut all = BTreeSet::new();
        types(&value.input_type, &mut all);
        types(&value.output_type, &mut all);
        nodes(&value.root, &mut all);
        let mut speech_names = BTreeSet::new();
        let mut audio_names = BTreeSet::new();
        for ty in &all {
            if let Some(owner) = speech_metadata.iter().find(|owner| &owner.value_type == ty) {
                speech_names.insert(owner.name.as_str());
            } else if let Some(owner) = audio_metadata.iter().find(|owner| &owner.value_type == ty)
            {
                audio_names.insert(owner.name.as_str());
            } else {
                assert_builtin_structure(ty);
            }
        }
        let speech = conduit_speech::authoring::checked_document_with_law_custody_for(
            &speech_names.iter().copied().collect::<Vec<_>>(),
        )
        .unwrap();
        let audio = conduit_audio::authoring::checked_document_with_law_custody_for(
            &audio_names.iter().copied().collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(speech.source_document_id, original.source_document_id);
        assert_eq!(speech.native_types, speech_metadata);
        assert_eq!(audio.native_types, audio_metadata);
        assert_eq!(program(&speech, &name), encoded);
        let mut witnesses = Vec::new();
        for (document, expected, names) in [
            (&speech, &speech_metadata, &speech_names),
            (&audio, &audio_metadata, &audio_names),
        ] {
            for root in names {
                let witness = document.native_law_witness(root).unwrap();
                let expected = expected.iter().find(|ty| ty.name == *root).unwrap();
                assert_eq!(witness.checked_type(), expected);
                let material = witness.material().unwrap();
                eprintln!(
                    "G2_REGISTRY_ROOT program={short} root={root} root_source_bytes={} root_type_identity_bytes={} root_predicate_identity_bytes={}",
                    material.source.len(),
                    material.claimed.value_type.canonical_bytes().unwrap().len(),
                    material.claimed.invariants.iter().map(|program| program.canonical_bytes().unwrap().len()).sum::<usize>()
                );
                witnesses.push(witness.clone());
            }
        }
        eprintln!("G2_REGISTRY_PROGRAM program={short} native_roots={} nested_types={} canonical_hex_bytes={}", witnesses.len(), all.len(), encoded.len());
        let registry = match NativeLawRegistry::new(&witnesses) {
            Ok(registry) => registry,
            Err(cause) => {
                failures.push(format!("{short}: actual registry refused {cause:?}"));
                eprintln!("G2_REGISTRY_REFUSAL program={short} cause={cause:?}");
                continue;
            }
        };
        assert_eq!(registry.source_witnesses().len(), witnesses.len());
        for witness in &witnesses {
            let prepared = match registry.prepare_native(
                &witness.checked_type().identity,
                PreparedNativeBounds {
                    canonical_bytes: 262144,
                    scopes: 4096,
                    invariants: 4096,
                    contracts: 4096,
                },
            ) {
                Ok(prepared) => prepared,
                Err(cause) => {
                    failures.push(format!(
                        "{short}/{}: guard preparation refused {cause:?}",
                        witness.checked_type().name
                    ));
                    eprintln!(
                        "G2_REGISTRY_GUARD_REFUSAL program={short} root={} cause={cause:?}",
                        witness.checked_type().name
                    );
                    continue;
                }
            };
            assert_eq!(
                prepared.native_type().value_type,
                witness.checked_type().value_type
            );
            eprintln!(
                "G2_REGISTRY_GUARD program={short} root={} prepared={:?}",
                witness.checked_type().name,
                prepared.capacity_report()
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}
